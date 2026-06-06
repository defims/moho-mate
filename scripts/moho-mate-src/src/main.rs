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
mod ffmpeg_ffi;
mod encode_native;

use lua_ffi::*;
use ipc_core::*;

#[cfg(feature = "ffmpeg-builtin")]

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

/// 从 Lua 栈获取字符串
unsafe fn get_string(L: lua_State, idx: c_int) -> String {
    let s = luaL_checklstring(L, idx, std::ptr::null_mut());
    if s.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(s).to_string_lossy().to_string()
    }
}

// ========== Lua 函数实现 ==========

/// start() -> (running, path)
unsafe extern "C" fn l_start(L: lua_State) -> c_int {
    let (running, path) = ipc_start(L, None);
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
    let (status, status_text, progress, error_msg) = encode_status();

    lua_createtable(L, 0, 4);

    push_string(L, "status");
    lua_pushinteger(L, status as i64);
    lua_settable(L, -3);

    push_string(L, "status_text");
    push_string(L, status_text);
    lua_settable(L, -3);

    push_string(L, "progress");
    lua_pushnumber(L, progress as f64 / 100.0);
    lua_settable(L, -3);
    
    push_string(L, "error_msg");
    push_string(L, &error_msg);
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
use std::io::Write; // for flush()

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

    // 1. 杀掉旧 Moho
    info!("关闭旧 Moho...");
    std::process::Command::new("pkill")
        .args(["-9", "Moho"])
        .output()
        .ok();
    std::thread::sleep(std::time::Duration::from_secs(2));

    // 2. 删除旧 socket
    let _ = std::fs::remove_file("/tmp/moho_ipc.sock");
    let _ = std::fs::remove_file("/tmp/moho_ipc_token");
    let _ = std::fs::remove_file("/tmp/moho_ipc_owner");

    // 3. 备份配置 + 清空 Autosave
    backup_moho_config()?;

    // 4. 生成启动令牌
    let token: u32 = rand();
    let token_str = format!("{}", token);
    std::fs::write("/tmp/moho_ipc_token", &token_str)?;

    // 5. 创建 wrapper.lua
    let ipc_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取 IPC 目录"))?
        .to_string_lossy()
        .to_string();

    let wrapper_content = generate_wrapper_lua(&ipc_dir, &token_str, project, script, timeout);
    let wrapper_path = "/tmp/moho_wrapper.lua";
    std::fs::write(wrapper_path, wrapper_content)?;

    info!("创建 wrapper.lua: {}", wrapper_path);

    // 6. 用 open -a Moho --args 启动（避免直接运行二进制文件导致崩溃）
    info!("启动 Moho...");
    
    let child = std::process::Command::new("open")
        .args(["-a", "Moho", "--args", wrapper_path])
        .spawn()?;

    info!("Moho 已启动");

    // 7. 等待 IPC 就绪
    println!("\n等待 IPC 就绪...");
    let socket_path = "/tmp/moho_ipc.sock";
    let max_wait = std::cmp::min(timeout, 30); // 最多等待30秒
    let start_time = std::time::Instant::now();

    let mut ipc_ready = false;
    while start_time.elapsed().as_secs() < max_wait as u64 {
        if std::path::Path::new(socket_path).exists() {
            // 尝试连接测试
            std::thread::sleep(std::time::Duration::from_millis(500));
            
            if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket_path) {
                use std::io::Write;
                let _ = stream.write_all(b"print('ipc_ready')\n");
                ipc_ready = true;
                break;
            }
        }
        print!(".");
        std::io::stdout().flush().ok();
        std::thread::sleep(std::time::Duration::from_millis(500));
    };

    if ipc_ready {
        // 8. IPC 就绪后立即恢复配置（让用户下次正常启动 Moho 不受影响）
        info!("恢复配置...");
        if let Err(e) = restore_moho_config() {
            println!("⚠ 配置恢复失败: {}", e);
        } else {
            println!("✓ 配置已恢复");
        }
        
        println!("\n✓ IPC 服务已就绪: {}", socket_path);
        println!("  超时: {} 秒", timeout);
        return Ok(());
    }

    println!("\n⚠️ IPC 启动超时");
    println!("  请检查 Moho 是否正常运行");
    println!("  日志: /tmp/moho_ipc.log");

    // 超时时也尝试恢复配置
    let _ = restore_moho_config();

    Ok(())
}

/// Moho 配置目录（macOS: ~/Library/Preferences/Lost Marble/Moho Pro/14）
/// 注意：dirs crate 在 macOS 上返回 Application Support 而非 Preferences
fn moho_config_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow::anyhow!("无法获取 HOME 环境变量"))?;
    let config_dir = std::path::PathBuf::from(home)
        .join("Library/Preferences/Lost Marble/Moho Pro/14");
    Ok(config_dir)
}

/// 备份 Moho 配置
fn backup_moho_config() -> Result<()> {
    // macOS: ~/Library/Preferences/Lost Marble/Moho Pro/14/
    let config_dir = moho_config_dir()?;

    if !config_dir.exists() {
        info!("配置目录不存在，跳过备份");
        return Ok(());
    }

    let backup_dir = std::path::PathBuf::from("/tmp/moho_ipc_config_backup");
    
    // 删除旧备份
    if backup_dir.exists() {
        std::fs::remove_dir_all(&backup_dir)?;
    }

    // 复制配置
    copy_dir_all(&config_dir, &backup_dir)?;
    info!("配置已备份: {:?}", backup_dir);

    // 清空 Autosave 目录（防止之前项目污染）
    let autosave_dir = config_dir.join("Autosave");
    info!("Autosave 目录: {:?}", autosave_dir);
    if autosave_dir.exists() {
        info!("开始删除 Autosave...");
        if let Err(e) = std::fs::remove_dir_all(&autosave_dir) {
            info!("删除失败: {}", e);
        } else {
            info!("删除成功");
        }
        if let Err(e) = std::fs::create_dir_all(&autosave_dir) {
            info!("创建失败: {}", e);
        }
        info!("Autosave 已清空");
    } else {
        info!("Autosave 目录不存在");
    }

    Ok(())
}

/// 恢复 Moho 配置
fn restore_moho_config() -> Result<()> {
    let config_dir = moho_config_dir()?;
    
    let backup_dir = std::path::PathBuf::from("/tmp/moho_ipc_config_backup");
    
    if !backup_dir.exists() {
        info!("备份不存在，跳过恢复");
        return Ok(());
    }
    
    // 删除当前配置中的 Autosave（保留 IPC 运行时产生的 autosave）
    let autosave_dir = config_dir.join("Autosave");
    if autosave_dir.exists() {
        std::fs::remove_dir_all(&autosave_dir)?;
    }
    
    // 从备份恢复 Autosave
    let backup_autosave = backup_dir.join("Autosave");
    if backup_autosave.exists() {
        copy_dir_all(&backup_autosave, &autosave_dir)?;
    } else {
        std::fs::create_dir_all(&autosave_dir)?;
    }
    
    info!("配置已恢复");
    Ok(())
}

/// 生成 wrapper.lua 内容
fn generate_wrapper_lua(ipc_dir: &str, token: &str, project: Option<&str>, script: Option<&str>, timeout: u32) -> String {
    format!(r#"
-- wrapper.lua - IPC 启动包装脚本
-- 由 moho-mate start 自动生成

local IPC_DIR = "{}"
local MOHO_MATE_EXE = IPC_DIR .. "/moho-mate"

-- 启动令牌
IPC_START_TOKEN = "{}"
IPC_DIR = IPC_DIR
USER_PROJECT = {}
USER_SCRIPT = {}
IPC_TIMEOUT = {}

-- 加载 IPC 模块（从 moho-mate 可执行文件加载）
-- macOS: 可执行文件可以被 dlopen 加载（需要 export_dynamic）
package.cpath = MOHO_MATE_EXE .. ";" .. package.cpath
package.loaded["moho_ipc"] = nil

local ok, ipc = pcall(require, "moho_ipc")
if not ok then
    print("✗ 加载 IPC 模块失败: " .. tostring(ipc))
    return
end

-- 存到 registry 和全局变量
local registry = debug.getregistry()
registry._ipc_module = ipc
_G.moho_ipc = ipc

function MohoScript(moho)
    print("=== IPC 启动 ===")
    
    -- 启动 socket
    local running, path = ipc.start()
    if not running then
        print("✗ Socket 启动失败")
        return
    end
    print("✓ Socket: " .. tostring(path))

    -- 创建/打开项目
    if USER_PROJECT and USER_PROJECT ~= "" then
        -- 检查文件是否存在，避免弹出 GUI 阻塞 IPC
        local file = io.open(USER_PROJECT, "r")
        if file then
            file:close()
            print("[项目] 打开: " .. USER_PROJECT)
            moho:FileOpen(USER_PROJECT)
        else
            print("✗ 文件不存在: " .. USER_PROJECT)
            return
        end
    else
        print("[项目] 新建文档")
        moho:FileNew()
    end

    _G.moho = moho
    print("✓ moho 已就绪")

    -- 执行用户脚本
    if USER_SCRIPT and USER_SCRIPT ~= "" then
        print("[脚本] 执行: " .. USER_SCRIPT)
        local ok, err = pcall(dofile, USER_SCRIPT)
        if not ok then
            print("✗ 脚本错误: " .. tostring(err))
        else
            print("✓ 脚本完成")
        end
    end

    print("=== IPC 运行中 (超时: " .. IPC_TIMEOUT .. "s) ===")
end
"#,
        ipc_dir,
        token,
        project.map(|s| format!("\"{}\"", s)).unwrap_or("nil".to_string()),
        script.map(|s| format!("\"{}\"", s)).unwrap_or("nil".to_string()),
        timeout
    )
}

/// 查找 Moho 应用
fn find_moho_app() -> Result<String> {
    // 尝试多个可能的路径
    let candidates = vec![
        "/Applications/Moho.app/Contents/MacOS/Moho",
        "/Applications/Moho Pro 14.app/Contents/MacOS/Moho Pro 14",
        "/Applications/Moho Pro 13.app/Contents/MacOS/Moho Pro 13",
    ];

    for path in candidates {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    // 使用默认的 Moho.app
    Ok("/Applications/Moho.app/Contents/MacOS/Moho".to_string())
}

/// 简单的随机数生成
fn rand() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (duration.as_nanos() & 0xFFFFFFFF) as u32
}

fn cmd_call(code: Option<&str>, file: Option<&str>) -> Result<()> {
    use std::os::unix::net::UnixStream;
    use std::io::{Read, Write};
    use std::net::Shutdown;

    let socket_path = "/tmp/moho_ipc.sock";

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(_) => {
            anyhow::bail!("IPC 未运行。请先启动 Moho 并运行 IPC 脚本。");
        }
    };

    let cmd = if let Some(f) = file {
        // 读取文件内容并发送
        std::fs::read_to_string(f)?
    } else if let Some(c) = code {
        c.to_string()
    } else {
        anyhow::bail!("需要指定 Lua 代码或 --file 参数");
    };

    // 发送命令
    stream.write_all(cmd.as_bytes())?;
    stream.write_all(b"\n");
    
    // 关闭写入端，告诉服务端"发送完毕"
    // 这样服务端会在响应完成后检测到 EOF 并断开连接
    stream.shutdown(Shutdown::Write)?;

    // 读取响应（服务端断开连接后 read_to_string 会返回）
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
                
                // 关闭写端，让服务端知道我们发完了
                use std::net::Shutdown;
                let _ = stream.shutdown(Shutdown::Write);
                
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

    let (status, status_text, progress, _error_msg) = encode_status();
    println!("\n=== 编码状态 ===");
    println!("  状态: {} ({})", status, status_text);
    if status == 1 {
        println!("  进度: {:.0}%", progress as f64 / 100.0);
    }

    Ok(())
}

fn cmd_render(project: &str, format: &str, output: Option<&str>, start: u32, end: u32) -> Result<()> {
    // 检查项目文件
    let project_path = std::path::Path::new(project);
    if !project_path.exists() {
        anyhow::bail!("项目不存在: {}", project);
    }
    
    // 解析格式和扩展名
    let (ext, is_video) = match format.to_uppercase().as_str() {
        "JPEG" | "JPG" => ("jpg", false),
        "BMP" => ("bmp", false),
        "TGA" => ("tga", false),
        "MP4" => ("mp4", true),
        "GIF" => ("gif", true),
        "APNG" => ("png", true),
        "QT" => ("mov", true),
        _ => ("png", false),
    };
    
    if is_video {
        let format_name = match format {
            "APNG" => "APNG(动画 PNG)",
            "QT" => "QuickTime",
            _ => format,
        };
        println!("▶ 渲染 + 编码: {}", format_name);
    }
    
    println!("▶ 渲染项目: {}", project);
    println!("  格式: {}", format);
    println!("  帧范围: {}-{}", start, end);
    
    // 生成输出路径
    let output_path = if let Some(o) = output {
        let mut path = o.to_string();
        // 检查是否需要添加后缀
        if is_video {
            let has_suffix = match format {
                "GIF" => path.ends_with(".gif"),
                "MP4" => path.ends_with(".mp4"),
                "APNG" => path.ends_with(".png") || path.ends_with(".apng"),
                "QT" => path.ends_with(".mov"),
                _ => true,
            };
            if !has_suffix {
                path.push_str(match format {
                    "APNG" => ".png",
                    "QT" => ".mov",
                    "GIF" => ".gif",
                    _ => ".mp4",
                });
                println!("  输出路径已修正: {}", path);
            }
        }
        path
    } else {
        // 从项目名生成输出名
        let base = project_path.file_stem().unwrap_or_default().to_string_lossy();
        if is_video {
            format!("/tmp/{}.{}", base, ext)
        } else {
            format!("/tmp/{}", base)
        }
    };
    
    if output.is_some() {
        println!("  输出: {}", output_path);
    }
    
    // 视频格式需要临时 PNG 目录
    let png_dir = if is_video {
        format!("/tmp/moho_render_frames_{}", std::process::id())
    } else {
        output_path.clone()
    };
    
    // 创建输出目录
    std::fs::create_dir_all(&png_dir)?;
    
    // 自动启动 IPC
    auto_start_ipc()?;
    
    // 检查当前项目是否已打开，避免重复 FileOpen 导致 Moho 崩溃
    let check_cmd = r#"
local doc = moho.document
if doc then
  local name = doc:Name()
  if name and name ~= '' then
    print('current_project:' .. name)
  else
    print('current_project:')
  end
else
  print('current_project:')
end"#;
    let current = ipc_send(check_cmd)?;
    let current_project = current.trim();
    
    // 获取目标项目名
    let project_name = std::path::Path::new(project)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    
    // 只有当前项目不是目标项目时才打开
    if !current_project.contains(project_name) {
        // Lua 脚本：检查文件存在 + FileOpen
        let open_cmd = format!(
            r#"local path = "{}"
local file = io.open(path, "r")
if file then
    file:close()
    moho:FileOpen(path)
else
    print('ERROR: 文件不存在: ' .. path)
end"#,
            project
        );
        ipc_send(&open_cmd)?;
        // 等待项目加载
        std::thread::sleep(std::time::Duration::from_millis(500));
    } else {
        println!("  项目已打开: {}", project_name);
    }
    
    // 渲染 PNG 序列（逐帧发送 IPC 命令，避免 Moho 崩溃）
    // 视频格式时始终渲染 PNG，然后再编码
    let render_ext = if is_video { "png" } else { ext };
    
    // 逐帧渲染
    for f in start..=end {
        let frame_path = format!("{}/frame_{:05}.{}", png_dir, f, render_ext);
        let render_cmd = format!(
            r#"moho:SetCurFrame({}, false, false)
moho:FileRender("{}")"#,
            f, frame_path
        );
        ipc_send(&render_cmd)?;
        
        // 显示进度
        if f == start || f == end {
            println!("  帧 {} → {}", f, frame_path);
        }
        
        // 延迟避免 Moho 崩溃
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    
    println!("✓ 渲染完成: {} 帧", end - start + 1);
    
    // 视频格式：调用编码
    if is_video {
        println!("✓ 序列已保存到: {}", png_dir);
        
        let codec = match format {
            "GIF" => "gif",
            "APNG" => "apng",
            "QT" => "mpeg4",
            _ => "mpeg4",
        };
        
        println!("▶ 编码 {}: {}", format, output_path);
        
        // Lua 脚本：同步等待编码完成
        let encode_cmd = format!(
            r#"local ipc = require('moho_ipc')
local input = "{}/frame_%05d.png"
local output = "{}"
local fps = 24
local ok, err = ipc.encode_video(input, output, fps, 23, "{}")
if not ok then
  print('✗ 编码启动失败: ' .. tostring(err))
  return
end
-- 同步等待编码完成(最多 300 秒)
local max_wait = 300
local waited = 0
while waited < max_wait do
  local s = ipc.encode_status()
  if s.status == 2 then
    print('✓ 编码完成: ' .. output)
    break
  elseif s.status == 3 then
    print('✗ 编码失败')
    break
  end
  os.execute('sleep 1')
  waited = waited + 1
  if waited % 10 == 0 then
    print('  等待 ' .. waited .. ' 秒...')
  end
end
if waited >= max_wait then
  print('✗ 编码超时')
end"#,
            png_dir, output_path, codec
        );
        
        ipc_send_multiline(&encode_cmd)?;
        
        // 清理临时 PNG
        println!("▶ 清理临时帧...");
        let _ = std::fs::remove_dir_all(&png_dir);
        println!("✓ 完成: {}", output_path);
    } else {
        println!("✓ 完成: {}", png_dir);
    }
    
    Ok(())
}

fn cmd_encode(input: &str, output: &str, fps: u32, crf: u32) -> Result<()> {
    // 判断输出格式
    let is_gif = output.ends_with(".gif");
    let is_apng = output.ends_with(".apng");
    
    // APNG 实际输出路径（标准后缀是 .png）
    let actual_output = if is_apng {
        output.replace(".apng", ".png")
    } else {
        output.to_string()
    };
    
    if is_apng {
        println!("▶ 编码 APNG(动画 PNG,无损 + 透明)");
    } else if is_gif {
        println!("▶ 编码 GIF(libavfilter 调色板优化)");
    } else {
        println!("▶ 编码 MP4(内置 FFmpeg)");
    }
    
    println!("  输入: {}", input);
    if is_apng && output != actual_output {
        println!("  输出: {}(APNG 使用标准 PNG 后缀)", actual_output);
    } else {
        println!("  输出: {}", output);
    }
    println!("  帧率: {} fps", fps);
    
    // 自动启动 IPC
    auto_start_ipc()?;
    
    // 发送编码命令（同步等待完成）
    let lua_cmd = format!(
        r#"local ipc = require('moho_ipc')
local ok, err = ipc.encode_video("{}", "{}", {}, {}, "mpeg4")
if not ok then
  print('✗ 编码启动失败: ' .. tostring(err))
  return
end
-- 等待编码完成
local max_wait = 300
local waited = 0
while waited < max_wait do
  local s = ipc.encode_status()
  if s.status == 2 then
    print('✓ 编码完成: {}')
    break
  elseif s.status == 3 then
    print('✗ 编码失败: ' .. tostring(s.error_msg))
    break
  end
  os.execute('sleep 1')
  waited = waited + 1
end
if waited >= max_wait then
  print('✗ 编码超时')
end"#,
        input, actual_output, fps, crf, actual_output
    );
    
    ipc_send_multiline(&lua_cmd)?;
    
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

// ========== IPC 辅助函数（render 命令使用）==========

/// 自动启动 IPC 服务
fn auto_start_ipc() -> Result<()> {
    use std::os::unix::net::UnixStream;
    
    let socket_path = "/tmp/moho_ipc.sock";
    
    // 检查 socket 是否存在且可连接
    if let Ok(stream) = UnixStream::connect(socket_path) {
        drop(stream);  // 关闭连接
        
        // 检查 Moho 进程是否在运行
        let moho_running = std::process::Command::new("pgrep")
            .arg("-x")
            .arg("Moho")
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
        
        if moho_running {
            // IPC 已运行，等待一小段时间确保就绪
            std::thread::sleep(std::time::Duration::from_millis(500));
            return Ok(());
        }
        
        // Moho 已退出，清理 socket
        println!("⚠️ Moho 已退出，重新启动...");
        let _ = std::fs::remove_file(socket_path);
    }
    
    println!("▶ 启动 IPC 服务...");
    
    // 启动 moho-mate start
    let _child = std::process::Command::new(std::env::current_exe()?)
        .args(["start"])
        .spawn()?;
    
    // 等待 socket 就绪（最多 30 秒）
    for i in 0..30 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if let Ok(stream) = UnixStream::connect(socket_path) {
            drop(stream);
            println!("✓ IPC 已就绪 ({}秒)", i + 1);
            return Ok(());
        }
    }
    
    anyhow::bail!("IPC 启动超时")
}

/// 发送单行 IPC 命令
fn ipc_send(cmd: &str) -> Result<String> {
    use std::os::unix::net::UnixStream;
    use std::io::{Read, Write};
    use std::net::Shutdown;
    
    let mut stream = UnixStream::connect("/tmp/moho_ipc.sock")?;
    
    // 发送命令
    writeln!(stream, "{}", cmd)?;
    stream.shutdown(Shutdown::Write)?;
    
    // 读取响应
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    
    Ok(response)
}

/// 发送多行 IPC 命令
fn ipc_send_multiline(cmd: &str) -> Result<String> {
    use std::os::unix::net::UnixStream;
    use std::io::{Read, Write};
    use std::net::Shutdown;
    
    let mut stream = UnixStream::connect("/tmp/moho_ipc.sock")?;
    
    // 发送多行命令（以 ---END--- 结尾）
    write!(stream, "{}\n---END---\n", cmd)?;
    stream.shutdown(Shutdown::Write)?;
    
    // 读取响应
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    
    // 打印响应
    print!("{}", response);
    
    Ok(response)
}
