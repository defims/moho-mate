//! start 命令 - 启动 IPC 服务

use anyhow::Result;
use tracing::info;

use crate::ipc::IpcClient;

pub async fn execute(project: Option<&str>, script: Option<&str>, timeout: u32) -> Result<()> {
    info!("▶ 启动 IPC 服务");
    println!("  超时: {} 秒", timeout);
    if let Some(p) = project {
        println!("  项目: {}", p);
    }
    if let Some(s) = script {
        println!("  脚本: {}", s);
    }

    let client = IpcClient::new();
    client.start_ipc(project, script, timeout).await?;

    println!("\n发送命令: moho-mate call '<lua>'");
    println!("关闭 Moho: moho-mate call 'moho_ipc.quit()'");

    Ok(())
}
