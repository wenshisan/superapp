# 插件式 Agent 通用框架 —— 架构设计（Rust 内核 + Flutter 插件化 UI）

> 参考 **DeepSeek Harness (DSH)** 的"一切皆插件（Everything is a Plugin）"与"Cordis 时空可组合性"理念，
> 将底层运行时从 **Node.js** 替换为 **Rust**，用户界面从 Web 替换为**插件化的 Flutter**。
> 目标：一套内存安全、高性能、模型中立、可热插拔的通用 Agent 运行基础设施，覆盖桌面与移动端。

---

## 1. 设计哲学：一切皆插件 + 时空可组合性

沿用 DSH 的核心范式，并映射到 Rust 工程现实：

| 维度 | 原 DSH（Node.js + Web UI） | 本框架（Rust 内核 + Flutter UI） |
|------|----------------------------|----------------------------------|
| 内核语言 | TypeScript（约 2700 行内核） | Rust（约 3000 行内核，零成本抽象） |
| 插件形态 | npm 包 + TS 装饰器 | 动态库（`.so`/`.dylib`/`.dll`）或 WASM 模块 |
| 热插拔引擎 | Cordis（基于 TS 反射） | **Cordis-RS**（基于 Rust trait 对象 + 生命周期作用域） |
| UI | 内置 Web（3030 端口） | **Flutter 插件化界面**（可装卸 UI 插件） |
| 沙箱 | Linux 内核级 | `seccomp`/命名空间 + WASM 沙箱双轨 |
| 模型绑定 | 近 40 家 | 近 40 家（模型亦为插件） |

**时空可组合性（Spatio-Temporal Composability）** 在本框架中的实现：
- **空间**：每个插件注册到独立的"作用域（Scope）"，插件卸载时其注册的服务、事件、副作用由 Rust 的 RAII / `Drop` 自动反向撤销。
- **时间**：执行轨迹（Trace）是不可变事件流（Event Sourcing），支持恢复、分叉、回放。

---

## 2. 总体架构分层

```
┌──────────────────────────────────────────────────────────────────┐
│                     Flutter UI 层（Dart，插件化）                   │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌─────────────────┐ │
│  │ 会话插件   │ │ 插件市场   │ │ 工作流画布 │ │ 调试/轨迹回放   │ │
│  └────────────┘ └────────────┘ └────────────┘ └─────────────────┘ │
│            所有 UI 均为可装卸插件，经 UI-Bridge 通信                │
└───────────────────────────────┬────────────────────────────────────┘
                                 │  FFI (flutter_rust_bridge) / gRPC
┌───────────────────────────────┴────────────────────────────────────┐
│                      Rust 内核（Cordis-RS）                          │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │  Scope / Service Registry / Event Bus / Lifecycle Manager     │ │
│  └──────────────────────────────────────────────────────────────┘ │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────┐  │
│  │ 模型插件 │ │ 工具插件 │ │ 技能插件 │ │ 沙箱插件 │ │ Agent循环插件│  │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────────┘  │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐                  │
│  │ 存储插件 │ │ 调度插件 │ │ 追踪插件 │ │ 通信插件 │                  │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘                  │
└───────────────────────────────┬────────────────────────────────────┘
                                 │  进程隔离 / gRPC
┌───────────────────────────────┴────────────────────────────────────┐
│                    沙箱层（安全执行环境）                             │
│   WASM 微沙箱（工具/技能）  │   OS 级沙箱（seccomp + namespace）     │
└────────────────────────────────────────────────────────────────────┘
```

---

## 3. Rust 内核（Cordis-RS）

### 3.1 内核职责（约 3000 行，零业务）

内核只负责"让插件能安全地组合在一起"，自身不含任何 Agent 业务逻辑：

- **Scope（作用域）**：插件的隔离容器，`Arc<Scope>` 共享，卸载时级联 `drop`。
- **Service Registry（服务注册表）**：类型化服务（`TypeId` → 实例）注册/反注册。
- **Event Bus（事件总线）**：发布/订阅，支持同步与异步通道（`tokio`）。
- **Lifecycle Manager（生命周期管理器）**：插件加载、激活、暂停、卸载、热替换。
- **Plugin Loader（插件加载器）**：从动态库或 WASM 中解析 `Plugin` trait 实现并实例化。
- **Trace Collector（轨迹收集器）**：所有事件的不可变记录，供 UI 回放。

### 3.2 插件协议（Plugin Trait）

```rust
/// 所有插件必须实现的核心 trait（编译期契约）
pub trait Plugin: Send + Sync {
    /// 插件元数据：id、版本、依赖、能力声明
    fn manifest(&self) -> &Manifest;

    /// 激活：注册服务、订阅事件、声明副作用
    fn activate(&self, ctx: &mut PluginContext) -> Result<()>;

    /// 停用：反向撤销（由 Scope Drop 保证自动执行）
    fn deactivate(&self, ctx: &mut PluginContext) -> Result<()> { Ok(()) }
}

/// 插件上下文：注入内核能力
pub struct PluginContext<'a> {
    pub scope: &'a Scope,
    pub services: &'a mut ServiceRegistry,
    pub events: &'a EventBus,
    pub tracing: &'a TraceCollector,
}
```

### 3.3 安全热插拔（替代 Cordis 的 TS 反射机制）

- 插件以 **动态库** 或 **WASM** 形式提供，`extern "C"` 导出 `create_plugin() -> Box<dyn Plugin>`。
- 加载时进入独立 `Scope`；卸载时 `Scope` 被 `drop`，其持有的服务句柄、事件订阅、打开的文件/连接全部按 RAII 自动释放（对应 Cordis 的"副作用自动撤销"）。
- 支持 **模型在运行时现场写 Rust/WASM 插件并挂载、用完拆卸**（对应 DSH 的"模型给自己挂插件"）。

### 3.4 四种工作模式（对应 DSH）

| 模式 | 实现方式（Rust 插件组合） |
|------|--------------------------|
| 标准模式 | 加载 `agent-loop-standard` + `tool-fs` + `tool-shell` + `tool-web` + `skill-*` |
| PTC 模式 | 加载 `code-mode-sdk`（用 Rust/WASM 程序组合多步工具调用） |
| 极简模式 | 仅 `tool-bash` + `tool-str-replace`（接近"裸奔"，用于基准测试） |
| 创造模式 | 运行时动态调试插件组合，保存为自定义 Agent 预设 |

---

## 4. 插件分类与契约示例

### 4.1 模型插件（模型中立）

```rust
pub trait ModelProvider: Plugin {
    fn chat(&self, req: ChatRequest) -> AsyncStream<ChatChunk>;
    fn supports(&self, cap: Capability) -> bool;
}
```
已适配 DeepSeek / OpenAI / Anthropic / Google / Kimi 等近 40 家，统一为内部 `ChatRequest/ChatChunk` 协议。

### 4.2 工具插件

```rust
pub trait Tool: Plugin {
    fn schema(&self) -> ToolSchema;          // JSON Schema 描述
    fn invoke(&self, args: Value, sandbox: &Sandbox) -> Result<Value>;
}
```

### 4.3 技能 / 会话 / 沙箱 / 存储 / 调度 / 追踪插件

均以独立 trait + 独立动态库存在，可自由替换（如 `sandbox-wasm` ↔ `sandbox-nspawn`）。

---

## 5. 安全沙箱（双轨）

| 沙箱类型 | 适用 | 机制 |
|----------|------|------|
| WASM 微沙箱 | 工具/技能代码 | `wasmtime` + 能力授权（文件/网络白名单） |
| OS 级沙箱 | Shell / 重型命令 | Linux `seccomp-BPF` + `namespaces` + `landlock`；Windows/macOS 用平台等价物 |

对应 DSH"确保 Agent 操作文件、执行命令不越界"。

---

## 6. Flutter 插件化 UI（用户界面亦为插件）

UI 不再是一套内置 Web，而是 **Flutter 插件体系**：

- **UI-Bridge**：Flutter 侧经 `flutter_rust_bridge` 调用内核，内核经事件总线推送状态。
- **UI 插件契约**：

```dart
abstract class UiPlugin {
  String get id;
  Widget build(BuildContext ctx, KernelBridge bridge);
  void onAttach(KernelBridge bridge);
  void onDetach();
}
```

- **内置 UI 插件**：
  - 会话插件（聊天/工具调用可视化）
  - 插件市场（浏览、安装、卸载内核 & UI 插件）
  - 工作流画布（编排 Agent 循环与工具链）
  - 轨迹回放（读取 Trace，恢复/分叉/回放运行过程）
  - 设置/密钥管理
- **跨平台**：同一套 Flutter 代码编译到 Windows / macOS / Linux / Android / iOS。

---

## 7. 全链路可追溯

- 内核 `TraceCollector` 以 **不可变事件流** 记录：系统提示词、思维链、工具调用及结果、子 Agent 调度、上下文注入。
- Flutter 轨迹回放插件消费该流，支持时间轴拖拽、断点恢复、分支分叉（对应 DSH 的"全链路可追溯、可回放"）。

---

## 8. 工程结构（建议）

```
superapp/
├── kernel/                 # Rust 内核（Cordis-RS），~3000 行
│   ├── scope.rs
│   ├── registry.rs
│   ├── event_bus.rs
│   ├── lifecycle.rs
│   ├── loader.rs           # 动态库 / WASM 加载
│   └── trace.rs
├── plugins/                # Rust 插件（各为独立 crate / 动态库）
│   ├── model-*             # 模型插件
│   ├── tool-*              # 工具插件
│   ├── sandbox-*           # 沙箱插件
│   ├── agent-loop-*        # Agent 循环插件
│   └── ...
├── ui/                     # Flutter 应用
│   ├── lib/
│   │   ├── bridge/         # flutter_rust_bridge 生成层
│   │   ├── ui_plugins/     # Flutter UI 插件
│   │   └── core/
│   └── pubspec.yaml
├── proto/                  # FFI / gRPC 契约（.proto）
└── ARCHITECTURE.md
```

---

## 9. 与原 DSH 的能力对照

| 能力 | DSH（Node.js + Web） | 本框架（Rust + Flutter） |
|------|----------------------|--------------------------|
| 一切皆插件 | ✅ | ✅ |
| 安全热插拔 | Cordis（TS 反射） | Cordis-RS（trait + Scope/Drop） |
| 模型中立 | 近 40 家 | 近 40 家（同协议） |
| 轻量内核 | 2700+ 行 | 3000+ 行（Rust） |
| 安全沙箱 | Linux 内核级 | WASM + OS 级双轨 |
| 全链路可追溯 | ✅ | ✅（Trace 事件流） |
| 四种模式 | ✅ | ✅ |
| 界面 | 内置 Web（3030） | 插件化 Flutter（跨端） |
| 启动 | `npx @deepseek-ai/dsh web` | 原生二进制 / 移动 App |

---

## 10. 总结

本框架将 DeepSeek Harness 的范式革新完整迁移到 Rust + Flutter 技术栈：
- **Rust 内核** 以零成本抽象和 RAII 提供比 TS 更确定的内存安全与热插拔保证；
- **Flutter 插件化 UI** 让"界面本身也是插件"，一套代码覆盖全平台；
- 保留 DSH 的全部核心主张——模型中立、一切皆插件、安全沙箱、全链路可追溯、四种工作模式。

---

## 11. 性能差距分析：Rust 内核 vs Node.js 内核（DeepSeek Harness）

从七个维度量化对比"把 DSH 的 Node.js 内核换成 Rust 内核"后的性能差距。带 ⚡ 的为 Rust 明显占优项。

### 11.1 启动时间

| 项 | Node.js (DSH) | Rust (Cordis-RS) |
|----|---------------|------------------|
| 运行时启动 | V8 冷启动 + JIT 预热，~150–400 ms | 原生二进制，无 VM，~5–20 ms |
| 插件加载 | 解析 npm 包 + TS 装饰器反射 | `dlopen` 动态库 / WASM 实例化，无反射开销 |
| 整体冷启动 | 数百 ms 级（npx 还要拉包） | 数十 ms 级 |

**结论**：Rust 冷启动快 **1–2 个数量级**。对"极简模式做基准测试""创造模式频繁重载"场景尤其关键。

### 11.2 内存占用 ⚡

| 项 | Node.js | Rust |
|----|---------|------|
| 运行时基础开销 | V8 堆 + JIT 区，常驻 50–150 MB | 仅内核 + 插件，常驻 5–20 MB |
| GC 行为 | 周期性 STW 停顿，堆越大越明显 | 编译期所有权，无 GC，无停顿 |
| 长会话/大上下文 | 上下文膨胀 → 堆增长 → GC 抖动 | 栈/堆精确控制，可零拷贝借用 |

**结论**：常驻内存少 **一个数量级**；长任务下 Node 的 GC 停顿会让 Agent 响应出现"卡顿尖峰"，Rust 平滑。

### 11.3 并发与多 Agent 编排 ⚡

| 项 | Node.js | Rust |
|----|---------|------|
| 模型 | 单线程事件循环 + 异步（受限于 V8 单线程） | `tokio` 多核 M:N 调度，真并行 |
| 子 Agent 并行 | 受事件循环吞吐制约，CPU 密集任务阻塞 | 多核并行 spawn，CPU/IO 任务隔离 |
| 多模型并发调用 | 受单线程调度上限 | 线性扩展至多核 |

**结论**：标准模式里"子 Agent + 工具并行"时，Rust 在多核上可线性扩展；Node 在 CPU 密集工具调用（如大文件解析）时会饿死事件循环。差距在 **N 核机上可达 N 倍**。

### 11.4 插件热插拔开销

| 项 | Node.js (Cordis) | Rust (Cordis-RS) |
|----|------------------|------------------|
| 卸载副作用撤销 | 依赖 TS 装饰器登记的清理回调 | RAII `Drop` 确定性执行，编译期保证 |
| 悬挂引用风险 | 运行时可能漏注销 → 内存泄漏 | 编译期借用检查，类型系统排除悬挂 |
| 热替换延迟 | 卸载+重载模块图，含 GC | `Scope` drop + `dlclose`，确定性 |

**结论**：Rust 把"安全热插拔"从**运行时约定**变成**编译期保证**，无泄漏、无不确定停顿。

### 11.5 工具执行（Agent 真正"动手"的部分）⚡

- **纯计算/解析工具**（JSON/代码解析、向量化、压缩）：Rust 比 Node **快 5–50 倍**（无 JIT 预热、无 GC）。
- **IO 密集型工具**（读写、网络）：两者接近，但 Rust 无事件循环瓶颈，尾部延迟更低。
- **PTC 模式**（程序化多步工具组合）：若用 WASM 而非 TS 跑组合逻辑，Rust/WASM 接近原生速度，Node 仍是解释执行。

### 11.6 沙箱开销

| 项 | Node.js (DSH) | Rust (Cordis-RS) |
|----|---------------|------------------|
| 沙箱实现 | Linux 内核级（与运行时正交） | WASM 微沙箱 + OS 级 |
| WASM 工具 | 需额外桥接到 Node | `wasmtime` 原生嵌入，近零桥接成本 |
| 上下文切换 | 进程/容器级，较重 | WASM 实例轻量，切换 μs 级 |

**结论**：Rust 内嵌 `wasmtime` 让"工具级微沙箱"几乎零成本，比 Node 侧外挂 WASM 更高效。

### 11.7 能耗与部署 ⚡

| 项 | Node.js | Rust |
|----|---------|------|
| 移动端/边缘 | 需带 V8，体积大、耗电 | 单二进制，体积小、省电 |
| 移动 App 集成 | 需内嵌 JS 引擎 | 直接 FFI，无额外运行时 |

**结论**：选 Flutter 做 UI 后，Rust 原生二进制可无缝打包进 App，省去移动端 V8 体积与功耗开销——这是 Node 方案在移动端**无法补齐**的硬伤。

### 11.8 综合差距速览

| 维度 | Rust 相对 Node 的提升 |
|------|----------------------|
| 冷启动 | 快 10–50× |
| 常驻内存 | 少 5–10× |
| 多 Agent 并行 | 多核线性扩展（N×） |
| 计算型工具 | 快 5–50× |
| 热插拔确定性 | 编译期保证 vs 运行时约定 |
| 移动端部署 | 原生可行 vs 需带引擎 |

### 11.9 何时差距"不明显"

- **瓶颈在 LLM 网络延迟**：模型响应 1–10s 时，内核快慢被淹没，此时差距感知弱。
- **工具全为外部 API 调用**：IO 等待主导，内核语言差异被掩盖。
- **单 Agent 低频交互**：并发优势无法体现。

### 11.10 一句话总结

Rust 内核在**启动、内存、并发、计算型工具、移动部署**上相对 Node.js 有数量级优势，且把"安全热插拔"从运行时约定升级为编译期保证；但当 Agent 瓶颈在**网络/模型延迟**时，内核语言差异对用户感知影响有限。对"创造模式频繁重载 + 多 Agent 并行 + 移动端"三位一体的场景，Rust + Flutter 的组合收益最大。
