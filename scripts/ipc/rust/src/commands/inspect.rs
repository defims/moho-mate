//! inspect 命令 - 查看项目信息

use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn execute(project: &str) -> Result<()> {
    if !std::path::Path::new(project).exists() {
        return Err(anyhow!("项目不存在: {}", project));
    }

    println!("=== 项目信息 ===");
    println!("  文件: {}", project);

    let file = File::open(project)?;
    let reader = BufReader::new(file);

    let mut layer_count = 0;
    let mut bone_count = 0;
    let mut mesh_count = 0;
    let mut start_frame = 0;
    let mut end_frame = 72;
    let mut fps = 24;

    for line in reader.lines() {
        let line = line?;

        if line.contains("<layer") {
            layer_count += 1;
        }
        if line.contains("<bone") {
            bone_count += 1;
        }
        if line.contains("<mesh") {
            mesh_count += 1;
        }

        // 提取帧范围
        if let Some(pos) = line.find("start_frame=\"") {
            let rest = &line[pos + 13..];
            if let Some(end) = rest.find('"') {
                start_frame = rest[..end].parse().unwrap_or(0);
            }
        }

        if let Some(pos) = line.find("end_frame=\"") {
            let rest = &line[pos + 11..];
            if let Some(end) = rest.find('"') {
                end_frame = rest[..end].parse().unwrap_or(72);
            }
        }

        if let Some(pos) = line.find("fps=\"") {
            let rest = &line[pos + 5..];
            if let Some(end) = rest.find('"') {
                fps = rest[..end].parse().unwrap_or(24);
            }
        }
    }

    println!("\n=== 内容统计 ===");
    println!("  图层数: {}", layer_count);
    println!("  骨骼数: {}", bone_count);
    println!("  Mesh数: {}", mesh_count);
    println!("\n=== 动画设置 ===");
    println!("  帧范围: {} - {}", start_frame, end_frame);
    println!("  帧率: {} fps", fps);
    println!("  时长: {:.2} 秒", (end_frame - start_frame + 1) as f32 / fps as f32);

    Ok(())
}
