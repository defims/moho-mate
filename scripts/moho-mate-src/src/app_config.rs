//! 应用配置管理
//!
//! 跨平台路径 + 配置文件读写

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};

/// 应用数据目录（跨平台）
///
/// macOS:   ~/Library/Application Support/com.maohou.moho-mate/
/// Linux:   ~/.local/share/moho-mate/
/// Windows: %APPDATA%\maohou\moho-mate\
pub fn app_data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    
    #[cfg(target_os = "macos")]
    let path = base.join("com.maohou.moho-mate");
    
    #[cfg(target_os = "linux")]
    let path = base.join("moho-mate");
    
    #[cfg(target_os = "windows")]
    let path = base.join("maohou").join("moho-mate");
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let path = base.join("moho-mate");
    
    path
}

/// 二进制目录
pub fn bin_dir() -> PathBuf {
    app_data_dir().join("bin")
}

/// 二进制路径
pub fn bin_path() -> PathBuf {
    bin_dir().join("moho-mate")
}

/// 备份二进制路径
pub fn backup_bin_path() -> PathBuf {
    bin_dir().join("moho-mate.bak")
}

/// 包目录
pub fn packages_dir() -> PathBuf {
    app_data_dir().join("packages")
}

/// 配置文件路径
pub fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

/// 版本信息文件路径
pub fn version_path() -> PathBuf {
    app_data_dir().join("version.json")
}

/// 上次检查更新时间文件
pub fn last_check_path() -> PathBuf {
    app_data_dir().join("last-check")
}

// ============ 配置结构 ============

/// 用户配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 版本
    #[serde(default)]
    pub version: String,
    
    /// 安装时间
    #[serde(default)]
    pub installed_at: Option<String>,
    
    /// Moho 相关配置
    #[serde(default)]
    pub moho: MohoConfig,
    
    /// 用户设置
    #[serde(default)]
    pub settings: Settings,
}

/// Moho 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MohoConfig {
    /// Moho 应用路径
    #[serde(default)]
    pub app_path: Option<String>,
    
    /// Moho 配置目录
    #[serde(default)]
    pub config_dir: Option<String>,
    
    /// Moho 脚本目录
    #[serde(default)]
    pub scripts_dir: Option<String>,
}

/// 用户设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// 自动检查更新
    #[serde(default = "default_auto_check_update")]
    pub auto_check_update: bool,
    
    /// 更新通道: stable | beta | nightly
    #[serde(default = "default_update_channel")]
    pub update_channel: String,
    
    /// 语言
    #[serde(default = "default_language")]
    pub language: String,
    
    /// 默认项目目录
    #[serde(default)]
    pub default_project_dir: Option<String>,
}

fn default_auto_check_update() -> bool { true }
fn default_update_channel() -> String { "stable".to_string() }
fn default_language() -> String { "zh-CN".to_string() }

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_check_update: default_auto_check_update(),
            update_channel: default_update_channel(),
            language: default_language(),
            default_project_dir: None,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            installed_at: None,
            moho: MohoConfig::default(),
            settings: Settings::default(),
        }
    }
}

// ============ 配置操作 ============

impl AppConfig {
    /// 加载配置
    pub fn load() -> Result<Self> {
        let path = config_path();
        
        if !path.exists() {
            bail!("配置文件不存在，请先运行 'moho-mate init'");
        }
        
        let content = fs::read_to_string(&path)
            .with_context(|| format!("读取配置文件失败: {:?}", path))?;
        
        let config: AppConfig = serde_json::from_str(&content)
            .with_context(|| "解析配置文件失败")?;
        
        Ok(config)
    }
    
    /// 加载或创建默认配置
    pub fn load_or_default() -> Result<Self> {
        if config_path().exists() {
            Self::load()
        } else {
            Ok(Self::default())
        }
    }
    
    /// 保存配置
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        let parent = path.parent().context("无法获取配置目录")?;
        
        fs::create_dir_all(parent)
            .with_context(|| format!("创建目录失败: {:?}", parent))?;
        
        let content = serde_json::to_string_pretty(self)
            .context("序列化配置失败")?;
        
        fs::write(&path, content)
            .with_context(|| format!("写入配置文件失败: {:?}", path))?;
        
        Ok(())
    }
    
    /// 初始化配置（首次安装）
    pub fn init() -> Result<()> {
        println!("▶ 初始化 moho-mate...\n");
        
        // 创建目录
        let dirs = vec![bin_dir(), packages_dir()];
        for dir in &dirs {
            if !dir.exists() {
                fs::create_dir_all(dir)
                    .with_context(|| format!("创建目录失败: {:?}", dir))?;
            }
        }
        
        // 检测 Moho
        let moho_config = detect_moho()?;
        
        // 生成配置
        let config = AppConfig {
            version: env!("CARGO_PKG_VERSION").to_string(),
            installed_at: Some(chrono_lite_now()),
            moho: moho_config,
            settings: Settings::default(),
        };
        
        config.save()?;
        
        println!("✓ 初始化完成\n");
        println!("配置文件: {:?}", config_path());
        println!("\n提示:");
        println!("  - 运行 'moho-mate --help' 查看命令");
        println!("  - 运行 'moho-mate config' 修改设置");
        println!("  - 运行 'moho-mate self update' 更新版本");
        
        Ok(())
    }
}

/// 简单的 ISO 时间戳
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // 简化版：只返回 Unix 时间戳，后续可以换成真正的 ISO 格式
    format!("{}", duration.as_secs())
}

/// 检测 Moho 安装
fn detect_moho() -> Result<MohoConfig> {
    println!("▶ 检测 Moho 安装...");
    
    let mut config = MohoConfig::default();
    
    // macOS
    #[cfg(target_os = "macos")]
    {
        let app_path = "/Applications/Moho.app";
        if Path::new(app_path).exists() {
            println!("  ✓ 找到: {}", app_path);
            config.app_path = Some(app_path.to_string());
        }
        
        // Moho 配置目录
        if let Some(home) = dirs::home_dir() {
            let config_dir = home.join("Library/Preferences/Lost Marble/Moho Pro/14");
            if config_dir.exists() {
                println!("  ✓ 配置目录: {:?}", config_dir);
                config.config_dir = Some(config_dir.to_string_lossy().to_string());
            }
            
            // 脚本目录
            let scripts_dir = home.join("Documents/moho_user_content/Moho Pro/Scripts");
            if scripts_dir.exists() {
                println!("  ✓ 脚本目录: {:?}", scripts_dir);
                config.scripts_dir = Some(scripts_dir.to_string_lossy().to_string());
            }
        }
    }
    
    // Linux / Windows（待实现）
    #[cfg(not(target_os = "macos"))]
    {
        println!("  ⚠ 自动检测暂不支持此平台，请手动配置");
    }
    
    println!();
    Ok(config)
}
