# superapp —— 插件式 Agent 通用框架（Rust 内核 + Flutter 插件化 UI）

基于 DeepSeek Harness 的"一切皆插件"与"Cordis 时空可组合性"理念，
将 Node.js 内核替换为 **Rust**（Cordis-RS），UI 用 **Flutter 插件化** 实现。

详见架构设计：`ARCHITECTURE.md`

## 工程结构

```
superapp/
├── Cargo.toml                 # Rust workspace 根
├── rust-toolchain.toml        # 固定 stable 工具链
├── kernel/                    # Rust 内核（Cordis-RS）
│   ├── src/
│   │   ├── lib.rs             # Kernel 聚合入口
│   │   ├── plugin.rs          # Plugin trait 编译期契约
│   │   ├── scope.rs           # Scope 作用域（RAII 隔离）
│   │   ├── registry.rs        # ServiceRegistry 类型化服务表
│   │   ├── event_bus.rs       # EventBus 事件总线
│   │   ├── lifecycle.rs       # LifecycleManager 热插拔
│   │   ├── loader.rs          # PluginLoader 动态库/WASM
│   │   └── trace.rs           # TraceCollector 全链路轨迹
│   └── examples/boot.rs       # 最小启动示例
├── plugins/
│   ├── model-echo/            # 示例模型插件（echo）
│   └── tool-bash/             # 示例工具插件（bash）
└── ui/                        # （待建）Flutter 插件化界面
```

## 快速开始

```bash
# 构建内核与示例
cargo build -p cordis-rs --examples

# 运行最小启动示例（验证加载/事件/轨迹/卸载）
cargo run -p cordis-rs --example boot
```

预期输出：
```
active plugins: ["model.echo", "tool.bash"]
trace entries: 3
after unload: []
```

## 核心概念对照（DSH → Cordis-RS）

| DSH (Cordis/TS) | Cordis-RS (Rust) |
|-----------------|------------------|
| 装饰器反射注册 | `Plugin` trait 编译期契约 |
| Scope 自动撤销 | `Scope` + `Drop` RAII |
| 事件流 | `EventBus` (tokio broadcast) |
| 轨迹回放 | `TraceCollector` 不可变事件流 |
| dylib/WASM 插件 | `PluginLoader` (libloading/wasmtime) |

## 下一步

1. 实现 `PluginLoader::load_from_dylib`（引入 `libloading`）与 `load_from_wasm`（引入 `wasmtime`）。
2. 定义 `ModelProvider` / `Tool` / `AgentLoop` 等子 trait，迁移 120+ 插件。
3. 搭建 `ui/` Flutter 工程，用 `flutter_rust_bridge` 桥接内核（见 ARCHITECTURE.md §6）。
4. 接入真实模型（DeepSeek/OpenAI/...）与 OS 级沙箱（seccomp/landlock）。
