//! config 命令 - 配置管理

use anyhow::{anyhow, Result};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;

pub fn execute(action: &str) -> Result<()> {
    match action {
        "list" => list_config(),
        "backup" => backup_config(),
        "restore" => restore_config(),
        _ => Err(anyhow!("用法: moho-mate config list|backup|restore")),
    }
}

fn list_config() -> Result<()> {
    println!("=== Moho 配置目录 ===");
    println!("  路径: {}\n", config::moho_config_dir().display());

    let config_dir = config::moho_config_dir();
    if !config_dir.exists() {
        println!("  (目录不存在)");
        return Ok(());
    }

    for entry in fs::read_dir(&config_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata()?;
        let modified: chrono::DateTime<chrono::Local> = metadata.modified()?.into();

        println!(
            "  {}  ({}, {} bytes)",
            name,
            modified.format("%Y-%m-%d %H:%M"),
            metadata.len()
        );
    }

    Ok(())
}

fn backup_config() -> Result<()> {
    println!("▶ 备份 Moho 配置");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let backup_dir = format!("/tmp/moho_config_backup_{}", now);

    let status = Command::new("cp")
        .args(["-R", &format!("{}/", config::moho_config_dir().display())])
        .arg(&backup_dir)
        .status()?;

    if status.success() {
        println!("✓ 已备份到: {}", backup_dir);
    } else {
        return Err(anyhow!("备份失败"));
    }

    Ok(())
}

fn restore_config() -> Result<()> {
    println!("▶ 恢复 Moho 配置");

    // 找最新的备份
    let output = Command::new("sh")
        .args(["-c", "ls -dt /tmp/moho_config_backup_* 2>/dev/null | head -1"])
        .output()?;

    let backup_dir = String::from_utf8_lossy(&output.stdout);
    let backup_dir = backup_dir.trim();

    if backup_dir.is_empty() {
        return Err(anyhow!("无可用备份"));
    }

    println!("  源: {}", backup_dir);

    let status = Command::new("cp")
        .args(["-R", &format!("{}/", backup_dir)])
        .arg(config::moho_config_dir())
        .status()?;

    if status.success() {
        println!("✓ 已恢复");
    } else {
        return Err(anyhow!("恢复失败"));
    }

    Ok(())
}
