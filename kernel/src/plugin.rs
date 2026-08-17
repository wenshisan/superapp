//! 插件协议：所有插件必须实现的编译期契约。

use crate::{Scope, ServiceRegistry, EventBus, TraceCollector};
use std::collections::HashSet;

/// 插件元数据：id、版本、依赖、能力声明。
#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub capabilities: HashSet<Capability>,
}

impl Manifest {
    pub fn new(id: &str) -> Self {
        Manifest {
            id: id.to_string(),
            version: "0.1.0".to_string(),
            dependencies: Vec::new(),
            capabilities: HashSet::new(),
        }
    }
}

/// 能力声明（用于依赖解析与冲突检测）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    ModelProvider,
    Tool,
    Skill,
    Sandbox,
    AgentLoop,
    Storage,
    Scheduler,
    Tracer,
    Ui,
}

/// 插件核心 trait —— 编译期契约（替代 Cordis 的 TS 装饰器反射）。
pub trait Plugin: Send + Sync {
    /// 插件元数据。
    fn manifest(&self) -> &Manifest;

    /// 激活：注册服务、订阅事件、声明副作用。
    fn activate(&self, ctx: &mut PluginContext) -> anyhow::Result<()>;

    /// 停用：反向撤销（由 Scope Drop 保证自动执行）。
    fn deactivate(&self, _ctx: &mut PluginContext) -> anyhow::Result<()> {
        Ok(())
    }
}

/// 注入内核能力的插件上下文。
pub struct PluginContext<'a> {
    pub scope: &'a Scope,
    pub services: &'a ServiceRegistry,
    pub events: &'a EventBus,
    pub tracing: &'a TraceCollector,
}
