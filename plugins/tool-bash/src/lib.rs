//! 示例工具插件：bash（极简模式双工具之一）。
//! 演示 Tool 插件契约；真实实现应在 Sandbox 内执行命令。

use cordis_rs::{Plugin, PluginContext, Manifest, Capability};

pub struct BashTool;

impl Plugin for BashTool {
    fn manifest(&self) -> &Manifest {
        static M: std::sync::OnceLock<Manifest> = std::sync::OnceLock::new();
        M.get_or_init(|| {
            let mut m = Manifest::new("tool.bash");
            m.capabilities.insert(Capability::Tool);
            m
        })
    }

    fn activate(&self, _ctx: &mut PluginContext) -> anyhow::Result<()> {
        // 真实实现：注册 Tool trait 实例到 ctx.services，
        // invoke() 在 Sandbox 内执行 shell（见 ARCHITECTURE.md §6）。
        Ok(())
    }
}

#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn create_plugin_tool_bash() -> Box<dyn Plugin> {
    Box::new(BashTool)
}
