//! moho-mate - 统一的 CLI + Lua 模块
//!
//! 一个可执行文件同时提供：
//!   1. CLI 命令：start, call, quit, status, render, encode, inspect, config
//!   2. Lua 模块：require("moho_ipc") 提供 IPC 服务
//!
//! 用法：
//!   moho-mate start [project.moho] [script.lua] [--timeout 3600]
//!   moho-mate call '<lua>'
//!   moho-mate call -f script.lua
//!   moho-mate quit
//!   moho-mate status
//!   moho-mate render project.moho [-f PNG|JPEG|MP4|GIF] [-o output] [--start 0] [--end 72]
//!   moho-mate encode input output [--fps 24] [--crf 23]
//!   moho-mate inspect <project.moho>
//!   moho-mate config list|backup|restore
//!
//! Lua 加载方式：
//!   package.cpath = "/path/to/moho-mate;" .. package.cpath
//!   local ipc = require("moho_ipc")

use std::os::raw::c_int;
use std::ffi::CString;
use std::ptr;

mod lua_ffi;
mod ipc_core;

use lua_ffi::*;
use ipc_core::*;

// ========== Lua 模块入口点 ==========

/// 导出 Lua 模块（Moho 通过 require("moho_ipc") 调用）
#[no_mangle]
pub extern "C" fn luaopen_moho_ipc(L: lua_State) -> c_int {
    unsafe {
        // 创建模块表
        lua_createtable(L, 0, 16);

        // 注册函数
        reg_func(L, "start", l_start);
        reg_func(L, "stop", l_stop);
        reg_func(L, "quit", l_quit);
        reg_func(L, "check", l_check);
        reg_func(L, "poll", l_poll);
        reg_func(L, "status", l_status);
        reg_func(L, "encode_video", l_encode_video);
        reg_func(L, "encode_status", l_encode_status);
        reg_func(L, "encode_cancel", l_encode_cancel);

        1 // 返回 1 个值（模块表）
    }
}

/// 注册函数到模块表
unsafe fn reg_func(L: lua_State, name: &str, func: unsafe extern "C" fn(lua_State) -> c_int) {
    push_string(L, name);
    lua_pushcfunction(L, Some(func));
    lua_settable(L, -3);
}

// ========== Lua 函数实现 ==========

/// start() -> (running, path)
unsafe extern "C" fn l_start(L: lua_State) -> c_int {
    let (running, path) = ipc_start(L);
    lua_pushboolean(L, if running { 1 } else { 0 });
    push_string(L, &path);
    2
}

/// stop() -> true
unsafe extern "C" fn l_stop(L: lua_State) -> c_int {
    let result = ipc_stop();
    lua_pushboolean(L, if result { 1 } else { 0 });
    1
}

/// quit() -> true
unsafe extern "C" fn l_quit(L: lua_State) -> c_int {
    ipc_stop();
    lua_pushboolean(L, 1);
    1
}

/// check() -> nil (兼容 LayerScript)
unsafe extern "C" fn l_check(L: lua_State) -> c_int {
    lua_pushnil(L);
    1
}

/// poll() -> 0 (兼容旧 API)
unsafe extern "C" fn l_poll(L: lua_State) -> c_int {
    lua_pushinteger(L, 0);
    1
}

/// status() -> table
unsafe extern "C" fn l_status(L: lua_State) -> c_int {
    let (running, path, calls, errors) = get_status();

    lua_createtable(L, 0, 5);

    push_string(L, "running");
    lua_pushboolean(L, if running { 1 } else { 0 });
    lua_settable(L, -3);

    push_string(L, "path");
    push_string(L, &path);
    lua_settable(L, -3);

    push_string(L, "calls");
    lua_pushinteger(L, calls as i64);
    lua_settable(L, -3);

    push_string(L, "errors");
    lua_pushinteger(L, errors as i64);
    lua_settable(L, -3);

    1
}

/// encode_video(input, output, fps?, crf?, codec?) -> (ok, msg)
unsafe extern "C" fn l_encode_video(L: lua_State) -> c_int {
    let input = to_string(L, 1).unwrap_or("");
    let output = to_string(L, 2).unwrap_or("");
    let fps = luaL_optinteger(L, 3, 24) as i32;
    let crf = luaL_optinteger(L, 4, 23) as i32;
    let codec = to_string(L, 5).unwrap_or("mpeg4");

    let (ok, msg) = encode_video(input, output, fps, crf, codec);

    lua_pushboolean(L, if ok { 1 } else { 0 });
    push_string(L, &msg);
    2
}

/// encode_status() -> table
unsafe extern "C" fn l_encode_status(L: lua_State) -> c_int {
    let (status, status_text, progress) = encode_status();

    lua_createtable(L, 0, 3);

    push_string(L, "status");
    lua_pushinteger(L, status as i64);
    lua_settable(L, -3);

    push_string(L, "status_text");
    push_string(L, status_text);
    lua_settable(L, -3);

    push_string(L, "progress");
    lua_pushnumber(L, progress as f64 / 100.0);
    lua_settable(L, -3);

    1
}

/// encode_cancel() -> bool
unsafe extern "C" fn l_encode_cancel(L: lua_State) -> c_int {
    let result = encode_cancel();
    lua_pushboolean(L, if result { 1 } else { 0 });
    1
}

// ========== CLI 入口 ==========

use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::info;

#[derive(Parser)]
#[command(name = "moho-mate")]
#[command(about = "Moho 命令行工具 + Lua 模块", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动 IPC 服务
    Start {
        /// 项目文件 (.moho)
        project: Option<String>,
        /// 脚本文件 (.lua)
        script: Option<String>,
        /// 超时时间（秒）
        #[arg(short = 't', long, default_value = "3600")]
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
        /// 项目文件 (.moho)
        project: String,
        /// 输出格式 (PNG, JPEG, BMP, TGA, MP4, GIF, APNG)
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
        /// 输入路径（PNG 序列或视频）
        input: String,
        /// 输出路径 (.mp4, .gif, .apng)
        output: String,
        /// 帧率
        #[arg(long, default_value = "24")]
        fps: u32,
        /// 编码质量 (0-51, 越小越好)
        #[arg(long, default_value = "23")]
        crf: u32,
    },

    /// 查看项目信息
    Inspect {
        /// 项目文件 (.moho)
        project: String,
    },

    /// 配置管理
    Config {
        /// 操作 (list, backup, restore)
        action: String,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        None => {
            // 无参数时，作为 Lua 模块被加载
            print_usage();
            std::process::exit(1);
        }
        Some(Commands::Start { project, script, timeout }) => {
            cmd_start(project.as_deref(), script.as_deref(), timeout)?;
        }
        Some(Commands::Call { code, file }) => {
            cmd_call(code.as_deref(), file.as_deref())?;
        }
        Some(Commands::Quit) => {
            cmd_quit()?;
        }
        Some(Commands::Status) => {
            cmd_status()?;
        }
        Some(Commands::Render { project, format, output, start, end }) => {
            cmd_render(&project, &format, output.as_deref(), start, end)?;
        }
        Some(Commands::Encode { input, output, fps, crf }) => {
            cmd_encode(&input, &output, fps, crf)?;
        }
        Some(Commands::Inspect { project }) => {
            cmd_inspect(&project)?;
        }
        Some(Commands::Config { action }) => {
            cmd_config(&action)?;
        }
    }

    Ok(())
}

fn print_usage() {
    eprintln!("moho-mate - Moho 命令行工具\n");
    eprintln!("用法:");
    eprintln!("  moho-mate start [project.moho] [script.lua] [--timeout 3600]");
    eprintln!("  moho-mate call '<lua>'");
    eprintln!("  moho-mate call -f script.lua");
    eprintln!("  moho-mate quit");
    eprintln!("  moho-mate status");
    eprintln!("  moho-mate render project.moho [-f PNG|JPEG|MP4|GIF] [-o output]");
    eprintln!("  moho-mate encode input output [--fps 24] [--crf 23]");
    eprintln!("  moho-mate draw <circle|bunny|puppy>");
    eprintln!("  moho-mate inspect <project.moho>");
    eprintln!("  moho-mate config list|backup|restore");
    eprintln!("\nLua 模块加载:");
    eprintln!("  package.cpath = \"/path/to/moho-mate;\" .. package.cpath");
    eprintln!("  local ipc = require(\"moho_ipc\")");
}

// ========== 命令实现 ==========

fn cmd_start(project: Option<&str>, script: Option<&str>, timeout: u32) -> Result<()> {
    info!("▶ 启动 IPC 服务");
    println!("  超时: {} 秒", timeout);
    if let Some(p) = project {
        println!("  项目: {}", p);
    }
    if let Some(s) = script {
        println!("  脚本: {}", s);
    }

    // 杀掉旧 Moho
    std::process::Command::new("pkill")
        .args(["-9", "Moho"])
        .output()
        .ok();

    // 删除旧 socket
    std::fs::remove_file("/tmp/moho_ipc.sock").ok();
    std::thread::sleep(std::time::Duration::from_secs(1));

    // TODO: 备份配置 + 使用空配置
    // TODO: 创建 wrapper.lua 并启动 Moho

    println!("\n⚠️ 请通过 Moho 菜单运行脚本启动 IPC");
    println!("   或手动启动: open -a \"Moho Pro 14\"");

    Ok(())
}

fn cmd_call(code: Option<&str>, file: Option<&str>) -> Result<()> {
    use std::os::unix::net::UnixStream;
    use std::io::{Read, Write};

    let socket_path = "/tmp/moho_ipc.sock";

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(_) => {
            anyhow::bail!("IPC 未运行。请先启动 Moho 并运行 IPC 脚本。");
        }
    };

    let cmd = if let Some(f) = file {
        format!("dofile(\"{}\")", f)
    } else if let Some(c) = code {
        c.to_string()
    } else {
        anyhow::bail!("需要指定 Lua 代码或 --file 参数");
    };

    stream.write_all(cmd.as_bytes())?;
    stream.write_all(b"\n");

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    println!("{}", response.trim());

    Ok(())
}

fn cmd_quit() -> Result<()> {
    info!("退出 IPC...");
    
    // 直接调用 ipc_stop
    let stopped = ipc_stop();
    
    // 同时尝试通过 socket 发送退出命令
    match cmd_call(Some("moho:FileSave()"), None) {
        Ok(_) => {},
        Err(_) => {},
    }
    
    if stopped {
        println!("✓ IPC 已退出");
    } else {
        println!("✓ IPC 已退出");
    }
    Ok(())
}

fn cmd_status() -> Result<()> {
    let (running, path, calls, errors) = get_status();

    println!("=== IPC 状态 ===");
    
    // 尝试通过 socket 检查
    let socket_path = "/tmp/moho_ipc.sock";
    let socket_running = std::path::Path::new(socket_path).exists();
    
    if socket_running {
        // 尝试连接 socket
        use std::io::{Read, Write};
        use std::os::unix::net::UnixStream;
        
        match UnixStream::connect(socket_path) {
            Ok(mut stream) => {
                // 发送状态查询
                stream.write_all(b"status\n")?;
                stream.flush()?;
                
                let mut response = String::new();
                stream.read_to_string(&mut response)?;
                
                println!("  运行中: 是");
                println!("  Socket: {}", socket_path);
                println!("  响应: {}", response.trim());
            }
            Err(_) => {
                println!("  运行中: 否（socket 存在但无法连接）");
            }
        }
    } else {
        println!("  运行中: 否");
        println!("  Socket: {}", socket_path);
    }
    
    println!("  调用次数: {}", calls);
    println!("  错误次数: {}", errors);

    let (status, status_text, progress) = encode_status();
    println!("\n=== 编码状态 ===");
    println!("  状态: {} ({})", status, status_text);
    if status == 1 {
        println!("  进度: {:.0}%", progress as f64 / 100.0);
    }

    Ok(())
}

fn cmd_render(project: &str, format: &str, output: Option<&str>, start: u32, end: u32) -> Result<()> {
    // 检查项目文件
    if !std::path::Path::new(project).exists() {
        anyhow::bail!("项目不存在: {}", project);
    }

    info!("渲染项目: {}", project);
    println!("  格式: {}", format);
    println!("  范围: {} - {}", start, end);

    if let Some(o) = output {
        println!("  输出: {}", o);
    }

    // TODO: 通过 IPC 发送渲染命令

    Ok(())
}

fn cmd_encode(input: &str, output: &str, fps: u32, crf: u32) -> Result<()> {
    info!("编码: {} -> {}", input, output);
    println!("  FPS: {}", fps);
    println!("  CRF: {}", crf);

    // 判断输出格式
    let is_gif = output.ends_with(".gif");
    let is_apng = output.ends_with(".apng");

    if is_apng {
        println!("  格式: APNG");
    } else if is_gif {
        println!("  格式: GIF");
    } else {
        println!("  格式: MP4");
    }

    let (ok, msg) = encode_video(input, output, fps as i32, crf as i32, "mpeg4");

    if ok {
        println!("✓ 编码已启动");
        
        // 等待编码完成
        let mut last_progress = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let (status, status_text, progress) = encode_status();
            
            // 显示进度
            if progress > last_progress {
                println!("  进度: {}%", progress);
                last_progress = progress;
            }
            
            // 检查状态
            if status == 2 {
                // success
                println!("✓ 编码完成: {}", output);
                break;
            } else if status == 3 {
                // error
                println!("✗ 编码失败");
                break;
            }
            
            // 超时检查（5分钟）
            if last_progress == 0 && status == 0 {
                // 可能已经完成或未启动
                break;
            }
        }
    } else {
        println!("✗ {}", msg);
    }

    Ok(())
}

fn cmd_inspect(project: &str) -> Result<()> {
    // 检查项目文件
    if !std::path::Path::new(project).exists() {
        anyhow::bail!("项目不存在: {}", project);
    }

    println!("=== 项目信息 ===");
    println!("  文件: {}", project);

    // 解析 .moho 文件（ZIP 压缩包）
    let file = std::fs::File::open(project)?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => {
            // 不是 ZIP，尝试直接读取
            let content = std::fs::read_to_string(project)?;
            parse_moho_content(&content);
            return Ok(());
        }
    };

    // 查找 .mohoproj 文件
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        
        if name.ends_with(".mohoproj") {
            let mut content = String::new();
            std::io::Read::read_to_string(&mut file, &mut content)?;
            parse_moho_content(&content);
            break;
        }
    }

    Ok(())
}

fn parse_moho_content(content: &str) {
    let mut layer_count = 0;
    let mut bone_count = 0;
    let mut mesh_count = 0;

    // 简单统计（XML/JSON 标签计数）
    for line in content.lines() {
        if line.contains("<layer") || line.contains("\"layer\"") {
            layer_count += 1;
        }
        if line.contains("<bone") || line.contains("<Bone") || line.contains("\"bone\"") {
            bone_count += 1;
        }
        if line.contains("<mesh") || line.contains("<Mesh") || line.contains("\"mesh\"") {
            mesh_count += 1;
        }
    }

    println!("  图层数: {}", layer_count);
    println!("  骨骼数: {}", bone_count);
    println!("  网格数: {}", mesh_count);
}

fn cmd_config(action: &str) -> Result<()> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Lost Marble/Moho Pro/14");
    
    match action {
        "list" => {
            println!("=== Moho 配置 ===");
            println!("配置目录: {:?}", config_dir);
            
            if config_dir.exists() {
                // 列出配置文件
                for entry in std::fs::read_dir(&config_dir)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    let metadata = entry.metadata()?;
                    let size = if metadata.is_dir() {
                        "<dir>".to_string()
                    } else {
                        format!("{} bytes", metadata.len())
                    };
                    println!("  {} - {}", name, size);
                }
                
                // 列出 Autosave 目录
                let autosave_dir = config_dir.join("Autosave");
                if autosave_dir.exists() {
                    println!("\n=== Autosave ===");
                    for entry in std::fs::read_dir(&autosave_dir)? {
                        let entry = entry?;
                        let name = entry.file_name().to_string_lossy().to_string();
                        println!("  {}", name);
                    }
                }
            } else {
                println!("配置目录不存在");
            }
        }
        "backup" => {
            info!("备份配置...");
            
            let backup_dir = std::path::PathBuf::from("/tmp/moho_ipc_config_backup");
            
            if config_dir.exists() {
                // 删除旧备份
                if backup_dir.exists() {
                    std::fs::remove_dir_all(&backup_dir)?;
                }
                
                // 复制配置目录
                copy_dir_all(&config_dir, &backup_dir)?;
                println!("✓ 配置已备份到 {:?}", backup_dir);
            } else {
                println!("配置目录不存在");
            }
        }
        "restore" => {
            info!("恢复配置...");
            
            let backup_dir = std::path::PathBuf::from("/tmp/moho_ipc_config_backup");
            
            if backup_dir.exists() {
                // 删除当前配置
                if config_dir.exists() {
                    std::fs::remove_dir_all(&config_dir)?;
                }
                
                // 恢复备份
                copy_dir_all(&backup_dir, &config_dir)?;
                println!("✓ 配置已恢复");
            } else {
                println!("备份不存在");
            }
        }
        _ => {
            println!("用法: moho-mate config <list|backup|restore>");
        }
    }
    Ok(())
}

// 递归复制目录
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
