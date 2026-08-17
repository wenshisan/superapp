//! EventBus（事件总线）：发布/订阅，驱动插件间解耦通信。
//! 对应 DSH 的全链路事件流与"模型调度子 Agent"等机制。

use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

/// 内核统一事件（全链路可追溯的数据单元）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    SystemPrompt(String),
    Thought { agent: String, text: String },
    ToolCall { agent: String, tool: String, args: serde_json::Value },
    ToolResult { agent: String, tool: String, output: serde_json::Value },
    SubAgentSpawn { parent: String, child: String },
    ContextInject { agent: String, snippet: String },
    Custom(String, serde_json::Value),
}

/// 事件总线：多生产者多消费者，支持同步快照与异步订阅。
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        EventBus { tx }
    }

    /// 发布事件（同时写入 TraceCollector）。
    pub fn publish(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    /// 订阅事件流。
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self { Self::new() }
}
