//! 全局配置常量

use std::path::PathBuf;

/// Moho 应用路径
pub const MOHO_APP: &str = "/Applications/Moho.app";

/// IPC Socket 路径
pub const SOCKET_PATH: &str = "/tmp/moho_ipc.sock";

/// IPC 命令目录
pub const IPC_CMD_DIR: &str = "/tmp/moho_ipc_cmds";

/// Moho 配置目录
pub fn moho_config_dir() -> PathBuf {
    PathBuf::from("/Users/def/Library/Preferences/Lost Marble/Moho Pro/14")
}

/// IPC 配置备份目录
pub const IPC_CONFIG_BACKUP: &str = "/tmp/moho_ipc_config_backup";

/// IPC 备份 PID 文件
pub const IPC_BACKUP_PID_FILE: &str = "/tmp/moho_ipc_backup.pid";

/// IPC 令牌文件
pub const IPC_TOKEN_FILE: &str = "/tmp/moho_ipc_token";

/// 脚本目录
pub fn scripts_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/def".to_string());
    PathBuf::from(home)
        .join(".openclaw/workspace/skills/moho-mate/scripts")
}

/// IPC Lua 模块路径
pub fn ipc_tool_path() -> PathBuf {
    scripts_dir().join("ipc/moho_ipc.lua")
}

/// 空配置模板
pub fn empty_config_template() -> PathBuf {
    scripts_dir().join("ipc/empty_config")
}
