//! TraceCollector（轨迹收集器）：不可变事件流，支持恢复/分叉/回放。
//! 对应 DSH"全链路可追溯、可回放"。

use serde::{Serialize, Deserialize};
use std::sync::Mutex;

/// 一条轨迹记录（不可变）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceEntry {
    PluginLoaded(String),
    PluginUnloaded(String),
    Event(crate::event_bus::Event),
    Checkpoint(String), // 用于分叉点
}

/// 轨迹收集器：追加式写入，供 UI 回放插件消费。
#[derive(Default)]
pub struct TraceCollector {
    entries: Mutex<Vec<TraceEntry>>,
}

impl TraceCollector {
    pub fn new() -> Self { Self::default() }

    pub fn record(&self, entry: TraceEntry) {
        self.entries.lock().unwrap().push(entry);
    }

    /// 打一个分叉检查点（创造模式/轨迹回放用）。
    pub fn checkpoint(&self, label: &str) {
        self.record(TraceEntry::Checkpoint(label.to_string()));
    }

    /// 导出全部轨迹（供 Flutter 轨迹回放插件读取）。
    pub fn snapshot(&self) -> Vec<TraceEntry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn len(&self) -> usize { self.entries.lock().unwrap().len() }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}
