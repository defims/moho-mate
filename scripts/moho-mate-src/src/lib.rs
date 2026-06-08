//! moho-mate 库
//!
//! 提供 IPC 核心功能和 Lua FFI 绑定

pub mod config;
pub mod lua_ffi;
pub mod ipc_core;

#[cfg(target_os = "macos")]
pub mod ffmpeg_ffi;

#[cfg(target_os = "macos")]
pub mod encode_native;
