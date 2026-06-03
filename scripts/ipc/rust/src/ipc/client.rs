//! IPC 客户端 - 发送命令到 Moho

use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::fs::PermissionsExt; // for from_mode
use std::path::Path;
use std::time::Duration;
use std::fs;
use std::process::Command;
use tracing::info;

use crate::config;

/// IPC 客户端
pub struct IpcClient {
    socket_path: String,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            socket_path: config::SOCKET_PATH.to_string(),
        }
    }

    /// 检查 IPC 是否运行
    pub fn is_running(&self) -> bool {
        Path::new(&self.socket_path).exists()
    }

    /// 发送命令（自动启动 IPC）
    pub async fn send_auto(&self, cmd: &str) -> Result<String> {
        if !self.is_running() {
            info!("IPC 未启动，自动启动...");
            self.start_ipc(None, None, 3600).await?;
        }
        self.send(cmd).await
    }

    /// 发送命令
    pub async fn send(&self, cmd: &str) -> Result<String> {
        let sock = UnixStream::connect(&self.socket_path)
            .map_err(|e| anyhow!("IPC 连接失败(服务未启动?): {}", e))?;

        sock.set_read_timeout(Some(Duration::from_secs(60)))?;
        sock.set_write_timeout(Some(Duration::from_secs(10)))?;

        let mut sock = sock;

        // 发送命令
        sock.write_all(cmd.as_bytes())?;
        sock.write_all(b"\n");

        // 接收响应
        let mut response = String::new();
        sock.read_to_string(&mut response)?;

        // 解析响应
        let response = response.trim();
        if let Some(result) = response.strip_prefix("ok|") {
            Ok(result.to_string())
        } else if let Some(err) = response.strip_prefix("error|") {
            Err(anyhow!("IPC 错误: {}", err))
        } else {
            Ok(response.to_string())
        }
    }

    /// 发送文件
    pub async fn send_file(&self, filepath: &str) -> Result<String> {
        let path = Path::new(filepath);
        if !path.exists() {
            return Err(anyhow!("文件不存在: {}", filepath));
        }

        let cmd = format!("dofile(\"{}\")", filepath);
        self.send(&cmd).await
    }

    /// 发送多行代码（写入临时文件）
    pub async fn send_multiline(&self, code: &str) -> Result<String> {
        // 创建临时目录
        fs::create_dir_all(config::IPC_CMD_DIR)?;

        let tmpfile = Path::new(config::IPC_CMD_DIR).join("cmd.lua");
        fs::write(&tmpfile, code)?;

        self.send_file(tmpfile.to_str().unwrap()).await
    }

    /// 启动 IPC 服务
    pub async fn start_ipc(&self, project: Option<&str>, script: Option<&str>, timeout: u32) -> Result<()> {
        info!("启动 IPC 服务...");

        // 杀掉旧 Moho
        let _ = Command::new("pkill")
            .args(["-9", "Moho"])
            .output();

        // 删除旧 socket
        let _ = fs::remove_file(&self.socket_path);
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 备份配置
        self.backup_config()?;

        // 创建临时 wrapper
        fs::create_dir_all(config::IPC_CMD_DIR)?;
        let wrapper_path = Path::new(config::IPC_CMD_DIR).join("wrapper.lua");

        // 生成启动令牌
        let token = format!("{}_{}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            std::process::id(),
            rand_token()
        );

        // 写入令牌文件
        fs::write(config::IPC_TOKEN_FILE, &token)?;
        fs::set_permissions(config::IPC_TOKEN_FILE, fs::Permissions::from_mode(0o600))?;

        // 写入 wrapper
        let wrapper_content = format!(
            r#"IPC_DIR = "{}"
USER_PROJECT = "{}"
USER_SCRIPT = "{}"
IPC_TIMEOUT = {}
IPC_START_TOKEN = "{}"
dofile("{}")
"#,
            config::scripts_dir().display(),
            project.unwrap_or(""),
            script.unwrap_or(""),
            timeout,
            token,
            config::ipc_tool_path().display()
        );
        fs::write(&wrapper_path, wrapper_content)?;

        // 启动 Moho
        Command::new("open")
            .args(["-a", "Moho", "--args"])
            .arg(&wrapper_path)
            .spawn()?;

        // 等待 socket
        for i in 0..30 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if self.is_running() {
                info!("✓ IPC 服务已启动");
                tokio::time::sleep(Duration::from_secs(1)).await; // 等待就绪
                return Ok(());
            }
            if i % 5 == 0 {
                info!("等待 IPC socket... ({}/30)", i + 1);
            }
        }

        Err(anyhow!("IPC 启动超时"))
    }

    /// 备份 Moho 配置
    fn backup_config(&self) -> Result<()> {
        use std::process::Command;

        // 清理旧备份
        let _ = Command::new("rm")
            .args(["-rf", config::IPC_CONFIG_BACKUP])
            .output();

        // 创建备份目录
        fs::create_dir_all(config::IPC_CONFIG_BACKUP)?;

        // 备份配置
        let status = Command::new("cp")
            .args(["-R", &format!("{}/", config::moho_config_dir().display())])
            .arg(config::IPC_CONFIG_BACKUP)
            .status()?;

        if status.success() {
            info!("✓ 配置已备份");
            // 写入 PID 文件
            fs::write(config::IPC_BACKUP_PID_FILE, format!("{}\n", std::process::id()))?;
        }

        Ok(())
    }

    /// 恢复 Moho 配置
    fn restore_config(&self) -> Result<()> {
        use std::process::Command;

        // 检查 PID 文件
        if !Path::new(config::IPC_BACKUP_PID_FILE).exists() {
            info!("无 IPC 会话标记，跳过恢复");
            return Ok(());
        }

        // 检查备份目录
        if !Path::new(config::IPC_CONFIG_BACKUP).exists() {
            info!("无配置备份，跳过恢复");
            let _ = fs::remove_file(config::IPC_BACKUP_PID_FILE);
            return Ok(());
        }

        // 恢复配置
        let status = Command::new("cp")
            .args(["-R", &format!("{}/", config::IPC_CONFIG_BACKUP)])
            .arg(config::moho_config_dir())
            .status()?;

        if status.success() {
            info!("✓ 配置已恢复");
            // 清理备份
            let _ = Command::new("rm").args(["-rf", config::IPC_CONFIG_BACKUP]).output();
            let _ = fs::remove_file(config::IPC_BACKUP_PID_FILE);
        }

        Ok(())
    }

    /// 发送退出命令
    pub async fn quit(&self) -> Result<()> {
        if !self.is_running() {
            info!("Moho 未运行");
            return Ok(());
        }

        info!("退出 Moho...");
        let _ = self.send("moho_ipc.quit()").await;

        // 等待 socket 断开
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !self.is_running() {
                break;
            }
        }

        // 恢复配置
        self.restore_config()?;

        Ok(())
    }
}

/// 生成随机令牌
fn rand_token() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (ts as u32) % 10000
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}
