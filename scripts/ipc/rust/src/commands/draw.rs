//! draw 命令 - 绘制形状（不保存）

use anyhow::{anyhow, Result};
use tracing::info;

use crate::ipc::IpcClient;

pub async fn execute(shape: &str) -> Result<()> {
    // 检查支持的形状
    if !matches!(shape, "circle" | "bunny" | "puppy") {
        return Err(anyhow!("未知形状: {}\n可用形状: circle, bunny, puppy", shape));
    }

    info!("▶ 绘制形状: {}", shape);
    println!("⚠️ draw 只绘制，不保存。请手动 Cmd+S");

    let client = IpcClient::new();

    // 使用 draw_ipc.lua 脚本
    let draw_lua = format!(
        r#"dofile(IPC_DIR .. '/../draw_ipc.lua')
draw_shape('{}')"#,
        shape
    );

    client.send_multiline(&draw_lua).await?;

    println!("✓ 已绘制 {}，请手动保存", shape);

    Ok(())
}
