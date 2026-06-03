//! render 命令 - 渲染项目

use anyhow::{anyhow, Result};
use std::path::Path;
use std::fs;
use tracing::info;

use crate::ipc::IpcClient;

pub async fn execute(
    project: &str,
    format: &str,
    output: Option<&str>,
    start: u32,
    end: u32,
) -> Result<()> {
    // 检查项目存在
    if !Path::new(project).exists() {
        return Err(anyhow!("项目不存在: {}", project));
    }

    let is_video = matches!(format, "MP4" | "GIF" | "APNG" | "QT");

    if is_video {
        let format_name = if format == "APNG" { "APNG(动画 PNG)" } else { format };
        info!("▶ 渲染 + 编码: {}", format_name);
    }

    info!("▶ 渲染项目: {}", project);
    println!("  格式: {}", format);
    println!("  帧范围: {}-{}", start, end);

    let client = IpcClient::new();
    client.start_ipc(None, None, 3600).await?;

    // 打开项目
    let open_cmd = format!("moho:FileOpen(\"{}\")", project);
    client.send(&open_cmd).await?;

    // 确定输出路径
    let output_path = if let Some(o) = output {
        o.to_string()
    } else {
        let base = Path::new(project)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap();
        if is_video {
            let ext = if format == "APNG" { "png" } else { &format.to_lowercase() }; // 修复：借用
            format!("/tmp/{}.{}", base, ext)
        } else {
            format!("/tmp/{}", base)
        }
    };

    let ext = match format {
        "JPEG" | "JPG" => "jpg",
        "BMP" => "bmp",
        "TGA" => "tga",
        _ => "png",
    };

    // PNG 序列目录
    let png_dir = if is_video {
        format!("/tmp/moho_render_frames_{}", std::process::id())
    } else {
        output_path.to_string()
    };

    fs::create_dir_all(&png_dir)?;

    // 渲染 Lua
    let render_lua = format!(
        r#"local ipc = require('moho_ipc')
local moho = _G.moho
if not moho then
    local helper = MOHO.ScriptInterfaceHelper:new_local()
    moho = helper:MohoObject()
end
local output_dir = "{png_dir}"
for f = {start} to {end} do
    moho:SetCurFrame(f, true)
    local frame_path = output_dir .. "/frame_" .. string.format("%05d", f) .. ".{ext}"
    moho:FileRender(frame_path)
end
print("✓ 渲染完成: " .. ({end} - {start} + 1) .. " 帧")"#,
        png_dir = png_dir,
        start = start,
        end = end,
        ext = ext
    );

    client.send_multiline(&render_lua).await?;

    // 视频编码
    if is_video {
        info!("✓ 序列已保存到: {}", png_dir);

        let codec = match format {
            "GIF" => "gif",
            "APNG" => "apng",
            _ => "mpeg4",
        };

        info!("▶ 编码 {}: {}", format, output_path);

        let encode_lua = format!(
            r#"local ipc = require('moho_ipc')
local input = "{png_dir}/frame_%05d.png"
local output = "{output_path}"
local fps = 24
local ok, err = ipc.encode_video(input, output, fps, 23, "{codec}")
if not ok then
    print("✗ 编码启动失败: " .. tostring(err))
    return
end
local max_wait = 300
local waited = 0
while waited < max_wait do
    local s = ipc.encode_status()
    if s.status == 2 then
        print("✓ 编码完成: " .. output)
        break
    elseif s.status == 3 then
        print("✗ 编码失败: " .. tostring(s.error_msg))
        break
    end
    os.execute('sleep 1')
    waited = waited + 1
end"#,
            png_dir = png_dir,
            output_path = output_path,
            codec = codec
        );

        client.send_multiline(&encode_lua).await?;

        // 清理临时帧
        info!("▶ 清理临时帧...");
        fs::remove_dir_all(&png_dir)?;

        if Path::new(&output_path).exists() {
            println!("✓ 视频已保存到: {}", output_path);
        } else {
            return Err(anyhow!("输出文件不存在: {}", output_path));
        }
    } else {
        println!("✓ 序列已保存到: {}", output_path);
    }

    Ok(())
}