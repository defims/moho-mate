//! quit 命令 - 退出 Moho

use anyhow::Result;
use tracing::info;

use crate::ipc::IpcClient;

pub async fn execute() -> Result<()> {
    info!("▶ 退出 Moho");

    let client = IpcClient::new();
    client.quit().await?;

    println!("✓ Moho 已退出");

    Ok(())
}