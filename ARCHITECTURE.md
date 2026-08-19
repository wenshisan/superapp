# Architecture — a pluggable agent framework (Rust kernel + Flutter plugin UI)

> Two ideas are borrowed from **DeepSeek Harness (DSH)** and its **Cordis** runtime:
> *everything is a plugin*, and *plugins compose across space and time*. superapp keeps both
> and swaps the substrate: the runtime moves from **Node.js to Rust**, and the interface moves
> from a built-in web app to a **Flutter shell in which the UI itself is a plugin**.
>
> **Flutter is the UI from day one.** This gives us native desktop binaries and mobile apps from
> the same codebase, with UI surfaces (chat, plugin market, trace viewer) as attachable plugins
> rather than built-in views.
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

The paradigm is DSH's; the mechanics are what Rust makes natural. superapp adds one thing DSH
does not have: **the user creates plugins by asking, and every ask becomes permanent software.**

### The core inversion

A conventional agent app treats each request as disposable. You ask, it answers, the context is
gone; ask the same thing tomorrow and the model does the work again. superapp treats each request
as a **build instruction**. The ask bubble generates a plugin, the plugin lands in the left
navigation, and it stays there — clickable, editable, composable with everything else the user
has built.

| | Conventional agent chat | superapp |
|-|-------------------------|----------|
| What an ask produces | an answer | a plugin |
| Where it lives afterward | conversation scrollback | left navigation, persisted |
| Repeat use | re-ask, model re-runs | click the nav entry, no model call |
| Cost of the second use | another inference | zero |
| Composition | copy-paste between chats | plugins call plugins |

The practical consequence: **the model is used to build features, not to answer questions
repeatedly.** A user who asks "summarise today's traces by cost" once gets a nav entry they click
every morning for free. That's the whole bet — inference builds the tool, then the tool runs
without inference.

This is why the plugin system is the kernel's only job and why the UI is registry-driven rather
than designed. The set of features the app has is not fixed at build time; it's whatever its
users have asked for.

| Dimension | DSH (Node.js + web UI) | superapp (Rust kernel + Flutter UI) |
|-----------|------------------------|-------------------------------------|
| Kernel language | TypeScript, ~2 700 lines | Rust — ~460 lines today, ~3 000 budgeted |
| Plugin artifact | npm package + TS decorators | Static crate today; dynamic library (`.so`/`.dylib`/`.dll`) or WASM module planned |
| Hot-swap engine | Cordis, via TS reflection | **Cordis-RS**, via trait objects + scoped lifetimes |
| UI | built-in web app on port 3030 | **Flutter shell, built first**, with attachable UI plugins ⬜ |
| Sandbox | OS-level (Linux) | `seccomp`/namespaces **and** a WASM micro-sandbox ⬜ |
| Model bindings | ~40 providers | providers are plugins; **one echo stub exists so far** |
| Who writes plugins | developers, ahead of time | **developers and end users, via the ask bubble** ⬜ |
| What an ask produces | a chat answer | **a plugin with a nav entry** ⬜ |

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
│  ┌─────────┬──────────────────────────────────────────────────┐  │
│  │  LEFT   │  active plugin surface                           │  │
│  │  NAV    │                                                  │  │
│  │         │  ┌───────────┐ ┌───────────┐ ┌────────────────┐  │  │
│  │ built-in│  │ trace     │ │ plugin    │ │ user-generated │  │  │
│  │ ────────│  │ viewer    │ │ list      │ │ custom views   │  │  │
│  │ installd│  └───────────┘ └───────────┘ └────────────────┘  │  │
│  │ ────────│                                                  │  │
│  │ custom  │                          ╭──────────╮            │  │
│  │  ↑ from │                          │ ask ◉    │ ← floating │  │
│  │  asks   │                          ╰──────────╯   bubble   │  │
│  └─────────┴──────────────────────────────────────────────────┘  │
│   nav entries ARE the plugin registry; the bubble WRITES to it    │
│   every panel is an attachable plugin, reached through the bridge │
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

## 6. Flutter plugin UI ⬜ — the starting point

Where DSH's web UI is built in and started with `npx @deepseek-ai/dsh web`, superapp uses a
**Flutter shell** in which interface surfaces are attachable plugins — chat and trace-replay
sit on the same footing as a third-party dashboard.

**Flutter is the first thing to build, not the last.** The kernel boots but produces nothing a
person can look at, so the trace log and event stream are only inspectable from an example
binary. A minimal Flutter shell reading the kernel's trace makes every subsequent kernel change
observable, which is why the UI moves ahead of dependency resolution and dylib loading in the
roadmap.

### 6.1 The interaction model: ask → plugin

**Every user ask creates a plugin.** When the user types a request into the floating ask bubble,
the system generates a plugin to fulfill it — a new tool, a dashboard panel, a workflow
automation — and registers that plugin into the left navigation as a permanent entry. The user
is not calling an AI assistant to fix a transient problem; they are **building the application
itself** by describing what they need, and the result is installed software.

This inverts the usual agent chat pattern. Standard chat agents answer once, then the context
evaporates. Here, every answer is a plugin artifact: it has an entry point, persists across
sessions, can be edited or removed, and composes with other plugins the user has created. The
left navigation is the user's command palette, and every item in it came from an ask.

**Example flow:**
1. User types in the ask bubble: "Show me the most expensive API calls from the trace."
2. System generates a `UiPlugin` that queries `TraceCollector`, aggregates by cost, and renders
   a sorted table.
3. The plugin is registered with ID `expensive-calls`, and its name appears in the left
   navigation under a "Custom Views" section.
4. Next session, the entry is still there. Clicking it renders the same view. The user can edit
   the plugin's prompt to refine the query, or delete it if no longer needed.

The ask bubble is not a chatbot. It is the **plugin factory UI**. The left navigation is not a
static menu designed at build time; it is the **user's plugin collection**.

### 6.2 The UI plugin contract

```dart
abstract class UiPlugin {
  String get id;
  String get label;                      // shown in left navigation
  Widget build(BuildContext ctx, KernelBridge bridge);
  void onAttach(KernelBridge bridge);
  void onDetach();
}
```

The shell holds a registry of `UiPlugin`s and renders whichever surfaces are attached. Nothing
in the shell knows about chat specifically — the chat view is a plugin registered at startup,
same as one installed later from the plugin market or generated from an ask.

The left navigation is divided into sections:

- **Built-in** (shipped with the app): trace viewer, plugin list, settings
- **Installed** (from the plugin market or loaded from disk)
- **Custom** (generated from user asks, persisted in the user's plugin directory)

Every section is just a filter over the same plugin registry. The shell doesn't hard-code which
plugins exist; it asks the registry and renders them.

### 6.3 Bridge: `flutter_rust_bridge`, in-process

The decision from open question 6 is settled for the first milestone: **`flutter_rust_bridge`
with the kernel linked in-process**. Rationale — it code-generates the FFI from annotated Rust
signatures (no hand-written `dart:ffi` marshalling), it supports streams so `EventBus` maps onto
a Dart `Stream` naturally, and an in-process kernel is the only shape that packages cleanly into
a mobile app. gRPC stays available for out-of-process or remote-kernel layouts later; it is not
the default.

Surface the bridge needs to expose for the first milestone:

| Direction | Call | Maps to |
|-----------|------|---------|
| Dart → Rust | `bootKernel()` | `Kernel::new` + activate the statically linked plugin set |
| Dart → Rust | `listPlugins()` | `LifecycleManager` active table |
| Dart → Rust | `traceSnapshot()` | `TraceCollector::snapshot` |
| Dart → Rust | `generatePlugin(ask)` | ask → plugin pipeline (see §6.4) |
| Dart → Rust | `installPlugin(spec)` | persist to the user plugin dir + `LifecycleManager::activate` |
| Dart → Rust | `removePlugin(id)` | `deactivate` + delete from user plugin dir |
| Rust → Dart | `eventStream()` | `EventBus` subscription as a Dart `Stream<Event>` |

`Event` and trace entries need `serde`-serialisable shapes so codegen can mirror them in Dart.
Today they are plain Rust enums with no derive — that is the first concrete kernel change the UI
work forces.

### 6.4 The ask → plugin pipeline

This is the app's core mechanism, so it gets its own contract. The pipeline turns natural
language into a registered, persisted plugin.

```
ask bubble  →  ModelProvider  →  PluginSpec  →  validate  →  persist  →  activate  →  nav entry
   text         (generation)      (artifact)     (compile/     (user      (Lifecycle-    (UI
                                                  schema)      plugin      Manager)      registry)
                                                               dir)
```

`PluginSpec` is the generated artifact — the thing the model produces and the kernel installs:

```rust
pub struct PluginSpec {
    pub manifest: Manifest,        // id, label, version, capability, dependencies
    pub kind: PluginKind,          // what runtime executes this
    pub source: String,            // the generated code or declarative config
    pub ask: String,               // the original user request, kept for edit/regenerate
}

pub enum PluginKind {
    /// Dart widget code, run in the Flutter shell. Covers custom views and panels.
    UiWidget,
    /// WASM module compiled from generated Rust/other. Covers tools and computation.
    Wasm,
    /// Declarative: a composition of existing plugins, no new code.
    /// Cheapest and safest path — prefer it when the ask can be satisfied by wiring
    /// together plugins that already exist.
    Composition { steps: Vec<StepSpec> },
}
```

`PluginKind::Composition` matters more than it looks. Many asks ("show me X filtered by Y",
"run tool A then tool B") need no new code at all — only a wiring of registered plugins. Trying
composition before generating code makes the common case fast, deterministic, and free of the
sandbox questions that generated code raises.

Three problems this pipeline has to solve, none of them solved yet:

- **Generated code is untrusted code.** A plugin the model wrote has the same standing as a
  plugin downloaded from a stranger. This is the concrete reason the WASM sandbox (§5) is not
  optional — `PluginKind::Wasm` must run confined, and `UiWidget` needs its own answer since
  Dart widgets execute in the shell's process. Interpreting a constrained widget DSL rather than
  compiling arbitrary Dart is the likely path.
- **Validation before install.** A plugin that fails to compile, or whose manifest collides with
  an existing ID, must be rejected before it reaches the navigation — a broken nav entry is worse
  than a failed ask. Validation runs in the pipeline, not at first click.
- **Edit and regenerate.** `PluginSpec.ask` is retained so the user can refine the original
  request and regenerate rather than starting over. Whether regeneration replaces the plugin in
  place or creates a version alongside it is an open question (§11.7).

### 6.5 Build order

1. **Shell + left navigation + trace viewer.** `ui/` workspace, `flutter_rust_bridge` wired to
   `traceSnapshot()` and `eventStream()`, nav driven by the plugin registry, one read-only trace
   panel. Proves the bridge and the registry-driven nav.
2. **Plugin list panel.** Read `listPlugins()`, show manifest, capability, and active state.
   Together with step 1 this makes the plugin system visible before anything generates plugins.
3. **Ask bubble, composition only.** The floating bubble, wired to `generatePlugin()` restricted
   to `PluginKind::Composition`. No code generation, no sandbox needed — proves the full
   ask → spec → install → nav-entry loop against the safest plugin kind. Needs a real
   `ModelProvider`, so it lands alongside the first non-stub model plugin.
4. **Generated `UiWidget` plugins.** Constrained widget DSL plus validation. This is where the
   untrusted-code question has to be answered rather than deferred.
5. **Generated `Wasm` plugins.** Requires the WASM sandbox from §5.
6. **Plugin market, workflow canvas, trace replay with scrub/fork, settings and secret
   manager.** Each is a `UiPlugin`; none require shell changes.

Desktop (Windows/macOS/Linux) is the target for steps 1–3; Android and iOS come from the same
tree once the kernel builds for those triples. That single-codebase reach is the headline reason
for picking Flutter over a web UI, which would need webview or Electron packaging to run native.

**None of the above exists yet.** There is no `ui/` directory, no bridge code, no
`pubspec.yaml`. This is now roadmap item 1.

---

## 7. End-to-end traceability 🟡

The kernel's `TraceCollector` is meant to record every system prompt, thought, tool call,
result, subagent spawn, and context injection as an immutable event stream. **The Flutter trace
viewer plugin consumes that stream** and drives a timeline scrubber, breakpoint-resume, and
branch-fork controls — which is why building the UI comes before building deep agent features.
An inspector for the trace makes kernel development observable.

What exists: `TraceCollector` logs `PluginLoaded`, `PluginUnloaded`, and `Checkpoint` entries.
What's missing: `EventBus::publish` does not write to the trace. The boot example fires
`Event::Thought`, yet the trace ends up with three entries (two plugin loads, one checkpoint),
not four. Closing that loop is prerequisite to the trace viewer being useful.

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
├── bridge/                 # ⬜ crate `cordis-bridge` — flutter_rust_bridge API surface
│   ├── Cargo.toml          #    depends on cordis-rs; cdylib + staticlib targets
│   └── src/api.rs          #    annotated fns: bootKernel, listPlugins, traceSnapshot,
│                           #    eventStream
└── ui/                     # ⬜ Flutter workspace — does not exist yet
    ├── pubspec.yaml
    ├── lib/
    │   ├── bridge/         # flutter_rust_bridge codegen output (generated, checked in)
    │   ├── shell/          # app shell: UiPlugin registry, layout, routing
    │   ├── ui_plugins/     # trace_viewer/, plugin_list/, chat/ …
    │   └── core/           # shared models, theme
    ├── windows/ macos/ linux/ android/ ios/    # platform runners
    └── flutter_rust_bridge.yaml                # codegen config
```

The `bridge/` crate is deliberately separate from `kernel/`: the kernel stays a plain Rust
library with no FFI annotations or `cdylib` target, and the bridge crate owns everything
Flutter-specific. Swapping the UI layer, or adding a second one, means touching one crate.

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
| Interface | built-in web (port 3030) | **Flutter shell — first milestone** ⬜ |
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

6. **Bridge protocol (Rust ↔ Flutter).** Decision made: **`flutter_rust_bridge` in-process, FFI
   direct**. It generates the FFI from annotated Rust, supports streams (so `EventBus` maps to
   `Stream<Event>`), and lets the kernel link into the mobile app. gRPC over localhost is
   available for out-of-process layouts but is not the default — it adds latency and complicates
   mobile packaging.

7. **Edit and regenerate for user-generated plugins.** `PluginSpec.ask` is retained so the user
   can refine the original request and regenerate. Should regeneration replace the plugin in
   place (same ID, same nav entry, breaks anyone depending on the old version) or create a
   version alongside it (safe, but nav clutter if the old version is never removed)? In-place
   feels right for single-user custom views; versioning feels right for plugins other people
   depend on. The distinction might be whether the plugin has dependents.

8. **Sandboxing generated Dart widget code.** `PluginKind::Wasm` runs in the WASM sandbox,
   `PluginKind::Composition` is just wiring (no untrusted code). But `PluginKind::UiWidget`
   compiles Dart and runs it in the shell's process — same privileges as the rest of the UI.
   The safest path is not compiling arbitrary Dart at all; instead, interpret a constrained
   widget DSL (a declarative JSON-like shape that maps to a safe subset of Flutter widgets) or
   compile to WASM and render through a host widget bridge. The latter has Flutter-for-WASM
   challenges; the former needs a DSL design. This blocks step 4 of the build order (§6.5).

None of these block the next roadmap items, so they're deferred until the decision has to be
made.

---

**Document status:** This is a design spec and a roadmap, not a user manual. The kernel boots
and the contracts compile; most behavior is planned. See [`README.md`](./README.md) for the
current implementation state and known gaps.
