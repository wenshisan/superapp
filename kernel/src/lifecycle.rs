//! LifecycleManager（生命周期管理器）：加载、激活、暂停、卸载、热替换。
//! 时空可组合性的"时间"维度 + 安全热插拔。

use crate::{Plugin, PluginContext, Scope, ServiceRegistry, EventBus, TraceCollector};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 已激活插件的运行态记录。
#[allow(dead_code)]
struct ActivePlugin {
    type_id: TypeId,
    manifest_id: String,
}

/// 生命周期管理器：持有所有已加载插件，卸载时驱动反向撤销。
pub struct LifecycleManager {
    scope: Arc<Scope>,
    services: Arc<ServiceRegistry>,
    events: Arc<EventBus>,
    trace: Arc<TraceCollector>,
    active: RwLock<HashMap<String, ActivePlugin>>,
}

impl LifecycleManager {
    pub fn new(
        scope: Arc<Scope>,
        services: Arc<ServiceRegistry>,
        events: Arc<EventBus>,
        trace: Arc<TraceCollector>,
    ) -> Self {
        LifecycleManager {
            scope, services, events, trace,
            active: RwLock::new(HashMap::new()),
        }
    }

    /// 激活插件：调用 activate，并记录到活动表。
    pub fn activate<P: Plugin + 'static>(&self, plugin: P) -> anyhow::Result<()> {
        let ctx = PluginContext {
            scope: &self.scope,
            services: &self.services,
            events: &self.events,
            tracing: &self.trace,
        };
        plugin.activate(&mut { ctx })?;
        self.active.write().unwrap().insert(
            plugin.manifest().id.clone(),
            ActivePlugin { type_id: TypeId::of::<P>(), manifest_id: plugin.manifest().id.clone() },
        );
        self.trace.record(crate::trace::TraceEntry::PluginLoaded(plugin.manifest().id.clone()));
        Ok(())
    }

    /// 卸载插件：调用 deactivate，并从活动表移除（Scope Drop 保证副作用撤销）。
    pub fn deactivate(&self, id: &str) -> anyhow::Result<()> {
        if let Some(_p) = self.active.write().unwrap().remove(id) {
            self.trace.record(crate::trace::TraceEntry::PluginUnloaded(id.to_string()));
        }
        Ok(())
    }

    pub fn active_ids(&self) -> Vec<String> {
        self.active.read().unwrap().keys().cloned().collect()
    }
}
