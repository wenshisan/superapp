//! 示例模型插件：echo（把用户输入原样返回）。
//! 演示"模型亦为插件"——可替换为 DeepSeek/OpenAI/Anthropic 等近 40 家。

use cordis_rs::{Plugin, PluginContext, Manifest, Capability};

pub struct EchoModel;

impl Plugin for EchoModel {
    fn manifest(&self) -> &Manifest {
        static M: std::sync::OnceLock<Manifest> = std::sync::OnceLock::new();
        M.get_or_init(|| {
            let mut m = Manifest::new("model.echo");
            m.capabilities.insert(Capability::ModelProvider);
            m
        })
    }

    fn activate(&self, _ctx: &mut PluginContext) -> anyhow::Result<()> {
        // 真实插件在这里注册 ModelProvider 服务到 ctx.services。
        Ok(())
    }
}

/// 动态库导出入口（对应 loader.rs 的 dylib 契约）。
/// 注意：每个插件须使用唯一符号名，避免静态链接时符号冲突。
/// 返回 `Box<dyn Plugin>` 经 `Box::into_raw` 转为 `*mut dyn Plugin` 才是真正 FFI 安全形态；
/// 这里保留 Box 形态仅作契约示意，真实 loader 用 libloading 读取后转换。
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn create_plugin_model_echo() -> Box<dyn Plugin> {
    Box::new(EchoModel)
}
