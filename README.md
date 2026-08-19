# superapp — a pluggable agent framework (Rust kernel + Flutter plugin UI)

A general-purpose agent framework built on two ideas borrowed from DeepSeek Harness (DSH)
and its Cordis runtime: **everything is a plugin**, and **plugins compose across space and
time**. Where DSH runs a Node.js kernel driven by TypeScript decorator reflection, superapp
runs a **Rust** kernel (`cordis-rs`) with a **Flutter** UI in which the interface itself is
just another plugin.

**The core mechanism: every user ask creates a plugin.** This is not a chat app where you ask
and the model answers once. It's a **build-your-own-software app** where you ask and the model
generates a plugin — a custom view, a tool, a workflow automation — that lands in the left
navigation as a permanent entry. Click it again tomorrow, it runs without calling the model.
The model is used to **build features**, not to answer questions repeatedly.

**Next milestone: the Flutter shell.** The kernel boots but has no face — the trace log is only
visible from an example binary. Building the UI first makes every later kernel change
observable, so `ui/` plus the `flutter_rust_bridge` layer is roadmap item 1.

Full design document: [`ARCHITECTURE.md`](./ARCHITECTURE.md)

> **Status: early skeleton.** The kernel compiles, the boot example runs end to end, and the
> module boundaries are settled. Most subsystems are still contracts rather than working
> implementations — read [Implementation status](#implementation-status) before building on
> this.

## Design philosophy

- **Everything is a plugin.** Models, tools, skills, sandboxes, storage, schedulers, tracers
  and UI surfaces all implement one `Plugin` trait. The kernel holds no agent business logic;
  its only job is making plugins safely composable.
- **The user builds plugins by asking.** Type a request into the floating ask bubble, and the
  system generates a plugin to fulfill it — a dashboard panel, a custom tool, a workflow
  automation. The plugin lands in the left navigation as a permanent entry. This is not a chat
  app where you ask and it answers once; it's a **build-your-own-software app** where every ask
  becomes installed software. The model is used to build features, not to answer questions
  repeatedly.
- **Space/time composability.** *Space:* every plugin registers into its own `Scope`, so
  dropping that scope should cascade-release everything the plugin registered — RAII instead
  of manual teardown. *Time:* `LifecycleManager` owns load / activate / deactivate / hot swap,
  and `TraceCollector` keeps an append-only log so a run can be replayed or forked.

## Repository layout

```
superapp/
├── Cargo.toml                 # workspace root
├── rust-toolchain.toml        # pinned to stable
├── ARCHITECTURE.md            # full design doc
├── kernel/                    # the Rust kernel — crate `cordis-rs`, lib `cordis_rs`
│   ├── src/
│   │   ├── lib.rs             # `Kernel` aggregate: wires up every subsystem
│   │   ├── plugin.rs          # `Plugin` trait, `Manifest`, `Capability`, `PluginContext`
│   │   ├── scope.rs           # `Scope` — nested isolation container (RAII teardown)
│   │   ├── registry.rs        # `ServiceRegistry` — TypeId-keyed service lookup
│   │   ├── event_bus.rs       # `EventBus` — pub/sub over tokio broadcast
│   │   ├── lifecycle.rs       # `LifecycleManager` — activate / deactivate
│   │   ├── loader.rs          # `PluginLoader` — dylib / WASM contracts (stubs)
│   │   └── trace.rs           # `TraceCollector` — append-only trace log
│   └── examples/boot.rs       # minimal end-to-end boot example
├── plugins/
│   ├── model-echo/            # example model plugin (`model.echo`)
│   └── tool-bash/             # example tool plugin (`tool.bash`)
├── bridge/                    # ⬜ crate `cordis-bridge` — flutter_rust_bridge API surface
│   ├── Cargo.toml             #    cdylib + staticlib targets
│   └── src/api.rs             #    bootKernel, listPlugins, traceSnapshot, eventStream
└── ui/                        # ⬜ Flutter workspace — roadmap item 1
    ├── pubspec.yaml
    ├── lib/
    │   ├── bridge/            # flutter_rust_bridge codegen output (generated, checked in)
    │   ├── shell/             # app shell: UiPlugin registry, layout, routing
    │   ├── ui_plugins/        # trace_viewer/, plugin_list/, chat/ …
    │   └── core/              # shared models, theme
    ├── windows/ macos/ linux/ android/ ios/    # platform runners
    └── flutter_rust_bridge.yaml                # codegen config
```

`ui/` (the Flutter workspace) does not exist yet; it's roadmap item 1.

## Quick start

Requires a stable Rust toolchain (`rust-toolchain.toml` pins the channel; rustup will honour
it automatically). No other system dependencies.

```bash
# build the kernel and its examples
cargo build -p cordis-rs --examples

# run the minimal boot example: load two plugins, publish an event,
# checkpoint the trace, then unload
cargo run -p cordis-rs --example boot
```

Expected output:

```
active plugins: ["model.echo", "tool.bash"]
trace entries: 3
after unload: []
```

The three trace entries are `PluginLoaded("model.echo")`, `PluginLoaded("tool.bash")` and
`Checkpoint("boot-done")`. The published `Event::Thought` is *not* among them — see
[Known gaps](#known-gaps).

The build currently emits one warning (`field 'parent' is never read` in `scope.rs`), which is
expected: parent links are stored for future traversal but nothing reads them yet.

## Core concepts — DSH (Cordis/TS) → Cordis-RS (Rust)

| DeepSeek Harness (TS) | Cordis-RS (Rust) | What it is |
|-----------------------|------------------|------------|
| Decorator reflection | `Plugin` trait | Type-safe compile-time contract; no runtime introspection |
| Scope auto-teardown | `Scope` + `Drop` | RAII: dropping a scope releases everything registered into it |
| Event streams | `EventBus` | Pub/sub over `tokio::sync::broadcast` |
| Trace replay | `TraceCollector` | Append-only log of every load / event / checkpoint |
| dylib / WASM plugins | `PluginLoader` | Stubs pointing to `libloading` / `wasmtime` integration |

In DSH, plugins attach metadata with TS decorators and the Cordis runtime reflects over them
at load time. In Cordis-RS, plugins implement the `Plugin` trait directly — contract
enforcement moves from runtime reflection to compile-time trait bounds.

## Implementation status

The kernel builds and the boot example runs, but most subsystems are still minimal contracts:

### Working end to end

- **Module boundaries:** `Kernel`, `Plugin` trait, `Scope`, `ServiceRegistry`, `EventBus`,
  `LifecycleManager`, `TraceCollector`, `PluginLoader` — all present, structurally sound.
- **Load / activate / deactivate:** The lifecycle manager can activate a `Plugin` (call
  `plugin.activate(ctx)`, insert into active table, record to trace) and deactivate by ID
  (remove from table, record to trace). Works for statically linked plugins (the `EchoModel`
  and `BashTool` example plugins both load this way).
- **Trace recording:** `TraceCollector` logs `PluginLoaded`, `PluginUnloaded`, `Checkpoint`.
  Can snapshot the full trace. The boot example verifies this path.
- **Scope nesting:** `Scope::root()` and `Scope::child(...)` both work; child scopes link
  back via `Weak<Scope>` to avoid cycles. Dropping a parent scope drops all children (though
  nothing exercises that yet).
- **Service registry:** `ServiceRegistry::register` and `ServiceRegistry::get` work for
  `Send + Sync + 'static` types, keyed by `TypeId` (using `anymap2`). No plugin actually
  registers a service yet.

### Known gaps

- **EventBus → TraceCollector wiring:** `EventBus::publish` sends to the broadcast channel
  but does *not* write to the trace collector. The boot example publishes an `Event::Thought`,
  yet the trace ends up with only 3 entries (two plugin loads, one checkpoint), not 4. The
  original README claimed "全链路可追溯" (full traceability), but that loop isn't closed yet.
- **PluginLoader dylib / WASM:** Both `load_from_dylib` and `load_from_wasm` return
  `anyhow::bail!` with a message pointing to `ARCHITECTURE.md`. The contract exists (FFI
  signatures, extern "C" constructors in the example plugins), but no implementation.
  `libloading` and `wasmtime` are not in `Cargo.toml` yet.
- **Scope Drop side effects:** `Scope` holds child scopes, but nothing registers disposable
  resources into a scope yet. The RAII teardown path is structurally present but untested.
- **Plugin deactivate callback:** `LifecycleManager::deactivate(id)` removes the plugin from
  the active table and records the trace entry, but does *not* call the plugin's
  `.deactivate(ctx)` method. If a plugin registered services or event subscriptions in
  `.activate(...)`, those live until process exit.
- **Dependency resolution:** `Manifest.dependencies` is declared but never checked. The
  lifecycle manager has no DAG solver.
- **No real agent loop, model provider, tool, sandbox, or UI.** The two example plugins
  (`model-echo`, `tool-bash`) implement `Plugin::activate` as a no-op. They do not register
  services or drive any agent logic. The kernel is ready for them, but none exist yet.
- **The ask → plugin pipeline does not exist.** No ask bubble, no `PluginSpec`, no
  `generatePlugin()`, no plugin persistence directory. The kernel can activate a plugin handed
  to it in code; it cannot yet accept one generated at runtime, validate it, persist it, or
  surface it in a navigation that also doesn't exist. This is the app's core mechanism and it is
  entirely ahead of us — see `ARCHITECTURE.md` §6.4.

In short: the boot path works, the module contracts are in place, but hot-swap, scope
teardown, dylib loading, event → trace recording, the ask → plugin pipeline, and everything
downstream of `activate` are still to be built.

## Roadmap

1. **Build `ui/` as a Flutter workspace** plus `bridge/` as a separate crate. Use
   `flutter_rust_bridge` to expose `bootKernel()`, `listPlugins()`, `traceSnapshot()`, and
   `eventStream()`. Build the shell with a **registry-driven left navigation** (nav entries come
   from the plugin registry, not hard-coded), plus a trace viewer panel and a plugin list panel.
   Desktop (Windows/macOS/Linux) first; Android/iOS from the same tree once the kernel
   cross-compiles. See `ARCHITECTURE.md` §6.
2. **Close the EventBus → TraceCollector loop.** Every `EventBus::publish` should append to
   `TraceCollector` so the trace actually captures agent behavior, not just plugin lifecycle.
3. **Ship the ask bubble with composition-only plugin generation.** The floating ask bubble, a
   real `ModelProvider`, and `generatePlugin()` restricted to `PluginKind::Composition` — asks
   satisfied by wiring existing plugins together, no code generation, no sandbox needed. This
   proves the full ask → spec → install → nav-entry loop against the safest plugin kind. See
   `ARCHITECTURE.md` §6.4.
4. **Implement `PluginLoader`:** add `libloading` and `wasmtime` to `Cargo.toml`, wire up
   `load_from_dylib` and `load_from_wasm`. Test with out-of-tree plugin builds.
5. **Call `.deactivate(...)` on unload.** The lifecycle manager should invoke the plugin's
   deactivate hook, not just remove it from the table. Required before user-generated plugins
   can be removed or regenerated cleanly.
6. **Generated `UiWidget` plugins.** Needs the constrained widget DSL and validation — see open
   question 8 in `ARCHITECTURE.md`. This is where generated code stops being deferrable.
7. **Define domain sub-traits:** `ModelProvider`, `Tool`, `Skill`, `Sandbox` (etc.) as
   specializations of `Plugin`, and migrate the ~120 DSH plugins (or a representative subset)
   into Rust.
8. **Hook up real models** (DeepSeek, OpenAI, Anthropic, etc.) and real tools (bash in a
   proper sandbox, not the current no-op stub).
9. **Add the WASM sandbox and OS-level sandboxing:** `wasmtime` with capability grants for
   generated `PluginKind::Wasm` plugins; seccomp-bpf and Landlock on Linux, sandbox-exec on
   macOS, AppContainer on Windows for shell tools. Generated plugins are untrusted code, so the
   WASM track is a prerequisite for step 6's `Wasm` counterpart, not an optional hardening pass.

## License

MIT
