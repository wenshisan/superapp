---
name: superapp-architecture
description: Architecture guide for the superapp pluggable agent framework (Rust kernel + Flutter UI)
---

# Superapp Architecture

A pluggable agent framework with a Rust kernel (Cordis-RS) and Flutter plugin UI, where everything—models, tools, skills, storage, UI components—is implemented as a hot-swappable plugin.

## Core Philosophy: Everything is a Plugin

Every capability is a plugin implementing the core `Plugin` trait. Plugins receive a `PluginContext` with scoped services, events, and tracing. Hot-swap support allows runtime replacement without restart.

## The Plugin Contract

```rust
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn activate(&self, ctx: &mut PluginContext) -> anyhow::Result<()>;
    fn deactivate(&self, _ctx: &mut PluginContext) -> anyhow::Result<()> { Ok(()) }
}

pub struct PluginContext<'a> {
    pub scope: &'a Scope,
    pub services: &'a ServiceRegistry,
    pub events: &'a EventBus,
    pub tracing: &'a TraceCollector,
}
```

## Plugin Categories

### Model Plugins
```rust
pub trait ModelProvider: Plugin {
    fn chat(&self, req: ChatRequest) -> AsyncStream<ChatChunk>;
    fn supports(&self, cap: Capability) -> bool;
}
```

### Tool Plugins
```rust
pub trait Tool: Plugin {
    fn schema(&self) -> ToolSchema;
    fn invoke(&self, args: Value, sandbox: &Sandbox) -> Result<Value>;
}
```

### UI Plugins (Flutter)
```dart
abstract class UiPlugin {
  String get id;
  Widget build(BuildContext ctx, KernelBridge bridge);
  void onAttach(KernelBridge bridge);
  void onDetach();
}
```

## Repository Structure

```
superapp/
├── kernel/              # Rust kernel, crate `cordis-rs`
│   ├── src/
│   │   ├── lib.rs      # Kernel aggregate
│   │   ├── plugin.rs   # Plugin trait & manifest
│   │   ├── scope.rs    # Nested isolation container
│   │   ├── registry.rs # Service registry
│   │   ├── event_bus.rs# Pub/sub
│   │   ├── lifecycle.rs# Plugin lifecycle
│   │   ├── loader.rs   # Dylib/WASM loading
│   │   └── trace.rs    # Append-only event log
│   └── examples/boot.rs
├── plugins/
│   ├── model-echo/     # Example model plugin
│   └── tool-bash/      # Example tool plugin
└── ui/                 # Flutter workspace (planned)
```

## Key Design Patterns

**Space/Time Composability**
- Each plugin registers into its own `Scope`
- Dropping the scope cascade-releases all resources (RAII)
- Runs are append-only event logs, enabling resume/fork/replay

**Hot Swap Mechanism**
1. Plugin ships as dylib or WASM module
2. Loading creates a dedicated child `Scope`
3. Unloading drops that `Scope`, releasing resources via RAII

**Rust Kernel Responsibilities**
- Making plugins safely composable
- No agent business logic (no prompt assembly, tool dispatch, model selection)
- Lightweight: ~460 lines today, ~3,000 budgeted

## Performance Characteristics

| Dimension | Expected Improvement |
|-----------|---------------------|
| Cold start | 10–50× faster than Node.js |
| Resident memory | 5–10× smaller |
| Multi-agent parallelism | N× on N cores |
| Compute-heavy tools | 5–50× faster |
| Hot-swap | Compile-time guarantee vs runtime convention |

## Implementation Status Legend

- ✅ **built**: Implemented and working
- 🟡 **contract**: Types exist, behavior incomplete
- ⬜ **planned**: Not in tree yet

## When Writing Code

1. **Follow the plugin contract** — every capability is a plugin
2. **Use existing services** — register with `PluginContext`, don't create parallel systems
3. **Respect isolation** — plugins get their own `Scope`
4. **Match the Rust style** — memory-safe, zero-cost abstractions, trait-based
5. **Keep the kernel lightweight** — business logic belongs in plugins
6. **Design for hot-swap** — resources must be releasable via `Scope` drop

## Four Operating Modes (Planned)

- **Standard**: Full agent loop with tools and skills
- **PTC**: Programmatic tool calling via Rust/WASM
- **Minimal**: Bare baseline for benchmarking
- **Creative**: Compose and reload plugins live

## Reference

See `ARCHITECTURE.md` in the repository root for complete technical specifications, open design questions, and roadmap details.
