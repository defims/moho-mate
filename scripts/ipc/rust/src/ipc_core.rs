//! IPC 核心实现
//!
//! Socket 服务、命令处理、FFmpeg 编码、播放控制

use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixStream, UnixListener};
use std::thread;
use std::time::Duration;

use tracing::{info, warn, error};

use crate::lua_ffi::*;

// ========== 配置 ==========

const SOCKET_PATH: &str = "/tmp/moho_ipc.sock";
const LOG_FILE: &str = "/tmp/moho_ipc.log";

// ========== 全局状态 ==========

static RUNNING: AtomicBool = AtomicBool::new(false);
static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
static ERROR_COUNT: AtomicUsize = AtomicUsize::new(0);

// 编码状态
static ENCODE_STATUS: AtomicI32 = AtomicI32::new(0); // 0=idle, 1=running, 2=success, 3=error
static ENCODE_PROGRESS: AtomicI32 = AtomicI32::new(0); // 0-100 (百分比 * 100)

// Socket 和线程句柄
static SOCKET_LISTENER: Mutex<Option<UnixListener>> = Mutex::new(None);
static SOCKET_THREAD: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

// Lua state（仅在主线程使用，不需要 Send）
static mut LUA_STATE: Option<*mut std::ffi::c_void> = None;

// ========== 日志 ==========

fn log_msg(msg: &str) {
    println!("{}", msg);

    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)
    {
        use std::fmt::Write;
        let _ = writeln!(f, "{}", msg);
    }
}

// ========== IPC 服务 ==========

/// 启动 IPC 服务
pub fn ipc_start(L: lua_State) -> (bool, String) {
    log_msg("=== IPC start ===");

    // 保存 Lua state（仅在主线程）
    unsafe {
        LUA_STATE = Some(L);
    }

    if RUNNING.load(Ordering::SeqCst) {
        return (true, "already running".to_string());
    }

    // 删除旧 socket
    let _ = fs::remove_file(SOCKET_PATH);

    // 创建 socket
    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            log_msg(&format!("✗ bind() failed: {}", e));
            return (false, format!("bind() failed: {}", e));
        }
    };

    // 设置权限（unix 特有）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o600));
    }

    log_msg(&format!("✓ IPC 服务启动: {}", SOCKET_PATH));

    RUNNING.store(true, Ordering::SeqCst);

    // 存储 listener
    if let Ok(mut l) = SOCKET_LISTENER.lock() {
        *l = Some(listener);
    }

    // 启动监听线程
    let handle = thread::spawn(move || {
        listen_loop();
    });

    if let Ok(mut th) = SOCKET_THREAD.lock() {
        *th = Some(handle);
    }

    (true, SOCKET_PATH.to_string())
}

/// 停止 IPC 服务
pub fn ipc_stop() -> bool {
    log_msg("=== IPC stop ===");

    RUNNING.store(false, Ordering::SeqCst);

    // 关闭 socket
    if let Ok(mut l) = SOCKET_LISTENER.lock() {
        *l = None;
    }

    // 等待线程结束
    if let Ok(mut th) = SOCKET_THREAD.lock() {
        if let Some(handle) = th.take() {
            let _ = handle.join();
        }
    }

    // 清理 Lua state
    unsafe {
        LUA_STATE = None;
    }

    let _ = fs::remove_file(SOCKET_PATH);
    log_msg("✓ IPC 服务停止");

    true
}

/// 监听循环
fn listen_loop() {
    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }

        // 获取 listener
        let listener = match SOCKET_LISTENER.lock() {
            Ok(l) => l,
            Err(_) => break,
        };

        let listener = match listener.as_ref() {
            Some(l) => l,
            None => break,
        };

        // 设置非阻塞
        let _ = listener.set_nonblocking(true);

        match listener.accept() {
            Ok((stream, _addr)) => {
                drop(listener); // 释放锁
                handle_client(stream);
            }
            Err(e) => {
                drop(listener); // 释放锁
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    warn!("accept error: {}", e);
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// 处理客户端连接
fn handle_client(mut stream: UnixStream) {
    log_msg("新连接");

    // 设置超时
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

    // 读取命令
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let cmd = String::from_utf8_lossy(&buf[..n]);
    let cmd = cmd.trim();

    log_msg(&format!("收到命令: {}", cmd));

    // 执行命令
    let response = execute_command(cmd);

    // 发送响应
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(b"\n");

    log_msg(&format!("响应: {}", response));
    CALL_COUNT.fetch_add(1, Ordering::SeqCst);
}

/// 执行 Lua 命令
fn execute_command(cmd: &str) -> String {
    // 获取 Lua state
    let L = unsafe { LUA_STATE };

    if L.is_none() {
        return "error|no Lua state".to_string();
    }

    let L = unsafe { L.unwrap() };

    unsafe {
        // 加载并执行代码
        // luaL_dostring 是宏: luaL_loadstring(L, s) || lua_pcall(L, 0, LUA_MULTRET, 0)
        let c_cmd = CString::new(cmd).unwrap();

        // 先加载
        let ret = luaL_loadstring(L, c_cmd.as_ptr());
        if ret != 0 {
            // 加载错误
            let err = to_string(L, -1);
            let err_msg = err.unwrap_or("load error");
            ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
            return format!("error|{}", err_msg);
        }

        // 再执行
        let ret = lua_pcall(L, 0, 0, 0);

        if ret != 0 {
            // 执行错误
            let err = to_string(L, -1);
            let err_msg = err.unwrap_or("execution error");
            ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
            format!("error|{}", err_msg)
        } else {
            // 成功
            "ok|".to_string()
        }
    }
}

// ========== 编码 API ==========

pub fn encode_video(input: &str, output: &str, fps: i32, crf: i32, _codec: &str) -> (bool, String) {
    info!("encode_video: {} -> {} (fps={}, crf={})", input, output, fps, crf);

    // 检查是否正在编码
    if ENCODE_STATUS.load(Ordering::SeqCst) == 1 {
        return (false, "编码正在进行中".to_string());
    }

    ENCODE_STATUS.store(1, Ordering::SeqCst);
    ENCODE_PROGRESS.store(0, Ordering::SeqCst);

    let input = input.to_string();
    let output = output.to_string();

    // 启动编码线程
    thread::spawn(move || {
        let result = do_encode(&input, &output, fps, crf, "mpeg4");

        match result {
            Ok(_) => {
                ENCODE_STATUS.store(2, Ordering::SeqCst);
                info!("编码完成: {}", output);
            }
            Err(e) => {
                ENCODE_STATUS.store(3, Ordering::SeqCst);
                error!("编码失败: {}", e);
            }
        }
    });

    (true, "编码已启动".to_string())
}

fn do_encode(input: &str, output: &str, fps: i32, crf: i32, _codec: &str) -> anyhow::Result<()> {
    // 使用 FFmpeg 命令行编码（与 C 版本一致）
    use std::process::Command;

    let output_ext = if output.ends_with(".gif") {
        "gif"
    } else if output.ends_with(".mp4") {
        "mp4"
    } else {
        "mp4"
    };

    let _is_multi_frame = input.contains('%');

    let result = if output_ext == "gif" {
        // GIF: 使用 palettegen/paletteuse 优化
        Command::new("ffmpeg")
            .args([
                "-y",
                "-framerate", &fps.to_string(),
                "-i", input,
                "-vf", "fps=24,scale=320:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
                "-loop", "0",
                output,
            ])
            .output()
    } else {
        // MP4: 使用 mpeg4 编码
        Command::new("ffmpeg")
            .args([
                "-y",
                "-framerate", &fps.to_string(),
                "-i", input,
                "-c:v", "mpeg4",
                "-q:v", &crf.to_string(),
                "-pix_fmt", "yuv420p",
                output,
            ])
            .output()
    };

    // 更新进度（简化：假设命令完成后进度100%）
    ENCODE_PROGRESS.store(10000, Ordering::SeqCst); // 100.00%

    match result {
        Ok(output_result) => {
            if output_result.status.success() {
                info!("编码完成: {}", output);
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output_result.stderr);
                anyhow::bail!("FFmpeg 错误: {}", stderr);
            }
        }
        Err(e) => {
            anyhow::bail!("FFmpeg 执行失败: {}", e);
        }
    }
}

pub fn encode_status() -> (i32, &'static str, i32) {
    let status = ENCODE_STATUS.load(Ordering::SeqCst);
    let progress = ENCODE_PROGRESS.load(Ordering::SeqCst);

    let status_text = match status {
        0 => "idle",
        1 => "running",
        2 => "success",
        3 => "error",
        _ => "unknown",
    };

    (status, status_text, progress)
}

pub fn encode_cancel() -> bool {
    if ENCODE_STATUS.load(Ordering::SeqCst) == 1 {
        ENCODE_STATUS.store(0, Ordering::SeqCst);
        true
    } else {
        false
    }
}

// ========== 状态查询 ==========

pub fn get_status() -> (bool, String, usize, usize) {
    let running = RUNNING.load(Ordering::SeqCst);
    let calls = CALL_COUNT.load(Ordering::SeqCst);
    let errors = ERROR_COUNT.load(Ordering::SeqCst);

    (running, SOCKET_PATH.to_string(), calls, errors)
}
