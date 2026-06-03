//! moho-mate - Moho 命令行工具
//!
//! 功能:
//!   - IPC 服务: start, call, quit, status
//!   - 渲染编码: render, encode
//!   - 辅助工具: draw, inspect, config

use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing_subscriber::FmtSubscriber;

mod config;
mod ipc;
mod commands;

#[derive(Parser)]
#[command(name = "moho-mate")]
#[command(about = "Moho 命令行工具", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动 IPC 服务
    Start {
        /// 项目文件
        project: Option<String>,
        /// 脚本文件
        script: Option<String>,
        /// 超时秒数
        #[arg(short, long, default_value = "3600")]
        timeout: u32,
    },
    /// 发送 Lua 命令
    Call {
        /// Lua 代码
        code: Option<String>,
        /// Lua 文件
        #[arg(short, long)]
        file: Option<String>,
    },
    /// 退出 Moho
    Quit,
    /// IPC 状态
    Status,
    /// 渲染项目
    Render {
        /// 项目文件
        project: String,
        /// 输出格式 (PNG, JPEG, MP4, GIF, APNG)
        #[arg(short, long, default_value = "PNG")]
        format: String,
        /// 输出路径
        #[arg(short, long)]
        output: Option<String>,
        /// 起始帧
        #[arg(long, default_value = "0")]
        start: u32,
        /// 结束帧
        #[arg(long, default_value = "72")]
        end: u32,
    },
    /// 编码视频
    Encode {
        /// 输入路径
        input: String,
        /// 输出路径
        output: String,
        /// 帧率
        #[arg(long, default_value = "24")]
        fps: u32,
        /// CRF 质量
        #[arg(long, default_value = "23")]
        crf: u32,
    },
    /// 绘制形状 (不保存)
    Draw {
        /// 形状名称 (circle, bunny, puppy)
        shape: String,
    },
    /// 查看项目信息
    Inspect {
        /// 项目文件
        project: String,
    },
    /// 配置管理
    Config {
        /// 操作 (list, backup, restore)
        action: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set tracing subscriber");

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { project, script, timeout } => {
            commands::start::execute(project.as_deref(), script.as_deref(), timeout).await?;
        }
        Commands::Call { code, file } => {
            commands::call::execute(code.as_deref(), file.as_deref()).await?;
        }
        Commands::Quit => {
            commands::quit::execute().await?;
        }
        Commands::Status => {
            commands::status::execute()?;
        }
        Commands::Render { project, format, output, start, end } => {
            commands::render::execute(&project, &format, output.as_deref(), start, end).await?;
        }
        Commands::Encode { input, output, fps, crf } => {
            commands::encode::execute(&input, &output, fps, crf).await?;
        }
        Commands::Draw { shape } => {
            commands::draw::execute(&shape).await?;
        }
        Commands::Inspect { project } => {
            commands::inspect::execute(&project)?;
        }
        Commands::Config { action } => {
            commands::config::execute(&action)?;
        }
    }

    Ok(())
}
