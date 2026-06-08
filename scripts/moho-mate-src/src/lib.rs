//! moho-mate 库
//!
//! 提供 IPC 核心功能和 Lua FFI 绑定

pub mod config;
pub mod lua_ffi;
pub mod ipc_core;
pub mod pkg;

#[cfg(all(target_os = "macos", feature = "ffmpeg-builtin"))]
pub mod ffmpeg_ffi;

#[cfg(all(target_os = "macos", feature = "ffmpeg-builtin"))]
pub mod encode_native;
