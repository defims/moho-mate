//! moho-mate 库
//!
//! 提供 IPC 核心功能和 Lua FFI 绑定
//!
//! ## 关键说明
//!
//! ### 模块名
//!
//! - Cargo.toml 中 `[lib] name = "moho_mate"`（连字符转下划线）
//! - Lua 中 `require("moho_ipc")`（模块名由 luaopen_moho_ipc 决定）
//! - 两者不同：Cargo lib name 是 Rust 内部引用，Lua 模块名由 `#[no_mangle]` 函数名决定
//!
//! ### 符号导出
//!
//! - `#[no_mangle]` 防止函数名被 mangle
//! - `pub extern "C"` 使用 C ABI
//! - `luaopen_moho_ipc` 是 Lua 模块加载入口（命名规则：luaopen_模块名）
//! - 放在 lib.rs 确保符号不会被 bin LTO 优化掉
//!
//! ### bin 引用 lib
//!
//! main.rs 中通过 `use moho_mate::luaopen_moho_ipc` 引用，
//! 防止 LTO 优化掉这个符号（虽然 `#[no_mangle]` 应该足够，但双重保险）
//!
//! ## FFmpeg 编码模块
//!
//! ### 条件编译
//!
//! - `#[cfg(all(target_os = "macos", feature = "ffmpeg-builtin"))]`
//! - 目前仅 macOS 支持 Moho 内置 FFmpeg
//! - Windows/Linux 待后续支持
//!
//! ### 模块加载
//!
//! ```text
//! lib.rs
//!   ↓
//! ffmpeg_ffi.rs  → FFmpeg C API 绑定
//!   ↓
//! encode_native.rs → encode_gif/mp4/apng 实现
//!   ↓
//! ipc_core.rs    → IPC 调用入口
//! ```
//!
//! ### 库依赖
//!
//! Moho 内置 FFmpeg（macOS）：
//! - /Applications/Moho.app/Contents/Frameworks/
//! - libavcodec.61.dylib, libavformat.61.dylib, libavutil.59.dylib
//! - libswscale.8.dylib, libswresample.5.dylib
//!
//! scripts 目录（额外）：
//! - libavfilter.10.dylib（Moho 没有内置，用于 GIF 调色板优化）
//!
//! ### 相关文件
//!
//! - Cargo.toml: [lib] name = "moho_mate" 配置
//! - build.rs: 设置 rpath 和链接 FFmpeg
//! - build.sh: 执行 install_name_tool 修改库路径
//! - ffmpeg_ffi.rs: FFmpeg FFI 绑定
//! - encode_native.rs: FFmpeg 编码实现

pub mod config;
pub mod lua_ffi;
pub mod ipc_core;
pub mod pkg;

#[cfg(all(target_os = "macos", feature = "ffmpeg-builtin"))]
pub mod ffmpeg_ffi;

#[cfg(all(target_os = "macos", feature = "ffmpeg-builtin"))]
pub mod encode_native;

// ========== Lua 模块导出 ==========

use std::os::raw::{c_int, c_void};
use lua_ffi::*;

pub type lua_State = *mut c_void;

/// 导出 Lua 模块（Moho 通过 require("moho_ipc") 调用）
///
/// 放在 lib.rs 确保符号不会被 bin LTO 优化掉
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

// ========== Lua 回调函数（委托给 ipc_core） ==========

use ipc_core::{ipc_start, ipc_stop, get_status, encode_video, encode_status, encode_cancel};

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
