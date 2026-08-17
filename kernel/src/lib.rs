//! Cordis-RS —— 插件式 Agent 通用框架内核（Rust）
//!
//! 设计哲学（映射自 DeepSeek Harness 的 Cordis）：
//! - 一切皆插件（Everything is a Plugin）
//! - 时空可组合性：插件注册到独立 Scope，卸载时 RAII 自动撤销副作用
//!
//! 内核仅负责"让插件安全地组合在一起"，不含任何 Agent 业务逻辑。

pub mod scope;
pub mod registry;
pub mod event_bus;
pub mod lifecycle;
pub mod loader;
pub mod trace;
pub mod plugin;

pub use plugin::{Manifest, Plugin, PluginContext, Capability};
pub use scope::Scope;
pub use registry::ServiceRegistry;
pub use event_bus::{EventBus, Event};
pub use lifecycle::LifecycleManager;
pub use loader::{PluginLoader, load_plugin_from_lib};
pub use trace::{TraceCollector, TraceEntry};

use std::sync::Arc;

/// 内核聚合体：持有所有核心子系统，通过 Arc 在插件与 UI 间共享。
#[derive(Clone)]
pub struct Kernel {
    pub scope: Arc<Scope>,
    pub services: Arc<ServiceRegistry>,
    pub events: Arc<EventBus>,
    pub lifecycle: Arc<LifecycleManager>,
    pub trace: Arc<TraceCollector>,
}

impl Kernel {
    /// 创建一个空内核（约 3000 行内核代码的骨架入口）。
    pub fn new() -> Self {
        let scope = Scope::root();
        let services = Arc::new(ServiceRegistry::new());
        let events = Arc::new(EventBus::new());
        let trace = Arc::new(TraceCollector::new());
        let lifecycle = Arc::new(LifecycleManager::new(
            scope.clone(),
            services.clone(),
            events.clone(),
            trace.clone(),
        ));
        Kernel { scope, services, events, lifecycle, trace }
    }

    /// 加载并激活一个插件到根作用域。
    pub fn load<P: Plugin + 'static>(&self, plugin: P) -> anyhow::Result<()> {
        self.lifecycle.activate(plugin)
    }
}

impl Default for Kernel {
    fn default() -> Self { Self::new() }
}
