//! 自升级模块
//!
//! 从 GitHub Release 检查更新并下载替换

use anyhow::{Result, Context, bail};
use std::path::PathBuf;
use serde::Deserialize;
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::Read;

/// GitHub Release API（公开仓库）
const RELEASE_API: &str = "https://api.github.com/repos/defims/moho-mate/releases/latest";

/// Release 信息
#[derive(Debug, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: Option<String>,
    pub assets: Vec<Asset>,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// 检查更新
pub fn check_update() -> Result<Option<ReleaseInfo>> {
    let current = env!("CARGO_PKG_VERSION");
    
    println!("▶ 当前版本: {}", current);
    println!("▶ 检查更新...");
    
    // 调用 GitHub API
    let client = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(20))
        .build();
    
    let response = client.get(RELEASE_API)
        .set("User-Agent", "moho-mate")
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| anyhow::anyhow!("GitHub API 请求失败: {}", e))?;
    
    let body = response.into_string()
        .map_err(|e| anyhow::anyhow!("读取 GitHub 响应失败: {}", e))?;
    
    let release: ReleaseInfo = serde_json::from_str(&body)
        .map_err(|e| anyhow::anyhow!("解析 GitHub 响应失败: {}", e))?;
    
    // 比较版本号
    let latest = release.tag_name.trim_start_matches('v');
    let current_parts = parse_version(current)?;
    let latest_parts = parse_version(latest)?;
    
    if latest_parts > current_parts {
        println!("▶ 最新版本: {} ({})", latest, release.published_at.as_deref().unwrap_or("unknown"));
        
        if let Some(ref notes) = release.body {
            println!("\n更新内容:");
            for line in notes.lines().take(5) {
                if !line.is_empty() {
                    println!("  {}", line);
                }
            }
        }
        
        return Ok(Some(release));
    } else {
        println!("✓ 已是最新版本");
        return Ok(None);
    }
}

/// 解析版本号
fn parse_version(v: &str) -> Result<Vec<u32>> {
    let parts: Vec<u32> = v.split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    if parts.len() < 2 {
        bail!("无效版本号: {}", v);
    }
    
    Ok(parts)
}

/// 获取当前平台对应的 asset 名
pub fn get_asset_name() -> String {
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "macos-x64".to_string();
    
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "macos-arm64".to_string();
    
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux-x64".to_string();
    
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "windows-x64".to_string();
    
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    return "unsupported".to_string();
}

/// 查找匹配的 asset
pub fn find_asset(release: &ReleaseInfo) -> Option<(String, String)> {
    let platform = get_asset_name();
    
    for asset in &release.assets {
        if asset.name.contains(&platform) && asset.name.ends_with(".tar.gz") {
            return Some((asset.name.clone(), asset.browser_download_url.clone()));
        }
    }
    
    None
}

/// 下载并校验
pub fn download_and_verify(url: &str, expected_sha256: Option<&str>) -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir();
    let filename = url.rsplit('/').next().unwrap_or("moho-mate.tar.gz");
    let tar_path = temp_dir.join(filename);
    
    println!("▶ 下载 {}...", filename);
    
    // 下载
    let mut response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|e| anyhow::anyhow!("下载失败: {}", e))?
        .into_reader();
    
    let mut file = File::create(&tar_path)
        .with_context(|| format!("创建文件失败: {:?}", tar_path))?;
    
    std::io::copy(&mut response, &mut file)?;
    
    let size = tar_path.metadata()?.len();
    println!("  ✓ 已下载 {} 字节", size);
    
    // SHA256 校验
    if let Some(expected) = expected_sha256 {
        println!("▶ 校验 SHA256...");
        let actual = compute_sha256(&tar_path)?;
        
        if actual != expected.to_lowercase() {
            bail!("SHA256 校验失败:\n  期望: {}\n  实际: {}", expected, actual);
        }
        println!("  ✓ 校验通过");
    }
    
    Ok(tar_path)
}

/// 计算 SHA256
fn compute_sha256(path: &PathBuf) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    
    Ok(format!("{:x}", hasher.finalize()))
}

/// 替换二进制
pub fn replace_binary(tar_path: &PathBuf) -> Result<()> {
    use crate::app_config::{bin_dir, bin_path, backup_bin_path};
    use std::process::Command;
    
    println!("▶ 备份当前版本...");
    
    let current_bin = bin_path();
    let backup_bin = backup_bin_path();
    
    // 备份
    if current_bin.exists() {
        std::fs::copy(&current_bin, &backup_bin)
            .with_context(|| "备份失败")?;
        println!("  ✓ 已备份到 {:?}", backup_bin);
    }
    
    println!("▶ 解压替换...");
    
    // 解压 (macOS/Linux 用 tar)
    #[cfg(unix)]
    {
        let output = Command::new("tar")
            .arg("-xzf")
            .arg(tar_path)
            .arg("-C")
            .arg(bin_dir())
            .output()
            .with_context(|| "解压失败")?;
        
        if !output.status.success() {
            bail!("解压失败: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
    
    #[cfg(windows)]
    {
        // Windows 用 PowerShell 或 7z
        let output = Command::new("powershell")
            .arg("-Command")
            .arg(format!("Expand-Archive -Path '{}' -DestinationPath '{}'", 
                tar_path.display(), bin_dir().display()))
            .output()
            .with_context(|| "解压失败")?;
        
        if !output.status.success() {
            bail!("解压失败: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
    
    println!("  ✓ 解压完成");
    
    // 设置可执行权限 (Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&current_bin, 
            std::fs::Permissions::from_mode(0o755))?;
    }
    
    // 验证
    println!("▶ 验证新版本...");
    let output = Command::new(&current_bin)
        .arg("--version")
        .output()
        .with_context(|| "验证失败")?;
    
    if !output.status.success() {
        // 回滚
        println!("✗ 验证失败，回滚...");
        if backup_bin.exists() {
            std::fs::copy(&backup_bin, &current_bin)?;
        }
        bail!("新版本验证失败");
    }
    
    println!("  ✓ {}", String::from_utf8_lossy(&output.stdout).trim());
    
    // 清理
    std::fs::remove_file(tar_path).ok();
    
    println!("\n✓ 更新完成");
    
    Ok(())
}
