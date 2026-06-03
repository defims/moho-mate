//! encode 命令 - 编码视频

use anyhow::{anyhow, Result};
use tracing::info;

use crate::ipc::IpcClient;

pub async fn execute(input: &str, output: &str, fps: u32, crf: u32) -> Result<()> {
    let is_gif = output.ends_with(".gif");
    let is_apng = output.ends_with(".apng") || output.ends_with(".png");

    if is_gif {
        info!("▶ 编码 GIF(libavfilter 调色板优化)");
    } else if is_apng {
        info!("▶ 编码 APNG(动画 PNG,无损 + 透明)");
    } else {
        info!("▶ 编码 MP4(内置 FFmpeg)");
    }

    println!("  输入: {}", input);
    println!("  输出: {}", output);
    println!("  帧率: {} fps", fps);

    let client = IpcClient::new();
    client.start_ipc(None, None, 3600).await?;

    let codec = if is_gif {
        "gif"
    } else if is_apng {
        "apng"
    } else {
        "mpeg4"
    };

    // 同步编码 Lua
    let encode_lua = format!(
        r#"local ipc = require('moho_ipc')
local ok, err = ipc.encode_video("{input}", "{output}", {fps}, {crf}, "{codec}")
if not ok then
    print("✗ 编码启动失败: " .. tostring(err))
    return
end
local max_wait = 300
local waited = 0
while waited < max_wait do
    local s = ipc.encode_status()
    if s.status == 2 then
        print("✓ 编码完成: {output}")
        break
    elseif s.status == 3 then
        print("✗ 编码失败: " .. tostring(s.error_msg))
        break
    end
    os.execute('sleep 1')
    waited = waited + 1
    if waited % 10 == 0 then
        print("  等待 " .. waited .. " 秒...")
    end
end
if waited >= max_wait then
    print("✗ 编码超时")
end"#,
        input = input,
        output = output,
        fps = fps,
        crf = crf,
        codec = codec
    );

    client.send_multiline(&encode_lua).await?;

    // 检查输出
    if std::path::Path::new(output).exists() {
        println!("✓ 视频已保存到: {}", output);
    } else {
        return Err(anyhow!("输出文件不存在: {}", output));
    }

    Ok(())
}
