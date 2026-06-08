//! 脚本包管理模块
//!
//! 实现类似 npm/pnpm 的包管理功能：
//! - 包存储在 com.maohou.moho-mate/packages/{name}/{version}/
//! - 用户内容文件夹只放引用文件
//! - 引用文件内嵌 package.path 重定向

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use serde::{Deserialize, Serialize};

// ============ package.json 结构 ============

/// package.json 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageJson {
    /// 包名（支持 @org/name 格式）
    pub name: String,
    /// 版本号
    pub version: String,
    /// 主入口文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    /// 子路径导出定义（支持字符串或对象格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exports: Option<HashMap<String, ExportConfig>>,
    /// 文件清单
    #[serde(default)]
    pub files: Vec<String>,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 作者（支持字符串或对象格式）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub author: Option<Author>,
    /// 许可证
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// 依赖包
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<HashMap<String, String>>,
    /// Moho 特定配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moho: Option<MohoConfig>,
}

/// 作者信息（支持字符串或对象格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Author {
    String(String),
    Object(AuthorObject),
}

/// 作者对象格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorObject {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Author {
    /// 获取作者名称
    pub fn name(&self) -> &str {
        match self {
            Author::String(s) => s,
            Author::Object(obj) => &obj.name,
        }
    }
}

/// 导出配置（支持字符串或对象格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExportConfig {
    String(String),
    Object(ExportObject),
}

/// 导出对象格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Moho 特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MohoConfig {
    /// 最低 Moho 版本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    /// 最高 Moho 版本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_version: Option<String>,
    /// Tool 脚本清单
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolEntry>>,
}

/// Tool 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    /// 工具 ID（不含 .lua 后缀）
    pub id: String,
    /// 显示名称
    pub name: String,
    /// 分组名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl PackageJson {
    /// 从文件加载
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取 package.json 失败: {:?}", path))?;
        let pkg: PackageJson = serde_json::from_str(&content)
            .with_context(|| format!("解析 package.json 失败: {:?}", path))?;
        Ok(pkg)
    }
    
    /// 检查包是否可以被 require（有 main 或 exports["."]）
    pub fn is_requireable(&self) -> bool {
        self.main.is_some() || 
            self.exports.as_ref().map(|e| e.contains_key(".")).unwrap_or(false)
    }
    
    /// 获取主入口文件路径
    pub fn get_main_path(&self) -> Option<String> {
        if let Some(ref main) = self.main {
            Some(main.clone())
        } else if let Some(ref exports) = self.exports {
            if let Some(config) = exports.get(".") {
                match config {
                    ExportConfig::String(s) => Some(s.trim_start_matches("./").to_string()),
                    ExportConfig::Object(obj) => {
                        // 优先返回 require，其次 default
                        if let Some(ref req) = obj.require {
                            Some(req.trim_start_matches("./").to_string())
                        } else if let Some(ref default) = obj.default {
                            Some(default.trim_start_matches("./").to_string())
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

// ============ Lock 文件结构 ============

/// Lock 文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    /// 版本
    pub version: u32,
    /// 已安装的包
    pub packages: HashMap<String, LockPackage>,
}

/// Lock 中的包信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockPackage {
    /// 版本
    pub version: String,
    /// 下载地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    /// 完整性校验
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    /// 依赖
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<HashMap<String, String>>,
}

impl LockFile {
    /// 加载 Lock 文件
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(LockFile {
                version: 1,
                packages: HashMap::new(),
            });
        }
        
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取 lock 文件失败: {:?}", path))?;
        let lock: LockFile = serde_json::from_str(&content)
            .with_context(|| format!("解析 lock 文件失败: {:?}", path))?;
        Ok(lock)
    }
    
    /// 保存 Lock 文件
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .with_context(|| "序列化 lock 文件失败")?;
        std::fs::write(path, content)
            .with_context(|| format!("写入 lock 文件失败: {:?}", path))?;
        Ok(())
    }
}

// ============ 配置文件结构 ============

/// 配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkgConfig {
    /// registry 地址
    #[serde(default = "default_registry")]
    pub registry: String,
}

fn default_registry() -> String {
    "https://mirrors.cloud.tencent.com/npm".to_string()
}

impl Default for PkgConfig {
    fn default() -> Self {
        Self {
            registry: default_registry(),
        }
    }
}

impl PkgConfig {
    /// 加载配置
    pub fn load(base_dir: &Path) -> Result<Self> {
        let config_path = base_dir.join("config.json");
        
        if !config_path.exists() {
            return Ok(Self::default());
        }
        
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("读取配置文件失败: {:?}", config_path))?;
        let config: PkgConfig = serde_json::from_str(&content)
            .with_context(|| format!("解析配置文件失败: {:?}", config_path))?;
        Ok(config)
    }
    
    /// 保存配置
    pub fn save(&self, base_dir: &Path) -> Result<()> {
        let config_path = base_dir.join("config.json");
        std::fs::create_dir_all(base_dir)?;
        
        let content = serde_json::to_string_pretty(self)
            .with_context(|| "序列化配置失败")?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("写入配置文件失败: {:?}", config_path))?;
        Ok(())
    }
}

// ============ 包管理器 ============

/// 包管理器
pub struct PackageManager {
    /// 基础目录（com.maohou.moho-mate）
    base_dir: PathBuf,
    /// 包存储目录
    packages_dir: PathBuf,
    /// 用户脚本目录
    user_scripts_dir: PathBuf,
    /// 配置
    config: PkgConfig,
    /// Lock 文件路径
    lock_path: PathBuf,
}

impl PackageManager {
    /// 创建新的包管理器
    pub fn new() -> Result<Self> {
        // 获取基础目录
        let base_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?
            .join("com.maohou.moho-mate");
        
        let packages_dir = base_dir.join("packages");
        
        // 获取 Moho 用户脚本目录
        let user_scripts_dir = get_moho_user_scripts_dir()
            .unwrap_or_else(|_| {
                // 默认路径
                dirs::document_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("moho_user_content/Moho Pro/Scripts")
            });
        
        // 加载配置
        let config = PkgConfig::load(&base_dir)?;
        
        // Lock 文件路径
        let lock_path = user_scripts_dir.join("moho-mate-lock.json");
        
        Ok(Self {
            base_dir,
            packages_dir,
            user_scripts_dir,
            config,
            lock_path,
        })
    }
    
    /// 安装本地 zip/tar.gz 包
    pub fn install_local(&self, archive_path: &Path) -> Result<()> {
        println!("▶ 安装: {}", archive_path.display());
        
        // 1. 解压到临时目录
        let temp_dir = tempfile::tempdir()
            .with_context(|| "创建临时目录失败")?;
        
        extract_archive(archive_path, temp_dir.path())?;
        
        // 2. 读取 package.json
        let pkg_json_path = temp_dir.path().join("package.json");
        if !pkg_json_path.exists() {
            bail!("压缩包缺少 package.json 文件");
        }
        
        let pkg = PackageJson::from_file(&pkg_json_path)?;
        
        // 3. 验证 files 字段（只警告，不阻塞）
        for file in &pkg.files {
            let file_path = temp_dir.path().join(file);
            if !file_path.exists() {
                eprintln!("  ⚠ 文件不存在: {} (跳过)", file);
            }
        }
        
        // 4. 解析依赖
        let deps = self.resolve_dependencies(&pkg)?;
        
        // 5. 安装主包到 packages 目录
        let target_dir = self.packages_dir.join(&pkg.name).join(&pkg.version);
        if target_dir.exists() {
            bail!("包已安装: {}@{}, 请先卸载", pkg.name, pkg.version);
        }
        
        std::fs::create_dir_all(target_dir.parent().unwrap())?;
        copy_dir_all(temp_dir.path(), &target_dir)?;
        
        println!("✓ 已安装: {}@{}", pkg.name, pkg.version);
        
        // 6. 生成引用文件
        self.generate_ref_files(&target_dir, &pkg, &deps)?;
        
        // 7. 创建 node_modules 符号链接
        self.create_node_modules_symlinks(&target_dir, &deps)?;
        
        // 8. 更新 _tool_list.txt
        if let Some(ref moho) = pkg.moho {
            if let Some(ref tools) = moho.tools {
                if !tools.is_empty() {
                    self.update_tool_list(tools, true)?;
                }
            }
        }
        
        // 9. 更新 Lock 文件
        self.update_lock(&pkg, &deps, None)?;
        
        println!("✓ 安装完成");
        Ok(())
    }
    
    /// 卸载包
    pub fn uninstall(&self, package_name: &str) -> Result<()> {
        println!("▶ 卸载: {}", package_name);
        
        // 1. 查找已安装的包
        let versions = self.find_installed_versions(package_name)?;
        if versions.is_empty() {
            bail!("包未安装: {}", package_name);
        }
        
        // 2. 读取 Lock 文件
        let lock = LockFile::load(&self.lock_path)?;
        
        // 3. 获取包信息
        let lock_pkg = lock.packages.get(package_name)
            .ok_or_else(|| anyhow::anyhow!("Lock 文件中找不到包: {}", package_name))?;
        
        let pkg_dir = self.packages_dir.join(package_name).join(&lock_pkg.version);
        let pkg = PackageJson::from_file(&pkg_dir.join("package.json"))?;
        
        // 4. 删除引用文件
        for file in &pkg.files {
            let relative = file.strip_prefix("Scripts/").unwrap_or(file);
            let ref_path = self.user_scripts_dir.join(relative);
            if ref_path.exists() {
                std::fs::remove_file(&ref_path)?;
                println!("  ✓ 删除: {}", relative);
            }
        }
        
        // 5. 更新 _tool_list.txt
        if let Some(ref moho) = pkg.moho {
            if let Some(ref tools) = moho.tools {
                if !tools.is_empty() {
                    let tool_ids: Vec<&str> = tools.iter().map(|t| t.id.as_str()).collect();
                    self.update_tool_list_remove(&tool_ids)?;
                }
            }
        }
        
        // 6. 检查依赖包
        if let Some(ref deps) = pkg.dependencies {
            for (dep_name, _) in deps {
                // 检查是否还有其他包依赖
                let still_needed = lock.packages.values()
                    .any(|p| p.dependencies.as_ref()
                        .map(|d| d.contains_key(dep_name))
                        .unwrap_or(false));
                
                if !still_needed {
                    println!("  依赖 {} 无其他包使用，可卸载", dep_name);
                    // 可选：递归卸载
                    // self.uninstall(dep_name)?;
                }
            }
        }
        
        // 7. 删除包目录
        std::fs::remove_dir_all(&pkg_dir)?;
        
        // 8. 更新 Lock 文件
        let mut lock = lock;
        lock.packages.remove(package_name);
        lock.save(&self.lock_path)?;
        
        println!("✓ 卸载完成");
        Ok(())
    }
    
    /// 列出已安装的包
    pub fn list(&self) -> Result<Vec<(String, PackageJson)>> {
        let mut packages = Vec::new();
        
        if !self.packages_dir.exists() {
            return Ok(packages);
        }
        
        for entry in std::fs::read_dir(&self.packages_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // 检查是否是 scoped 包 (@org/name)
                if entry.file_name().to_string_lossy().starts_with('@') {
                    for scoped in std::fs::read_dir(&path)? {
                        let scoped = scoped?;
                        collect_packages(&scoped.path(), &mut packages)?;
                    }
                } else {
                    collect_packages(&path, &mut packages)?;
                }
            }
        }
        
        Ok(packages)
    }
    
    /// 解析依赖
    fn resolve_dependencies(&self, pkg: &PackageJson) -> Result<Vec<(String, String, PackageJson)>> {
        let mut deps = Vec::new();
        
        if let Some(ref dependencies) = pkg.dependencies {
            for (dep_name, version_range) in dependencies {
                // 简化：直接使用版本号（实际应解析 semver）
                let version = version_range.trim_start_matches('^').trim_start_matches('~');
                
                // TODO: 从 registry 下载依赖包
                // 目前只检查是否已安装
                let dep_dir = self.packages_dir.join(dep_name).join(version);
                if dep_dir.exists() {
                    let dep_pkg = PackageJson::from_file(&dep_dir.join("package.json"))?;
                    deps.push((dep_name.clone(), version.to_string(), dep_pkg));
                } else {
                    println!("⚠ 依赖未安装: {}@{}", dep_name, version);
                    println!("  请先安装依赖包");
                }
            }
        }
        
        Ok(deps)
    }
    
    /// 生成引用文件
    fn generate_ref_files(&self, pkg_dir: &Path, pkg: &PackageJson, deps: &[(String, String, PackageJson)]) -> Result<()> {
        for file in &pkg.files {
            let src_path = pkg_dir.join(file);
            
            // 跳过不存在的文件
            if !src_path.exists() {
                continue;
            }
            
            let relative = file.strip_prefix("Scripts/").unwrap_or(file);
            let ref_path = self.user_scripts_dir.join(relative);
            
            // 创建父目录
            if let Some(parent) = ref_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            if file.ends_with(".lua") {
                // Lua 文件：生成引用
                let content = self.generate_lua_ref_content(pkg, deps, &src_path, &ref_path);
                std::fs::write(&ref_path, content)?;
                println!("  ✓ {}", relative);
            } else {
                // 其他文件：复制（处理符号链接）
                let metadata = std::fs::symlink_metadata(&src_path)?;
                if metadata.is_file() {
                    std::fs::copy(&src_path, &ref_path)?;
                    println!("  ✓ {}", relative);
                } else if metadata.is_symlink() {
                    // 解析符号链接
                    let target = std::fs::read_link(&src_path)?;
                    let target_path = if target.is_relative() {
                        src_path.parent().unwrap().join(&target)
                    } else {
                        target
                    };
                    if target_path.exists() && target_path.is_file() {
                        std::fs::copy(&target_path, &ref_path)?;
                        println!("  ✓ {}", relative);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 生成 Lua 引用文件内容
    fn generate_lua_ref_content(&self, pkg: &PackageJson, deps: &[(String, String, PackageJson)], src_path: &Path, ref_path: &Path) -> String {
        let base = self.packages_dir.display();
        
        let mut cpath_code = String::new();
        
        // 1. 本包 Modules 路径
        let pkg_modules = format!("{}/{}/Scripts/Modules/?.lua", base, pkg.name);
        cpath_code.push_str(&format!(
r#"-- 本包 Modules
local path = [[{}]]
if not package.path:find(path, 1, true) then
    package.path = path .. ";" .. package.path
end
"#, pkg_modules));
        
        // 2. 依赖包路径
        for (dep_name, dep_version, dep_pkg) in deps {
            // 如果依赖包有 main 或 exports["."]，添加根目录
            if dep_pkg.is_requireable() {
                let dep_root = format!("{}/{} Scripts/Modules/?.lua", base, dep_name);
                cpath_code.push_str(&format!(
r#"-- 依赖包 {} 根目录
local path = [[{}]]
if not package.path:find(path, 1, true) then
    package.path = path .. ";" .. package.path
end
"#, dep_name, dep_root));
            }
            
            // 始终添加 Modules 目录
            let dep_modules = format!("{}/{} Scripts/Modules/?.lua", base, dep_name);
            cpath_code.push_str(&format!(
r#"-- 依赖包 {} Modules
local path = [[{}]]
if not package.path:find(path, 1, true) then
    package.path = path .. ";" .. package.path
end
"#, dep_name, dep_modules));
        }
        
        format!(
r#"-- {} (引用文件)
-- 包: {}@{}
-- 由 moho-mate 自动生成，请勿手动修改

local base = [[{}]]

{}

-- 加载实际脚本
local script_path = [[{}]]
local ok, result = pcall(loadfile, script_path)
if ok then
    return result(...)
else
    error("[moho-mate] 加载失败: " .. tostring(result))
end
"#,
            ref_path.file_name().unwrap().to_string_lossy(),
            pkg.name, pkg.version,
            base,
            cpath_code,
            src_path.display()
        )
    }
    
    /// 更新 _tool_list.txt
    fn update_tool_list(&self, tools: &[ToolEntry], add: bool) -> Result<()> {
        let tool_list_path = self.user_scripts_dir.join("Tool/_tool_list.txt");
        
        let mut lines: Vec<String> = if tool_list_path.exists() {
            std::fs::read_to_string(&tool_list_path)?
                .lines()
                .map(String::from)
                .collect()
        } else {
            Vec::new()
        };
        
        if add {
            for tool in tools {
                let line = format!("tool\t{}\t\t{}", tool.id, tool.name);
                if !lines.iter().any(|l| l.split_whitespace().nth(1) == Some(&tool.id)) {
                    lines.push(line);
                }
            }
        }
        
        std::fs::write(&tool_list_path, lines.join("\n") + "\n")?;
        Ok(())
    }
    
    /// 从 _tool_list.txt 删除工具
    fn update_tool_list_remove(&self, tool_ids: &[&str]) -> Result<()> {
        let tool_list_path = self.user_scripts_dir.join("Tool/_tool_list.txt");
        
        if !tool_list_path.exists() {
            return Ok(());
        }
        
        let lines: Vec<String> = std::fs::read_to_string(&tool_list_path)?
            .lines()
            .filter(|line| {
                !line.split_whitespace()
                    .nth(1)
                    .map(|id| tool_ids.contains(&id))
                    .unwrap_or(false)
            })
            .map(String::from)
            .collect();
        
        std::fs::write(&tool_list_path, lines.join("\n") + "\n")?;
        Ok(())
    }
    
    /// 更新 Lock 文件
    fn update_lock(&self, pkg: &PackageJson, deps: &[(String, String, PackageJson)], resolved: Option<&str>) -> Result<()> {
        let mut lock = LockFile::load(&self.lock_path)?;
        
        // 添加主包
        let mut pkg_deps = HashMap::new();
        for (name, version, _) in deps {
            pkg_deps.insert(name.clone(), version.clone());
        }
        
        lock.packages.insert(pkg.name.clone(), LockPackage {
            version: pkg.version.clone(),
            resolved: resolved.map(String::from),
            integrity: None,
            dependencies: if pkg_deps.is_empty() { None } else { Some(pkg_deps) },
        });
        
        // 添加依赖包
        for (dep_name, dep_version, dep_pkg) in deps {
            lock.packages.insert(dep_name.clone(), LockPackage {
                version: dep_version.clone(),
                resolved: None,
                integrity: None,
                dependencies: dep_pkg.dependencies.clone(),
            });
        }
        
        lock.save(&self.lock_path)?;
        Ok(())
    }
    
    /// 查找已安装的版本
    fn find_installed_versions(&self, package_name: &str) -> Result<Vec<String>> {
        let pkg_dir = self.packages_dir.join(package_name);
        
        if !pkg_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut versions = Vec::new();
        for entry in std::fs::read_dir(&pkg_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                versions.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        
        Ok(versions)
    }
    
    /// 创建 node_modules 符号链接
    fn create_node_modules_symlinks(&self, pkg_dir: &Path, deps: &[(String, String, PackageJson)]) -> Result<()> {
        if deps.is_empty() {
            return Ok(());
        }
        
        let node_modules_dir = pkg_dir.join("node_modules");
        
        // 删除旧的 node_modules（如果存在）
        if node_modules_dir.exists() {
            std::fs::remove_dir_all(&node_modules_dir)?;
        }
        
        std::fs::create_dir_all(&node_modules_dir)?;
        
        for (dep_name, dep_version, _) in deps {
            // 目标: packages/@org/name/version/
            let dep_pkg_dir = self.packages_dir.join(dep_name).join(dep_version);
            
            if !dep_pkg_dir.exists() {
                eprintln!("  ⚠ 依赖包不存在: {}@{}", dep_name, dep_version);
                continue;
            }
            
            // 创建符号链接: node_modules/@org/name -> ../../../@org/name/version/
            let symlink_path = node_modules_dir.join(dep_name);
            
            // 确保父目录存在
            if let Some(parent) = symlink_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            // 计算相对路径
            let relative_path = pathdiff::diff_paths(&dep_pkg_dir, symlink_path.parent().unwrap())
                .unwrap_or_else(|| dep_pkg_dir.clone());
            
            // 创建符号链接
            #[cfg(unix)]
            std::os::unix::fs::symlink(&relative_path, &symlink_path)?;
            
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(&relative_path, &symlink_path)?;
            
            println!("  ✓ 链接: {} -> {}", dep_name, relative_path.display());
        }
        
        Ok(())
    }
    
    /// 获取配置
    pub fn get_config(&self) -> &PkgConfig {
        &self.config
    }
    
    /// 设置 registry
    pub fn set_registry(&mut self, registry: String) -> Result<()> {
        self.config.registry = registry;
        self.config.save(&self.base_dir)?;
        Ok(())
    }
    
    /// 获取用户脚本目录
    pub fn get_user_scripts_dir(&self) -> &Path {
        &self.user_scripts_dir
    }
    
    /// 获取包存储目录
    pub fn get_packages_dir(&self) -> &Path {
        &self.packages_dir
    }
}

// ============ 辅助函数 ============

/// 获取 Moho 用户脚本目录
fn get_moho_user_scripts_dir() -> Result<PathBuf> {
    let settings_path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
        .join("Lost Marble/Moho Pro/14/Moho Pro14.user.settings");
    
    if !settings_path.exists() {
        bail!("Moho 设置文件不存在: {:?}", settings_path);
    }
    
    let content = std::fs::read_to_string(&settings_path)?;
    
    for line in content.lines() {
        if line.contains("EditableFilesDir") {
            let path = line.split('"')
                .nth(3)
                .ok_or_else(|| anyhow::anyhow!("无法解析 EditableFilesDir"))?;
            return Ok(PathBuf::from(path).join("Scripts"));
        }
    }
    
    bail!("未找到 EditableFilesDir 配置")
}

/// 解压归档文件
fn extract_archive(archive_path: &Path, dest: &Path) -> Result<()> {
    let ext = archive_path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    
    match ext.as_str() {
        "zip" => {
            let file = std::fs::File::open(archive_path)?;
            let mut archive = zip::ZipArchive::new(file)
                .with_context(|| "解压 ZIP 失败")?;
            
            // 查找 package.json 所在的目录
            let mut pkg_dir_prefix = String::new();
            for i in 0..archive.len() {
                let file = archive.by_index(i)?;
                let name = file.name();
                if name.ends_with("package.json") {
                    // 获取 package.json 所在目录
                    pkg_dir_prefix = name.strip_suffix("package.json").unwrap_or("").to_string();
                    break;
                }
            }
            
            // 解压，去除前缀目录
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let name = file.name().to_string();  // 克隆名称
                
                // 去除前缀目录（如果有的话）
                let relative_name = if pkg_dir_prefix.is_empty() {
                    name.as_str()
                } else {
                    name.strip_prefix(&pkg_dir_prefix).unwrap_or(&name)
                };
                
                if relative_name.is_empty() {
                    continue; // 跳过空名称
                }
                
                let out_path = dest.join(relative_name);
                
                if file.is_dir() {
                    std::fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut outfile = std::fs::File::create(&out_path)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        }
        "gz" | "tgz" => {
            // TODO: 实现 tar.gz 解压
            bail!("暂不支持 tar.gz 格式");
        }
        _ => {
            bail!("不支持的归档格式: {}", ext);
        }
    }
    
    Ok(())
}

/// 递归复制目录
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        // 获取文件类型（不跟随符号链接）
        let metadata = std::fs::symlink_metadata(&src_path)?;
        let file_type = metadata.file_type();
        
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path)
                .with_context(|| format!("复制文件失败: {}", src_path.display()))?;
        } else if file_type.is_symlink() {
            // 解析符号链接并复制目标文件
            let target = std::fs::read_link(&src_path)
                .with_context(|| format!("读取符号链接失败: {}", src_path.display()))?;
            
            // 如果目标是相对路径，解析为绝对路径
            let target_path = if target.is_relative() {
                src_path.parent().unwrap().join(&target)
            } else {
                target
            };
            
            if target_path.exists() && target_path.is_file() {
                std::fs::copy(&target_path, &dst_path)
                    .with_context(|| format!("复制符号链接目标失败: {}", target_path.display()))?;
            } else {
                eprintln!("  ⚠ 符号链接目标不存在，跳过: {} -> {}", src_path.display(), target_path.display());
            }
        } else {
            eprintln!("  ⚠ 跳过特殊文件: {}", src_path.display());
        }
    }
    
    Ok(())
}

/// 收集包
fn collect_packages(pkg_dir: &Path, packages: &mut Vec<(String, PackageJson)>) -> Result<()> {
    // 遍历版本目录
    for version_entry in std::fs::read_dir(pkg_dir)? {
        let version_entry = version_entry?;
        let version_path = version_entry.path();
        
        if version_path.is_dir() {
            let pkg_json_path = version_path.join("package.json");
            if pkg_json_path.exists() {
                let pkg = PackageJson::from_file(&pkg_json_path)?;
                packages.push((pkg.name.clone(), pkg));
            }
        }
    }
    
    Ok(())
}

// ============ Registry API (P1) ============

/// Registry 包元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPackage {
    /// 包名
    pub name: String,
    /// 所有版本
    pub versions: HashMap<String, RegistryVersion>,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 作者
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// 关键词
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    /// 最新版本
    #[serde(rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
}

/// Registry 版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryVersion {
    /// 版本号
    pub version: String,
    /// 下载地址
    pub dist: DistInfo,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 依赖
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<HashMap<String, String>>,
}

/// Distribution 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistInfo {
    /// tarball 下载地址
    pub tarball: String,
    /// SHA512 校验
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    /// SHA1 校验（旧格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shasum: Option<String>,
}

impl PackageManager {
    /// 从 registry 获取包元数据
    pub fn fetch_package_metadata(&self, package_name: &str) -> Result<RegistryPackage> {
        let url = format!("{}/{}", self.config.registry, package_name);
        
        println!("▶ 获取包信息: {}", url);
        
        let response = http_get(&url)?;
        let pkg: RegistryPackage = serde_json::from_str(&response)
            .with_context(|| format!("解析 registry 响应失败: {}", package_name))?;
        
        Ok(pkg)
    }
    
    /// 搜索 registry 包
    pub fn search(&self, keyword: &str) -> Result<Vec<SearchResult>> {
        // npm registry 搜索 API
        let url = format!("{}/-/v1/search?text={}&size=20", self.config.registry, keyword);
        
        println!("▶ 搜索: {}", keyword);
        
        let response = http_get(&url)?;
        
        // npm search 响应格式
        #[derive(Debug, Deserialize)]
        struct SearchResponse {
            objects: Vec<SearchObject>,
        }
        
        #[derive(Debug, Deserialize)]
        struct SearchObject {
            package: SearchPackage,
        }
        
        #[derive(Debug, Deserialize)]
        struct SearchPackage {
            name: String,
            version: String,
            description: Option<String>,
            author: Option<String>,
            keywords: Option<Vec<String>>,
        }
        
        let search: SearchResponse = serde_json::from_str(&response)
            .with_context(|| "解析搜索响应失败")?;
        
        let results: Vec<SearchResult> = search.objects.iter()
            .filter(|o| o.package.name.starts_with("@maohou/") || !o.package.name.contains("/"))
            .map(|o| SearchResult {
                name: o.package.name.clone(),
                version: o.package.version.clone(),
                description: o.package.description.clone(),
                author: o.package.author.clone(),
            })
            .collect();
        
        Ok(results)
    }
    
    /// 从 registry 安装包
    pub fn install_from_registry(&self, package_name: &str, version_range: Option<&str>) -> Result<()> {
        println!("▶ 安装: {}", package_name);
        println!("  Registry: {}", self.config.registry);
        
        // 1. 获取包元数据
        let metadata = self.fetch_package_metadata(package_name)?;
        
        // 2. 解析版本
        let version = self.resolve_version(&metadata, version_range)?;
        println!("  版本: {}", version);
        
        // 3. 获取版本信息
        let version_info = metadata.versions.get(&version)
            .ok_or_else(|| anyhow::anyhow!("版本不存在: {}@{}", package_name, version))?;
        
        // 4. 下载 tarball
        let tarball_url = &version_info.dist.tarball;
        println!("  下载: {}", tarball_url);
        
        let temp_dir = tempfile::tempdir()?;
        let tarball_path = temp_dir.path().join("package.tgz");
        
        download_file(tarball_url, &tarball_path)?;
        
        // 5. 解压 tarball (tar.gz)
        extract_tarball(&tarball_path, temp_dir.path())?;
        
        // 6. 读取 package.json
        let pkg_json_path = temp_dir.path().join("package/package.json");
        if !pkg_json_path.exists() {
            bail!("下载的包缺少 package.json 文件");
        }
        
        let pkg = PackageJson::from_file(&pkg_json_path)?;
        
        // 7. 验证 files 字段（只警告，不阻塞）
        for file in &pkg.files {
            let file_path = temp_dir.path().join("package").join(file);
            if !file_path.exists() {
                eprintln!("  ⚠ 文件不存在: {} (跳过)", file);
            }
        }
        
        // 8. 安装依赖
        let deps = self.install_dependencies(&pkg)?;
        
        // 9. 安装主包
        let target_dir = self.packages_dir.join(&pkg.name).join(&pkg.version);
        if target_dir.exists() {
            println!("⚠ 包已安装: {}@{}, 跳过", pkg.name, pkg.version);
        } else {
            std::fs::create_dir_all(target_dir.parent().unwrap())?;
            copy_dir_all(&temp_dir.path().join("package"), &target_dir)?;
            println!("✓ 已安装: {}@{}", pkg.name, pkg.version);
        }
        
        // 10. 创建 node_modules 符号链接（依赖包）
        self.create_node_modules_symlinks(&target_dir, &deps)?;
        
        // 11. 生成引用文件
        self.generate_ref_files(&target_dir, &pkg, &deps)?;
        
        // 12. 更新 _tool_list.txt
        if let Some(ref moho) = pkg.moho {
            if let Some(ref tools) = moho.tools {
                if !tools.is_empty() {
                    self.update_tool_list(tools, true)?;
                }
            }
        }
        
        // 13. 更新 Lock 文件
        self.update_lock(&pkg, &deps, Some(tarball_url))?;
        
        println!("✓ 安装完成");
        Ok(())
    }
    
    /// 解析版本范围
    fn resolve_version(&self, metadata: &RegistryPackage, version_range: Option<&str>) -> Result<String> {
        match version_range {
            Some(range) => {
                // 简化版 semver 解析
                // 支持: "latest", "^1.0.0", "~1.0.0", "1.0.0"
                if range == "latest" {
                    metadata.dist_tags.get("latest")
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("无 latest 标签"))
                } else if range.starts_with('^') || range.starts_with('~') {
                    // ^1.0.0 或 ~1.0.0: 查找匹配的最新版本
                    let base = range.trim_start_matches('^').trim_start_matches('~');
                    self.find_matching_version(&metadata.versions, base)
                } else {
                    // 精确版本
                    if metadata.versions.contains_key(range) {
                        Ok(range.to_string())
                    } else {
                        bail!("版本不存在: {}", range)
                    }
                }
            }
            None => {
                // 默认使用 latest
                metadata.dist_tags.get("latest")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("无 latest 标签"))
            }
        }
    }
    
    /// 查找匹配的版本
    fn find_matching_version(&self, versions: &HashMap<String, RegistryVersion>, base: &str) -> Result<String> {
        // 解析基础版本 (如 "1.0.0" -> "1")
        let parts: Vec<&str> = base.split('.').collect();
        if parts.is_empty() {
            bail!("无效版本格式: {}", base);
        }
        
        let major = parts[0];
        
        // 查找所有匹配主版本的版本
        let matching: Vec<String> = versions.keys()
            .filter(|v| v.starts_with(major))
            .cloned()
            .collect();
        
        if matching.is_empty() {
            bail!("无匹配版本: {}", base);
        }
        
        // 返回最新版本（简化：直接取最大的）
        let latest = matching.iter()
            .max_by(|a, b| {
                // 简化版本比较
                let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
                let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();
                
                for i in 0..std::cmp::min(a_parts.len(), b_parts.len()) {
                    if a_parts[i] != b_parts[i] {
                        return a_parts[i].cmp(&b_parts[i]);
                    }
                }
                a_parts.len().cmp(&b_parts.len())
            })
            .unwrap();
        
        Ok(latest.clone())
    }
    
    /// 安装依赖包
    fn install_dependencies(&self, pkg: &PackageJson) -> Result<Vec<(String, String, PackageJson)>> {
        let mut deps = Vec::new();
        
        if let Some(ref dependencies) = pkg.dependencies {
            for (dep_name, version_range) in dependencies {
                println!("▶ 安装依赖: {}@{}", dep_name, version_range);
                
                // 检查是否已安装
                let installed = self.find_installed_versions(dep_name)?;
                
                if !installed.is_empty() {
                    // 已安装，使用现有版本
                    let version = installed[0].clone();
                    let pkg_dir = self.packages_dir.join(dep_name).join(&version);
                    let dep_pkg = PackageJson::from_file(&pkg_dir.join("package.json"))?;
                    deps.push((dep_name.clone(), version.clone(), dep_pkg));
                    println!("  ✓ 已存在: {}@{}", dep_name, version);
                } else {
                    // 未安装，从 registry 安装
                    self.install_from_registry(dep_name, Some(version_range))?;
                    
                    // 重新读取安装后的包信息
                    let lock = LockFile::load(&self.lock_path)?;
                    let lock_pkg = lock.packages.get(dep_name)
                        .ok_or_else(|| anyhow::anyhow!("依赖安装失败: {}", dep_name))?;
                    
                    let pkg_dir = self.packages_dir.join(dep_name).join(&lock_pkg.version);
                    let dep_pkg = PackageJson::from_file(&pkg_dir.join("package.json"))?;
                    deps.push((dep_name.clone(), lock_pkg.version.clone(), dep_pkg));
                }
            }
        }
        
        Ok(deps)
    }
}

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
}

// ============ HTTP 辅助函数 ============

/// HTTP GET 请求
fn http_get(url: &str) -> Result<String> {
    // 使用 ureq (轻量级 HTTP 客户端)
    // 如果没有 ureq，使用 curl 命令
    
    #[cfg(feature = "http-client")]
    {
        let response = ureq::get(url)
            .set("Accept", "application/json")
            .call()
            .with_context(|| format!("HTTP 请求失败: {}", url))?;
        
        let mut body = String::new();
        response.into_reader().read_to_string(&mut body)?;
        Ok(body)
    }
    
    #[cfg(not(feature = "http-client"))]
    {
        // 使用 curl 命令
        let output = std::process::Command::new("curl")
            .args(["-s", "-H", "Accept: application/json", url])
            .output()
            .with_context(|| format!("curl 请求失败: {}", url))?;
        
        if !output.status.success() {
            bail!("curl 请求失败: {}", url);
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// 下载文件
fn download_file(url: &str, dest: &Path) -> Result<()> {
    #[cfg(feature = "http-client")]
    {
        let response = ureq::get(url)
            .call()
            .with_context(|| format!("下载失败: {}", url))?;
        
        let mut file = std::fs::File::create(dest)?;
        std::io::copy(&mut response.into_reader(), &mut file)?;
        Ok(())
    }
    
    #[cfg(not(feature = "http-client"))]
    {
        // 使用 curl 下载
        let output = std::process::Command::new("curl")
            .args(["-s", "-o", dest.to_str().unwrap_or(""), url])
            .output()
            .with_context(|| format!("下载失败: {}", url))?;
        
        if !output.status.success() {
            bail!("下载失败: {}", url);
        }
        
        Ok(())
    }
}

/// 解压 tarball (tar.gz)
fn extract_tarball(tarball_path: &Path, dest: &Path) -> Result<()> {
    // macOS/Linux: 使用 tar 命令
    let output = std::process::Command::new("tar")
        .args(["-xzf", tarball_path.to_str().unwrap_or("")])
        .current_dir(dest)
        .output()
        .with_context(|| "解压 tarball 失败")?;
    
    if !output.status.success() {
        bail!("tar 解压失败: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(())
}

// ============ P2: 开发命令 ============

impl PackageManager {
    /// 创建包模板
    pub fn create(&self, name: &str, output_dir: Option<&Path>) -> Result<()> {
        println!("▶ 创建包模板: {}", name);
        
        let target_dir = if let Some(d) = output_dir {
            d.join(name)
        } else {
            std::path::PathBuf::from(name)
        };
        
        if target_dir.exists() {
            bail!("目录已存在: {:?}", target_dir);
        }
        
        // 创建目录结构
        std::fs::create_dir_all(&target_dir)?;
        std::fs::create_dir_all(target_dir.join("Scripts/Tool"))?;
        std::fs::create_dir_all(target_dir.join("Scripts/Modules"))?;
        std::fs::create_dir_all(target_dir.join("Scripts/Menu"))?;
        
        // 创建 package.json
        let pkg = PackageJson {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: Some("Moho script package".to_string()),
            author: None,
            license: Some("MIT".to_string()),
            main: Some("Scripts/Modules/init.lua".to_string()),
            exports: None,
            files: vec![
                "Scripts/Modules/init.lua".to_string(),
                "Scripts/Tool/example.lua".to_string(),
            ],
            dependencies: None,
            moho: Some(MohoConfig {
                min_version: Some("14.0".to_string()),
                max_version: None,
                tools: Some(vec![
                    ToolEntry {
                        id: "example".to_string(),
                        name: "Example Tool".to_string(),
                        group: None,
                    }
                ]),
            }),
        };
        
        let pkg_json = serde_json::to_string_pretty(&pkg)?;
        std::fs::write(target_dir.join("package.json"), pkg_json)?;
        println!("  ✓ package.json");
        
        // 创建示例脚本
        let init_lua = r#"-- init.lua
-- Package entry point

local M = {}

function M.hello()
    print("Hello from package!")
end

return M
"#;
        std::fs::write(target_dir.join("Scripts/Modules/init.lua"), init_lua)?;
        println!("  ✓ Scripts/Modules/init.lua");
        
        let tool_lua = format!(r#"-- example.lua
-- Example Tool Script
-- ScriptName = "ExampleTool"

function ExampleTool:Run(moho)
    print("=== Example Tool ===")
    print("Package: {}")
    
    -- 在 Moho 中执行操作
    local doc = moho.document
    if doc then
        print("文档名: " .. doc:Name())
        print("图层数: " .. doc:CountLayers())
    end
end
"#, name);
        std::fs::write(target_dir.join("Scripts/Tool/example.lua"), tool_lua)?;
        println!("  ✓ Scripts/Tool/example.lua");
        
        // 创建 README.md
        let readme = format!(r#"# {}

Moho script package.

## 安装

```bash
moho-mate pkg install ./{}.zip
```

## 使用

### Tool 脚本

Moho GUI -> Scripts -> Tool -> Example Tool

### Lua 模块

```lua
local pkg = require("{}")
pkg.hello()
```
"#, name, name, name);
        std::fs::write(target_dir.join("README.md"), readme)?;
        println!("  ✓ README.md");
        
        println!("\n✓ 包模板已创建: {:?}", target_dir);
        println!("\n下一步: 编辑脚本，然后打包:");
        println!("  moho-mate pkg pack {}", name);
        
        Ok(())
    }
    
    /// 打包为 zip
    pub fn pack(&self, pkg_dir: &Path) -> Result<PathBuf> {
        println!("▶ 打包: {:?}", pkg_dir);
        
        // 读取 package.json
        let pkg_json_path = pkg_dir.join("package.json");
        if !pkg_json_path.exists() {
            bail!("package.json 不存在: {:?}", pkg_dir);
        }
        
        let pkg = PackageJson::from_file(&pkg_json_path)?;
        println!("  包名: {}@{}", pkg.name, pkg.version);
        
        // 验证 files 字段（只警告，不阻塞）
        for file in &pkg.files {
            let file_path = pkg_dir.join(file);
            if !file_path.exists() {
                eprintln!("  ⚠ 文件不存在: {} (跳过)", file);
            }
        }
        
        // 创建 zip 文件名
        let zip_name = format!("{}-{}.zip", pkg.name.replace("@", "_at_").replace("/", "_"), pkg.version);
        let zip_path = pkg_dir.parent()
            .unwrap_or(pkg_dir)
            .join(&zip_name);
        
        // 使用 zip 命令（macOS/Linux 内置）
        let output = std::process::Command::new("zip")
            .arg("-r")
            .arg(&zip_path)
            .arg(".")
            .args(pkg.files.iter().map(|f| f.as_str()))
            .current_dir(pkg_dir)
            .output()
            .with_context(|| "zip 命令失败")?;
        
        if !output.status.success() {
            bail!("zip 失败: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        println!("\n✓ 打包完成: {:?}", zip_path);
        println!("\n安装: moho-mate pkg install {:?}", zip_path);
        
        Ok(zip_path)
    }
    
    /// 更新包（重新安装最新版本）
    pub fn update(&self, package_name: Option<&str>) -> Result<()> {
        if let Some(name) = package_name {
            // 更新指定包
            println!("▶ 更新: {}", name);
            
            // 1. 获取当前安装版本
            let lock = LockFile::load(&self.lock_path)?;
            let current = lock.packages.get(name);
            
            if current.is_none() {
                bail!("包未安装: {}", name);
            }
            
            // 2. 获取 registry 最新版本
            let metadata = self.fetch_package_metadata(name)?;
            let latest = metadata.dist_tags.get("latest")
                .ok_or_else(|| anyhow::anyhow!("无 latest 标签"))?;
            
            println!("  当前: {}@{}", name, current.unwrap().version);
            println!("  最新: {}@{}", name, latest);
            
            if current.unwrap().version == *latest {
                println!("  ✓ 已是最新版本");
                return Ok(());
            }
            
            // 3. 卸载旧版本
            self.uninstall(name)?;
            
            // 4. 安装新版本
            self.install_from_registry(name, Some("latest"))?;
        } else {
            // 更新所有包
            println!("▶ 更新所有包");
            
            let lock = LockFile::load(&self.lock_path)?;
            let packages: Vec<String> = lock.packages.keys().cloned().collect();
            
            for name in packages {
                self.update(Some(&name))?;
            }
        }
        
        Ok(())
    }
    
    /// 清理无用包
    pub fn prune(&self) -> Result<()> {
        println!("▶ 清理无用包");
        
        // 1. 获取所有已安装包
        let installed = self.list()?;
        let installed_names: Vec<String> = installed.iter().map(|(n, _)| n.clone()).collect();
        
        // 2. 从 Lock 文件获取依赖关系
        let lock = LockFile::load(&self.lock_path)?;
        
        // 3. 查找被依赖的包
        let mut used_packages: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        for (_, lock_pkg) in &lock.packages {
            // 包本身被使用
            if let Some(ref deps) = lock_pkg.dependencies {
                for (dep_name, _) in deps {
                    used_packages.insert(dep_name.clone());
                }
            }
        }
        
        // 4. 找出未被依赖的包（可选清理）
        let unused: Vec<String> = installed_names.iter()
            .filter(|n| !used_packages.contains(&n.to_string()))
            .cloned()
            .collect();
        
        if unused.is_empty() {
            println!("  ✓ 无无用包");
        } else {
            println!("  可清理的包:");
            for name in &unused {
                println!("    {}", name);
            }
            
            println!("\n  清理命令:");
            for name in &unused {
                println!("    moho-mate pkg uninstall {}", name);
            }
        }
        
        Ok(())
    }
}
