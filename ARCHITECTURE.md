# Architecture — a pluggable agent framework (Rust kernel + Flutter plugin UI)

> Two ideas are borrowed from **DeepSeek Harness (DSH)** and its **Cordis** runtime:
> *everything is a plugin*, and *plugins compose across space and time*. superapp keeps both
> and swaps the substrate: the runtime moves from **Node.js to Rust**, and the interface moves
> from a built-in web app to a **Flutter shell in which the UI itself is a plugin**.
>
> Goal: one memory-safe, model-neutral, hot-swappable agent runtime that runs as a native
> binary on desktop and links into a mobile app.

**This document describes the target design, not the current state of the tree.** The kernel
today is ~460 lines across seven modules: it boots, activates statically linked plugins, and
records a trace. Everything else here is a contract or a plan. Each section is tagged so the
two never get confused:

| Tag | Meaning |
|-----|---------|
| ✅ **built** | Implemented and exercised by `cargo run -p cordis-rs --example boot` |
| 🟡 **contract** | Types/signatures exist and compile, behavior does not |
| ⬜ **planned** | Not in the tree at all |

See [`README.md`](./README.md) for the per-subsystem status list and the known gaps.

## Contents

1. [Design philosophy](#1-design-philosophy-everything-is-a-plugin)
2. [Layered architecture](#2-layered-architecture)
3. [The Rust kernel (Cordis-RS)](#3-the-rust-kernel-cordis-rs)
4. [Plugin categories and contracts](#4-plugin-categories-and-contracts)
5. [Sandboxing](#5-sandboxing-two-tracks)
6. [Flutter plugin UI](#6-flutter-plugin-ui)
7. [End-to-end traceability](#7-end-to-end-traceability)
8. [Repository layout](#8-repository-layout)
9. [Capability parity with DSH](#9-capability-parity-with-dsh)
10. [Why Rust: expected performance envelope](#10-why-rust-expected-performance-envelope)
11. [Open design questions](#11-open-design-questions)

---

## 1. Design philosophy: everything is a plugin

The paradigm is DSH's; the mechanics are what Rust makes natural.

| Dimension | DSH (Node.js + web UI) | superapp (Rust kernel + Flutter UI) |
|-----------|------------------------|-------------------------------------|
| Kernel language | TypeScript, ~2 700 lines | Rust — ~460 lines today, ~3 000 budgeted |
| Plugin artifact | npm package + TS decorators | Static crate today; dynamic library (`.so`/`.dylib`/`.dll`) or WASM module planned |
| Hot-swap engine | Cordis, via TS reflection | **Cordis-RS**, via trait objects + scoped lifetimes |
| UI | built-in web app on port 3030 | **Flutter**, with attachable UI plugins ⬜ |
| Sandbox | OS-level (Linux) | `seccomp`/namespaces **and** a WASM micro-sandbox ⬜ |
| Model bindings | ~40 providers | providers are plugins; **one echo stub exists so far** |

The contract shift is the substantive one. DSH attaches metadata with decorators and reflects
over it at load time, so a malformed plugin is a runtime failure. In Cordis-RS a plugin is a
type implementing `Plugin`, so the same mistake is a compile error — and the price is that
plugin authors must rebuild against a matching kernel ABI.

### Space/time composability

- **Space.** Each plugin registers into its own `Scope`. Dropping the scope is meant to
  cascade-release every service, subscription, and handle registered under it — RAII in place
  of a teardown callback that can be forgotten. Scopes nest today (`Scope::root()`,
  `Scope::child()`, parents held via `Weak` to avoid cycles); nothing registers disposable
  resources into one yet, so the release path is untested. 🟡
- **Time.** A run is an append-only event log, not mutable session state, which is what makes
  resume, fork, and replay possible at all. `TraceCollector` records plugin lifecycle entries
  and checkpoints today; agent events do not reach it yet. 🟡

---

## 2. Layered architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                 Flutter UI layer (Dart, pluggable)      ⬜ planned │
│  ┌───────────┐ ┌───────────┐ ┌────────────┐ ┌─────────────────┐  │
│  │ chat /    │ │ plugin    │ │ workflow   │ │ trace replay /  │  │
│  │ session   │ │ market    │ │ canvas     │ │ debugger        │  │
│  └───────────┘ └───────────┘ └────────────┘ └─────────────────┘  │
│     every panel is an attachable plugin, reached only through    │
│     the UI-Bridge                                                │
└─────────────────────────────┬────────────────────────────────────┘
                              │  FFI (flutter_rust_bridge) or gRPC
┌─────────────────────────────┴────────────────────────────────────┐
│                    Rust kernel (Cordis-RS)              ✅ boots  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ Scope · ServiceRegistry · EventBus · LifecycleManager      │  │
│  │ PluginLoader · TraceCollector                              │  │
│  └────────────────────────────────────────────────────────────┘  │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌─────────┐ ┌──────────────┐  │
│  │ model  │ │ tool   │ │ skill  │ │ sandbox │ │ agent-loop   │  │
│  └────────┘ └────────┘ └────────┘ └─────────┘ └──────────────┘  │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐                  │
│  │ storage│ │ sched. │ │ tracer │ │ transport│    ⬜ none built  │
│  └────────┘ └────────┘ └────────┘ └──────────┘                  │
└─────────────────────────────┬────────────────────────────────────┘
                              │  process isolation / gRPC
┌─────────────────────────────┴────────────────────────────────────┐
│                Sandbox layer (execution isolation)      ⬜ planned │
│  WASM micro-sandbox (tools, skills) │ OS sandbox (seccomp, etc.)  │
└──────────────────────────────────────────────────────────────────┘
```

Only the middle box exists. The two example plugins (`model-echo`, `tool-bash`) are linked
statically into the boot example and their `activate` is a no-op.

---

## 3. The Rust kernel (Cordis-RS)

### 3.1 Responsibilities

The kernel's only job is making plugins safely composable. It holds no agent business logic —
no prompt assembly, no tool dispatch policy, no model selection.

| Subsystem | Module | Role | Status |
|-----------|--------|------|--------|
| `Scope` | `scope.rs` | Nested isolation container; `Arc<Scope>`, cascade drop | ✅ nesting, 🟡 teardown |
| `ServiceRegistry` | `registry.rs` | `TypeId`-keyed service lookup (`anymap2`), values stored as `Arc<T>` | ✅ |
| `EventBus` | `event_bus.rs` | Pub/sub over `tokio::sync::broadcast`, 1 024-slot buffer | ✅ transport, 🟡 trace wiring |
| `LifecycleManager` | `lifecycle.rs` | load / activate / deactivate / hot swap | ✅ activate, 🟡 deactivate |
| `PluginLoader` | `loader.rs` | Instantiate a `Plugin` from a dylib or WASM module | 🟡 signatures only |
| `TraceCollector` | `trace.rs` | Append-only entry log plus snapshot | ✅ |

`Kernel` in `lib.rs` is the aggregate: it owns one `Arc` of each subsystem and is `Clone`, so
handing the whole kernel to a plugin or to the UI bridge is cheap.

### 3.2 The plugin contract

```rust
/// Compile-time contract every plugin implements.
pub trait Plugin: Send + Sync {
    /// Identity, version, dependencies, declared capabilities.
    fn manifest(&self) -> &Manifest;

    /// Register services, subscribe to events, declare side effects.
    fn activate(&self, ctx: &mut PluginContext) -> anyhow::Result<()>;

    /// Reverse those registrations. Scope drop is meant to be the backstop,
    /// not the primary path — see the caveat below.
    fn deactivate(&self, _ctx: &mut PluginContext) -> anyhow::Result<()> { Ok(()) }
}

/// Kernel capabilities handed to a plugin. Shared references: the registry and
/// bus are internally synchronised (`RwLock`), so activation needs no `&mut`.
pub struct PluginContext<'a> {
    pub scope: &'a Scope,
    pub services: &'a ServiceRegistry,
    pub events: &'a EventBus,
    pub tracing: &'a TraceCollector,
}
```

`Capability` is a closed enum (`ModelProvider`, `Tool`, `Skill`, `Sandbox`, `AgentLoop`,
`Storage`, `Scheduler`, `Tracer`, `Ui`) used for dependency resolution and conflict detection.
`Manifest.dependencies` is declared but never read — there is no DAG solver, so load order is
whatever the caller chooses. 🟡

> **Caveat worth stating plainly.** `LifecycleManager::deactivate(id)` removes the plugin from
> the active table and records a trace entry, but does **not** call `Plugin::deactivate`, and
> nothing registers resources into a `Scope`. So neither teardown mechanism runs today:
> anything a plugin registers in `activate` lives until the process exits. "Side effects are
> revoked automatically" is the design, not present behavior.

### 3.3 Hot swap

The intended mechanism, replacing Cordis's TS reflection:

1. A plugin ships as a dynamic library or WASM module exporting
   `extern "C" fn create_plugin() -> Box<dyn Plugin>`.
2. Loading it creates a dedicated child `Scope`.
3. Unloading drops that `Scope`, releasing service handles, subscriptions, and open
   files/sockets by RAII, then `dlclose`s the library.

Both `PluginLoader::load_from_dylib` and `load_from_wasm` currently `bail!` with a pointer back
to this section; `libloading` and `wasmtime` are not yet dependencies. 🟡

Two problems have to be solved before dylib loading is sound, and neither is solved here:

- **ABI stability.** `Box<dyn Plugin>` has no stable layout across compiler versions or crate
  versions. Passing it over an `extern "C"` boundary is unsound unless plugin and kernel are
  built by the same toolchain against the same kernel crate. The realistic fixes are a
  C-compatible vtable (e.g. `abi_stable` / `stabby`) or a version handshake that refuses
  mismatched plugins. Until then, treat dylib loading as same-build-only.
- **Unload safety.** `dlclose` while any code, data, or spawned task from the library is still
  reachable is undefined behavior. This needs the loader to keep the `Library` alive at least
  as long as the plugin object, and to join or cancel tasks the plugin spawned.

WASM has neither problem — the boundary is already defined and instances are isolated — at the
cost of a narrower host interface. That asymmetry is the main argument for making WASM the
default plugin format and dylibs the escape hatch for native performance.

### 3.4 The four operating modes ⬜

Modes are plugin sets, not kernel features; the composition is what varies.

| Mode | Plugin composition |
|------|--------------------|
| Standard | `agent-loop-standard` + `tool-fs` + `tool-shell` + `tool-web` + `skill-*` |
| PTC (programmatic tool calling) | `code-mode-sdk` — a Rust/WASM program composes multi-step tool calls |
| Minimal | `tool-bash` + `tool-str-replace` only; a near-bare baseline for benchmarking |
| Creative | Compose and reload plugins live, then save the set as a named agent preset |

None of these plugins exist yet.

---

## 4. Plugin categories and contracts

Domain traits specialise `Plugin`. **None of the sub-traits below are in the tree yet** — they
are the proposed shapes. 🟡

### 4.1 Model plugins

```rust
pub trait ModelProvider: Plugin {
    fn chat(&self, req: ChatRequest) -> AsyncStream<ChatChunk>;
    fn supports(&self, cap: Capability) -> bool;
}
```

Providers normalise to one internal `ChatRequest`/`ChatChunk` pair, which is what keeps the
kernel model-neutral. DSH ships ~40 provider bindings; superapp ships `model-echo`, a stub that
implements `Plugin` and nothing else. Porting the provider set is roadmap item 6.

Design note: streaming makes the return type the hard part. `AsyncStream<ChatChunk>` above is
shorthand — the concrete choice is between a boxed `Stream` (object-safe, allocates) and an
associated type (zero-cost, breaks `dyn ModelProvider`). Since the registry stores plugins as
trait objects, the boxed form is the likely answer.

### 4.2 Tool plugins

```rust
pub trait Tool: Plugin {
    fn schema(&self) -> ToolSchema;                                   // JSON Schema
    fn invoke(&self, args: Value, sandbox: &Sandbox) -> Result<Value>;
}
```

Taking `&Sandbox` as a parameter rather than letting the tool reach for the filesystem is
deliberate: a tool cannot execute unsandboxed by forgetting to opt in.

### 4.3 Skill, session, sandbox, storage, scheduler, and tracer plugins

Each gets its own trait and its own artifact, so implementations swap freely —
`sandbox-wasm` ↔ `sandbox-nspawn` with no kernel change.

---

## 5. Sandboxing (two tracks) ⬜

| Track | Used for | Mechanism |
|-------|----------|-----------|
| WASM micro-sandbox | tool and skill code | `wasmtime` + capability grants (filesystem/network allowlists) |
| OS-level sandbox | shell and heavyweight commands | Linux `seccomp-bpf` + namespaces + Landlock; `sandbox-exec` on macOS, AppContainer on Windows |

This covers DSH's guarantee that an agent's file and command operations stay inside their
authorised boundary. Neither track is implemented: `tool-bash` is a no-op stub, so nothing is
confined right now. Until the OS track lands, any real tool plugin inherits the full privileges
of the host process — which is the reason not to point this kernel at untrusted input yet.

---

## 6. Flutter plugin UI ⬜

Where DSH's web UI is built in and started with `npx @deepseek-ai/dsh web`, superapp plans a
**Flutter shell** in which interface surfaces are attachable plugins — chat and trace-replay
sit on the same footing as a third-party dashboard.

```dart
abstract class UiPlugin {
  String get id;
  Widget build(BuildContext ctx, KernelBridge bridge);
  void onAttach(KernelBridge bridge);
  void onDetach();
}
```

The bridge talks to the Rust kernel via `flutter_rust_bridge` (code-generated FFI, the low-
ceremony path) or hand-authored `dart:ffi`, or over gRPC for out-of-process layouts. The kernel
pushes state changes back through `EventBus`; UI plugins subscribe to the event stream and
react.

**Planned built-in UI plugins:**

- Chat / session view (message list, tool-call visualisations)
- Plugin market (browse, install, remove kernel and UI plugins)
- Workflow canvas (visual agent-loop and tool-chain editor)
- Trace replay (read the `TraceCollector` log, scrub through timeline, fork)
- Settings / secret manager

The same Flutter tree compiles to Windows, macOS, Linux, Android, and iOS. That's the headline
reason for picking Flutter: a desktop harness and a mobile app from one codebase, where a web
UI would need separate webview/electron packaging for native use.

**None of the above exists yet.** There is no `ui/` directory, no bridge code, no
`pubspec.yaml`. Roadmap item 5 is building the skeleton.

---

## 7. End-to-end traceability 🟡

The kernel's `TraceCollector` is meant to record every system prompt, thought, tool call,
result, subagent spawn, and context injection as an immutable event stream. A Flutter trace-
replay plugin consumes that stream and drives a timeline scrubber, breakpoint-resume, and
branch-fork controls.

What exists: `TraceCollector` logs `PluginLoaded`, `PluginUnloaded`, and `Checkpoint` entries.
What's missing: `EventBus::publish` does not write to the trace. The boot example fires
`Event::Thought`, yet the trace ends up with three entries (two plugin loads, one checkpoint),
not four. Closing that loop is roadmap item 1.

---

## 8. Repository layout

```
superapp/
├── Cargo.toml              # workspace root: kernel + plugin crates
├── rust-toolchain.toml     # pinned toolchain (currently stable)
├── ARCHITECTURE.md         # this document
├── README.md               # status and quick start
├── kernel/                 # Rust kernel, crate `cordis-rs`, lib `cordis_rs`
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs          # `Kernel` aggregate
│   │   ├── plugin.rs       # `Plugin` trait, `Manifest`, `Capability`, `PluginContext`
│   │   ├── scope.rs        # `Scope` — nested container, RAII teardown ✅🟡
│   │   ├── registry.rs     # `ServiceRegistry` — `TypeId` → `Arc<T>` ✅
│   │   ├── event_bus.rs    # `EventBus` — tokio broadcast pub/sub ✅
│   │   ├── lifecycle.rs    # `LifecycleManager` — activate / deactivate ✅🟡
│   │   ├── loader.rs       # `PluginLoader` — dylib / WASM stubs 🟡
│   │   └── trace.rs        # `TraceCollector` — append-only log ✅
│   └── examples/boot.rs    # end-to-end boot example ✅
├── plugins/
│   ├── model-echo/         # example model plugin (`model.echo`)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs      # `EchoModel` — implements `Plugin`, activate is a no-op ✅
│   └── tool-bash/          # example tool plugin (`tool.bash`)
│       ├── Cargo.toml
│       └── src/lib.rs      # `BashTool` — implements `Plugin`, activate is a no-op ✅
└── ui/                     # ⬜ Flutter workspace — does not exist yet
    ├── pubspec.yaml
    ├── lib/
    │   ├── bridge/         # flutter_rust_bridge codegen output
    │   ├── ui_plugins/     # Flutter UI plugins
    │   └── core/
    └── proto/              # ⬜ FFI / gRPC contracts if the bridge needs explicit schemas
```

Total Rust LOC today (kernel + two plugin stubs): **462 lines**. The budget is ~3 000; most of
the headroom is for dependency resolution, scope-resource tracking, dylib/WASM loading, and the
trait impls for domain sub-traits once those are added.

---

## 9. Capability parity with DSH

| Feature | DSH (Node.js + web UI) | superapp (Rust + Flutter) |
|---------|------------------------|---------------------------|
| Everything is a plugin | ✅ | ✅ contract, 🟡 dylib/WASM loading |
| Hot swap | Cordis (TS reflection) | Cordis-RS (trait + Scope/Drop) 🟡 |
| Model neutral | ~40 providers | ~40 planned, **1 stub today** |
| Lightweight kernel | 2 700+ lines TS | 460 lines Rust today, 3 000 budgeted |
| Secure sandbox | OS-level (Linux) | WASM + OS dual-track ⬜ |
| End-to-end trace | ✅ | ✅ infra, 🟡 EventBus wiring |
| Four operating modes | ✅ | ⬜ plugin sets not built |
| Interface | built-in web (port 3030) | Flutter plugin UI ⬜ |
| Launch command | `npx @deepseek-ai/dsh web` | native binary / mobile app ⬜ |

The mechanics are in place for the top three; most of the rest is planned.

---

## 10. Why Rust: expected performance envelope

Seven dimensions where swapping Node.js for Rust is expected to shift the performance
characteristics. Dimensions marked ⚡ are the ones where Rust wins outright; the rest are more
situational.

### 10.1 Startup time ⚡

| | Node.js (DSH) | Rust (Cordis-RS) |
|-|---------------|------------------|
| Runtime boot | V8 cold start + JIT warmup, ~150–400 ms | native binary, no VM, ~5–20 ms |
| Plugin load | parse npm package + TS decorator reflection | `dlopen` or WASM instantiation, no reflection overhead |
| Overall cold start | hundreds of ms (higher if `npx` has to fetch) | tens of ms |

**Implication:** Rust cold start is **10–50× faster**. Most visible for the minimal mode
(lightweight benchmarking) and creative mode (frequent reload).

### 10.2 Resident memory ⚡

| | Node.js | Rust |
|-|---------|------|
| Runtime baseline | V8 heap + JIT working set, 50–150 MB resident | kernel + plugins only, 5–20 MB |
| GC behavior | periodic stop-the-world pauses, proportional to heap size | compile-time ownership, no GC, no pauses |
| Long sessions / large contexts | context growth → heap growth → GC churn | stack/heap precisely controlled, zero-copy borrows possible |

**Implication:** Rust holds **5–10× less** memory at rest. In long-running tasks, Node's GC
introduces latency spikes; Rust's response curve stays flat.

### 10.3 Concurrency and multi-agent orchestration ⚡

| | Node.js | Rust |
|-|---------|------|
| Parallelism model | single-threaded event loop + async (V8 single-thread bound) | `tokio` M:N scheduler, true parallelism across cores |
| Subagent parallelism | constrained by event-loop throughput; CPU-heavy tools starve the loop | multi-core linear scaling, CPU/IO tasks isolated |
| Concurrent model calls | single-thread scheduling ceiling | scales linearly to available cores |

**Implication:** Standard-mode workloads with "subagent + tool parallelism" scale to N cores
under Rust; Node saturates one core. CPU-intensive tool calls (large file parsing, compression)
block the event loop in Node; Rust isolates them. **Speedup on an N-core machine can reach N×.**

### 10.4 Hot-swap determinism ⚡ (once implemented)

| | Node.js (Cordis) | Rust (Cordis-RS) |
|-|------------------|------------------|
| Side-effect teardown | relies on cleanup callbacks registered via TS decorators | RAII `Drop`, enforced at compile time |
| Dangling-reference risk | can leak at runtime if de-registration is forgotten | borrow checker rules it out statically |
| Hot-replace latency | unload + reload module graph, involves GC | `Scope` drop + `dlclose`, deterministic |

**Implication:** Rust turns "safe hot swap" from a **runtime convention** into a **compile-time
guarantee**. No leaks, no unpredictable pauses.

### 10.5 Tool execution ⚡

- **Compute-bound tools** (JSON/code parsing, embedding, compression): Rust is **5–50× faster**
  than Node (no JIT warmup, no GC interruption).
- **IO-bound tools** (file reads, network): roughly tied, but Rust has no event-loop bottleneck
  so tail latency is lower.
- **PTC mode** (programmatic multi-step tool composition): if the composition logic runs in WASM
  instead of interpreted TS, Rust/WASM approaches native speed; Node stays interpreted.

### 10.6 Sandbox overhead (once implemented)

| | Node.js (DSH) | Rust (Cordis-RS) |
|-|---------------|------------------|
| Sandbox mechanism | OS-level (orthogonal to runtime) | WASM micro-sandbox + OS-level |
| WASM tool bridge | requires extra shim to Node | `wasmtime` embedded natively, near-zero bridge cost |
| Context switch | process/container-level, heavyweight | WASM instance, microseconds |

**Implication:** Rust's in-process `wasmtime` makes tool-level micro-sandboxing practically
free. Node's external WASM integration is heavier.

### 10.7 Power and deployment footprint ⚡

| | Node.js | Rust |
|-|---------|------|
| Mobile / edge | needs embedded V8, large binary, high power draw | single native binary, small, efficient |
| Mobile app integration | requires bundled JS engine | direct FFI, no extra runtime |

**Implication:** Flutter + Rust packages as a self-contained mobile app with no JS engine tax.
Node's binary size and power overhead on mobile are deal-breakers for long-running or battery-
constrained use.

### 10.8 Summary table

| Dimension | Rust improvement over Node |
|-----------|----------------------------|
| Cold start | 10–50× faster |
| Resident memory | 5–10× smaller |
| Multi-agent parallelism | N× on N cores (Node: ~1×) |
| Compute-heavy tools | 5–50× faster |
| Hot-swap determinism | compile-time guarantee vs runtime convention |
| Mobile deployment | native-capable vs needs embedded engine |

### 10.9 When the difference doesn't matter

- **Bottleneck is LLM network latency.** If the model takes 1–10 seconds to respond, kernel
  speed is drowned out.
- **Tools are all external API calls.** IO wait dominates; kernel language is irrelevant.
- **Single agent, low interaction rate.** Concurrency advantage never shows up.

### 10.10 One-sentence summary

Rust delivers order-of-magnitude wins in **startup, memory, concurrency, compute-bound tools,
and mobile deployment**, and promotes hot-swap safety from a runtime discipline to a compile-
time invariant. But when the agent is **network/model-latency bound**, kernel performance has
limited impact on perceived responsiveness. The triple combination of *creative mode (frequent
reload) + multi-agent parallelism + mobile targets* is where Rust + Flutter pays off most.

---

## 11. Open design questions

These are unresolved choices, not implementation gaps.

1. **ABI stability for dylib plugins.** `Box<dyn Plugin>` over `extern "C"` is undefined
   behavior unless both sides are built by the same toolchain. Solutions: enforce a version
   handshake and reject mismatched builds (pragmatic, limits reuse); adopt a stable-ABI crate
   like `abi_stable` (heavier, more portable); or make WASM the default and treat dylibs as
   same-build-only (the current lean).

2. **Async trait shape for `ModelProvider::chat`.** The return type is an async stream.
   Concrete choice: boxed `Stream` trait object (object-safe, runtime cost) or associated type
   (zero-cost, breaks `dyn ModelProvider`). Since the registry stores plugins as trait objects,
   boxing is likely.

3. **EventBus → TraceCollector wiring.** Should `EventBus::publish` call `TraceCollector`
   directly (tight coupling, simple), or should the trace collector subscribe as a listener
   (loose coupling, one more indirection)? Listener is architecturally cleaner, but synchronous
   recording is easier to reason about.

4. **Scope-resource tracking.** How does a plugin declare "this handle belongs to scope S, drop
   it when S is dropped"? Two paths: wrap every resource in a scope-aware guard (boilerplate),
   or have the scope hold a `Vec<Box<dyn Drop>>` and let plugins push disposers (type-erased,
   simpler). Neither is sketched yet.

5. **Dependency resolution.** Should the lifecycle manager topologically sort plugin loads by
   `Manifest.dependencies`, or leave that to the caller? Sorting is safer but means the kernel
   needs a DAG solver; manual ordering is flexible but error-prone.

6. **Bridge protocol (Rust ↔ Flutter).** `flutter_rust_bridge` generates the FFI automatically
   from annotated Rust signatures (low ceremony, opaque wire format). Hand-authored `dart:ffi`
   gives full control but is verbose. gRPC over localhost decouples the processes but adds
   latency. The decision affects mobile packaging: FFI and `flutter_rust_bridge` let the kernel
   link into the app; gRPC requires a sidecar or background service.

None of these block the next roadmap items, so they're deferred until the decision has to be
made.

---

**Document status:** This is a design spec and a roadmap, not a user manual. The kernel boots
and the contracts compile; most behavior is planned. See [`README.md`](./README.md) for the
current implementation state and known gaps.
