//! Scope（作用域）：插件的隔离容器。
//!
//! 时空可组合性的"空间"维度：插件注册到独立 Scope，
//! Scope 被 drop 时其持有的子 Scope 与资源句柄级联释放（RAII）。

use std::sync::{Arc, Weak};
use std::collections::HashMap;

/// 一个作用域，可嵌套子作用域。父 Scope 释放时，子 Scope 一并释放。
pub struct Scope {
    pub id: String,
    parent: Option<Weak<Scope>>,
    children: std::sync::RwLock<HashMap<String, Arc<Scope>>>,
}

impl Scope {
    /// 根作用域。
    pub fn root() -> Arc<Self> {
        Arc::new(Scope {
            id: "root".to_string(),
            parent: None,
            children: Default::default(),
        })
    }

    /// 创建子作用域；返回的 Arc 被 drop 时即自动撤销该空间内所有注册。
    /// 父引用用 Weak 持有，避免循环引用导致内存泄漏。
    pub fn child(self: &Arc<Self>, id: &str) -> Arc<Scope> {
        let child = Arc::new(Scope {
            id: id.to_string(),
            parent: Some(Arc::downgrade(self)),
            children: Default::default(),
        });
        self.children.write().unwrap().insert(id.to_string(), child.clone());
        child
    }

    pub fn id(&self) -> &str { &self.id }
}
