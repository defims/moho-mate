//! call 命令 - 发送 Lua 命令

use anyhow::{anyhow, Result};
use tracing::info;

use crate::ipc::IpcClient;

pub async fn execute(code: Option<&str>, file: Option<&str>) -> Result<()> {
    let client = IpcClient::new();

    if let Some(filepath) = file {
        info!("▶ 发送 Lua 文件: {}", filepath);
        let response = client.send_file(filepath).await?;
        println!("{}", response);
        return Ok(());
    }

    if let Some(code) = code {
        info!("▶ 发送 Lua 命令");

        // 判断是否多行
        if code.contains('\n') {
            let response = client.send_multiline(code).await?;
            println!("{}", response);
        } else {
            let response = client.send_auto(code).await?;
            println!("{}", response);
        }
        return Ok(());
    }

    Err(anyhow!("用法: moho-mate call '<lua>' 或 -f script.lua"))
}
