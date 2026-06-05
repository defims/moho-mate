//! FFmpeg 编码模块
//!
//! 使用系统 ffmpeg 或内置 FFmpeg 库

use crate::ipc_core;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;
use tracing::{info, warn};

/// 检查 Moho 内置 FFmpeg 库是否可用
pub fn check_ffmpeg_available() -> bool {
    // 检查系统 ffmpeg
    if which::which("ffmpeg").is_ok() {
        return true;
    }
    
    // 检查 Moho 内置库
    let moho_fw = Path::new("/Applications/Moho.app/Contents/Frameworks");
    let libs = [
        "libavcodec.61.dylib",
        "libavformat.61.dylib",
        "libavutil.59.dylib",
        "libswscale.8.dylib",
        "libswresample.5.dylib",
    ];
    
    libs.iter().all(|lib| moho_fw.join(lib).exists())
}

/// 检查 libavfilter 是否可用
pub fn check_avfilter_available() -> bool {
    Path::new("/Users/def/.openclaw/workspace/skills/moho-mate/scripts/libavfilter.10.dylib").exists()
}

/// 使用内置 FFmpeg 编码 GIF（带调色板优化）
pub fn encode_gif_with_palette(input: &str, output: &str, fps: i32) -> anyhow::Result<()> {
    encode_with_system_ffmpeg(input, output, fps, 23, "gif")
}

/// 使用内置 FFmpeg 编码 MP4
pub fn encode_mp4(input: &str, output: &str, fps: i32, crf: i32) -> anyhow::Result<()> {
    encode_with_system_ffmpeg(input, output, fps, crf, "mp4")
}

/// 使用内置 FFmpeg 编码 APNG
pub fn encode_apng(input: &str, output: &str, fps: i32) -> anyhow::Result<()> {
    encode_with_system_ffmpeg(input, output, fps, 23, "apng")
}

/// 使用系统 ffmpeg 编码
fn encode_with_system_ffmpeg(input: &str, output: &str, fps: i32, crf: i32, format: &str) -> anyhow::Result<()> {
    // 查找系统 ffmpeg
    let ffmpeg = match which::which("ffmpeg").ok() {
        Some(f) => f,
        None => anyhow::bail!("未找到系统 ffmpeg，请安装: brew install ffmpeg"),
    };
    
    info!("使用系统 ffmpeg: {:?}", ffmpeg);
    
    let result = if format == "gif" {
        // GIF: 使用 libavfilter 调色板优化
        info!("GIF 调色板优化: palettegen + paletteuse");
        Command::new(&ffmpeg)
            .args([
                "-y",
                "-framerate", &fps.to_string(),
                "-i", input,
                "-vf", "fps=24,scale=320:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5",
                "-loop", "0",
                output,
            ])
            .output()
    } else if format == "apng" {
        // APNG: 动画 PNG，无损 + 透明
        let actual_output = if output.ends_with(".apng") {
            output.replace(".apng", ".png")
        } else {
            output.to_string()
        };
        
        info!("APNG 编码: {}", actual_output);
        Command::new(&ffmpeg)
            .args([
                "-y",
                "-framerate", &fps.to_string(),
                "-i", input,
                "-plays", "0",
                "-f", "apng",
                &actual_output,
            ])
            .output()
    } else {
        // MP4: 使用 mpeg4 编码器
        info!("MP4 编码: mpeg4, crf={}", crf);
        Command::new(&ffmpeg)
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

/// 使用内置 FFmpeg 编码（自动检测格式）
pub fn encode_with_builtin_ffmpeg(input: &str, output: &str, fps: i32, crf: i32) -> anyhow::Result<()> {
    // 先检查系统 ffmpeg
    if which::which("ffmpeg").is_err() {
        anyhow::bail!(
            "未找到系统 ffmpeg。请选择以下方案之一：\n\
              方案 A - 安装系统 ffmpeg（推荐）：\n\
                brew install ffmpeg\n\
              方案 B - 使用 C 版本 IPC 模式：\n\
                cp moho-mate.c.bak moho-mate\n\
                moho-mate start project.moho\n\
                moho-mate encode ..."
        );
    }
    
    let output_ext = if output.ends_with(".gif") {
        "gif"
    } else if output.ends_with(".apng") || output.ends_with(".png") {
        "apng"
    } else {
        "mp4"
    };
    
    info!("使用系统 ffmpeg 编码: {} -> {} ({})", input, output, output_ext);
    
    let result = if output_ext == "gif" {
        encode_gif_with_palette(input, output, fps)
    } else if output_ext == "apng" {
        encode_apng(input, output, fps)
    } else {
        encode_mp4(input, output, fps, crf)
    };
    
    // 更新进度
    ipc_core::ENCODE_PROGRESS.store(10000, Ordering::SeqCst);
    
    result
}
