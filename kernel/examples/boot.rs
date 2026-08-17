//! 最小启动示例：验证内核 + 插件加载 + 轨迹记录跑通。
//! 运行：cargo run -p cordis-rs --example boot

use cordis_rs::Kernel;
use model_echo::EchoModel;
use tool_bash::BashTool;

fn main() -> anyhow::Result<()> {
    let kernel = Kernel::new();

    // 加载插件（对应 DSH "一切皆插件"）
    kernel.load(EchoModel)?;
    kernel.load(BashTool)?;

    println!("active plugins: {:?}", kernel.lifecycle.active_ids());

    // 发布一条事件，验证 EventBus + Trace
    kernel.events.publish(cordis_rs::Event::Thought {
        agent: "main".into(),
        text: "hello from Cordis-RS".into(),
    });

    kernel.trace.checkpoint("boot-done");
    println!("trace entries: {}", kernel.trace.len());

    // 卸载插件（Scope/Drop 保证副作用自动撤销）
    kernel.lifecycle.deactivate("model.echo")?;
    kernel.lifecycle.deactivate("tool.bash")?;
    println!("after unload: {:?}", kernel.lifecycle.active_ids());

    Ok(())
}
