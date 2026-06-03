//! status 命令 - IPC 状态

use anyhow::Result;

use crate::ipc::IpcClient;
use crate::config;

pub fn execute() -> Result<()> {
    let client = IpcClient::new();

    if client.is_running() {
        println!("✓ IPC 运行中");
        println!("  Socket: {}", config::SOCKET_PATH);
    } else {
        println!("✗ IPC 未启动");
        println!("先用: moho-mate start");
    }

    Ok(())
}
