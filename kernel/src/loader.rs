//! PluginLoader（插件加载器）：从动态库或 WASM 解析 Plugin trait 实现。
//! 对应 DSH"模型可在运行时现场写代码挂插件，用完拆掉"。

use crate::Plugin;
use std::path::Path;

/// 插件加载器：负责从外部产物实例化插件。
pub struct PluginLoader;

impl PluginLoader {
    /// 从动态库（.so/.dylib/.dll）加载插件。
    /// 动态库需导出 `create_plugin() -> Box<dyn Plugin>`（extern "C"）。
    ///
    /// 生产实现使用 `libloading`；此处给出契约签名。
    pub fn load_from_dylib(_path: &Path) -> anyhow::Result<Box<dyn Plugin>> {
        // let lib = libloading::Library::new(path)?;
        // let ctor: libloading::Symbol<extern "C" fn() -> Box<dyn Plugin>> = lib.get(b"create_plugin\0")?;
        // Ok(ctor())
        anyhow::bail!("dylib loading requires `libloading` (see ARCHITECTURE.md §3.3)")
    }

    /// 从 WASM 模块加载插件（wasmtime 实例化后调用其 `create_plugin` 导出）。
    pub fn load_from_wasm(_bytes: &[u8]) -> anyhow::Result<Box<dyn Plugin>> {
        anyhow::bail!("wasm loading requires `wasmtime` (see ARCHITECTURE.md §6)")
    }
}

/// 便捷封装：直接传入已构造的插件对象（用于内置/测试插件）。
pub fn load_plugin_from_lib(_plugin: Box<dyn Plugin>) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(_plugin)
}
