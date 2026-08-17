//! ServiceRegistry（服务注册表）：类型化服务注册/反注册。
//!
//! 基于 TypeId 实现类型安全查找，替代 Cordis 的字符串服务定位。

use anymap2::AnyMap;
use std::sync::{Arc, RwLock};

/// 类型化服务容器。插件激活时注册，Scope 撤销时清空。
/// 内部以 `Arc<T>` 存储，以便 `get` 安全地返回共享引用。
#[derive(Default)]
pub struct ServiceRegistry {
    services: RwLock<AnyMap>,
}

impl ServiceRegistry {
    pub fn new() -> Self { Self::default() }

    /// 注册一个服务实例（按类型索引）。
    pub fn register<T: Send + Sync + 'static>(&self, service: T) {
        self.services.write().unwrap().insert(Arc::new(service));
    }

    /// 获取已注册服务的共享引用。
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.services.read().unwrap().get::<Arc<T>>().cloned()
    }
}
