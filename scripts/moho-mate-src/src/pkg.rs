//! 脚本包管理模块 v2 — PnP 架构
//!
//! - importmap.json 替代 lock.json（严格遵循 HTML Import Maps 规范）
//! - package.preload 替代 package.path（Lua 原生依赖解析）
//! - 无软链接/junction，纯 loadfile 引用

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use serde::{Deserialize, Serialize};

const MOHO_USER_AGENT: &str = "moho-mate/0.1.0 (Moho script manager; https://maohou.com)";
const MOHO_RATE_LIMIT_MS: u64 = 2000; // 2s between requests
const MOHOSCRIPTS_BASE: &str = "https://mohoscripts.com";

/// Rate-limited HTTP helper for mohoscripts.com
static LAST_REQUEST: Mutex<Option<Instant>> = Mutex::new(None);

fn moho_throttle() {
    let mut last = LAST_REQUEST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(instant) = *last {
        let elapsed = instant.elapsed().as_millis() as u64;
        if elapsed < MOHO_RATE_LIMIT_MS {
            std::thread::sleep(std::time::Duration::from_millis(MOHO_RATE_LIMIT_MS - elapsed));
        }
    }
    *last = Some(Instant::now());
}

/// Check robots.txt (cached). Returns Ok if allowed or no robots.txt.
fn check_robots_txt() -> Result<()> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Result<bool>> = OnceLock::new();
    
    let result = CACHED.get_or_init(|| {
        let url = format!("{}/robots.txt", MOHOSCRIPTS_BASE);
        let resp = ureq::get(&url)
            .set("User-Agent", MOHO_USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .call();
        match resp {
            Err(ureq::Error::Status(404, _)) => Ok(true), // no robots.txt = allow
            Err(e) => Err(anyhow::anyhow!("robots.txt 检查失败: {}", e)),
            Ok(response) => {
                let text = response.into_string().unwrap_or_default();
                // Simple check: if page contains html doctype, it's not a real robots.txt
                if text.contains("<!DOCTYPE") || text.contains("<html") {
                    return Ok(true);
                }
                // Check Disallow for /scripts/ and /script/ and /downloads/
                let mut disallowed = false;
                let mut in_our_agent = false;
                for line in text.lines() {
                    let line = line.trim();
                    if line.starts_with("User-agent:") {
                        let agent = line.split(':').nth(1).unwrap_or("").trim();
                        in_our_agent = agent == "*" || agent == "moho-mate";
                    } else if line.starts_with("Disallow:") && in_our_agent {
                        let path = line.split(':').nth(1).unwrap_or("").trim();
                        if path == "/" || path.starts_with("/scripts") || path.starts_with("/script") || path.starts_with("/downloads") {
                            disallowed = true;
                            break;
                        }
                    }
                }
                Ok(!disallowed)
            }
        }
    });
    
    match result {
        Ok(true) => Ok(()),
        Ok(false) => bail!("robots.txt 禁止访问该路径，请尊重网站规则"),
        Err(e) => Err(anyhow::anyhow!("{}", e)),
    }
}

// ============ package.json 结构 ============

/// package.json 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageJson {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exports: Option<HashMap<String, ExportConfig>>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub author: Option<Author>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moho: Option<MohoConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Author {
    String(String),
    Object(AuthorObject),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorObject {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl Author {
    pub fn name(&self) -> &str {
        match self {
            Author::String(s) => s,
            Author::Object(obj) => &obj.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExportConfig {
    String(String),
    Object(ExportObject),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MohoConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl PackageJson {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取 package.json 失败: {:?}", path))?;
        let pkg: PackageJson = serde_json::from_str(&content)
            .with_context(|| format!("解析 package.json 失败: {:?}", path))?;
        Ok(pkg)
    }

    /// 获取 main 字段指定的入口路径（相对路径）
    /// 无 main 则返回 None（importmap 不生成裸名映射）
    pub fn get_main_path(&self) -> Option<String> {
        if let Some(ref main) = self.main {
            Some(main.clone())
        } else {
            None
        }
    }
}

// ============ ImportMap 结构（严格遵循 HTML Import Maps 规范）============

/// importmap.json — 只有 imports 和 scopes 两个标准字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMap {
    /// 全局映射：裸名/前缀 → store 绝对路径
    pub imports: HashMap<String, String>,
    /// 作用域映射：包路径 → 该包可见的依赖映射
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<HashMap<String, HashMap<String, String>>>,
}

impl ImportMap {
    /// 加载 importmap.json
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(ImportMap {
                imports: HashMap::new(),
                scopes: Some(HashMap::new()),
            });
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取 importmap 失败: {:?}", path))?;
        let map: ImportMap = serde_json::from_str(&content)
            .with_context(|| format!("解析 importmap 失败: {:?}", path))?;
        Ok(map)
    }

    /// 保存 importmap.json
    pub fn save(&self, path: &Path) -> Result<()> {
        // imports 键排序输出，方便阅读
        let mut imports: Vec<(String, String)> = self.imports.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        imports.sort_by(|a, b| a.0.cmp(&b.0));

        let scopes = self.scopes.as_ref().map(|s| {
            let mut sorted: Vec<(&String, &HashMap<String, String>)> = s.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            sorted
        });

        // 手动构建 JSON 以控制排序
        let mut json = String::from("{\n  \"imports\": {\n");
        for (i, (key, value)) in imports.iter().enumerate() {
            json.push_str(&format!("    {:?}: {:?}", key, value));
            if i < imports.len() - 1 { json.push_str(",\n"); } else { json.push_str("\n"); }
        }
        json.push_str("  }");

        if let Some(ref scope_list) = scopes {
            if !scope_list.is_empty() {
                json.push_str(",\n  \"scopes\": {\n");
                for (i, (scope_path, scope_map)) in scope_list.iter().enumerate() {
                    json.push_str(&format!("    {:?}: {{\n", scope_path));
                    let mut entries: Vec<(&String, &String)> = scope_map.iter().collect();
                    entries.sort_by(|a, b| a.0.cmp(&b.0));
                    for (j, (k, v)) in entries.iter().enumerate() {
                        json.push_str(&format!("      {:?}: {:?}", k, v));
                        if j < entries.len() - 1 { json.push_str(",\n"); } else { json.push_str("\n"); }
                    }
                    json.push_str("    }");
                    if i < scope_list.len() - 1 { json.push_str(",\n"); } else { json.push_str("\n"); }
                }
                json.push_str("  }");
            }
        }

        json.push_str("\n}\n");

        std::fs::write(path, json)
            .with_context(|| format!("写入 importmap 失败: {:?}", path))?;
        Ok(())
    }

    /// 添加包到 importmap
    /// - 裸名映射：package.json 有 main 时生成
    /// - 前缀映射：始终生成（包名/ → 包根目录）
    pub fn add_package(&mut self, pkg_name: &str, pkg_version: &str, store_path: &Path, pkg: &PackageJson) {
        let pkg_dir = store_path.join(pkg_name).join(pkg_version);

        // 前缀映射（始终存在）
        self.imports.insert(
            format!("{}/", pkg_name),
            format!("{}/", pkg_dir.display()),
        );

        // 裸名映射（有 main 时）
        if let Some(main) = pkg.get_main_path() {
            self.imports.insert(
                pkg_name.to_string(),
                format!("{}/{}", pkg_dir.display(), main),
            );
        }
    }

    /// 添加作用域（包的依赖隔离）
    pub fn add_scope(&mut self, pkg_name: &str, pkg_version: &str, store_path: &Path, deps: &[ResolvedDep]) {
        if deps.is_empty() {
            return;
        }

        let scopes = self.scopes.get_or_insert_with(HashMap::new);
        let pkg_dir = format!("{}/{}/{}/", store_path.display(), pkg_name, pkg_version);

        let mut scope_map = HashMap::new();

        for dep in deps {
            // 前缀映射
            scope_map.insert(
                format!("{}/", dep.name),
                format!("{}/{}/{}/", store_path.display(), dep.name, dep.version),
            );

            // 裸名映射
            if let Some(ref main) = dep.main {
                scope_map.insert(
                    dep.name.clone(),
                    format!("{}/{}/{}/{}", store_path.display(), dep.name, dep.version, main),
                );
            }
        }

        scopes.insert(pkg_dir, scope_map);
    }

    /// 移除包
    pub fn remove_package(&mut self, pkg_name: &str, pkg_version: &str, store_path: &Path, pkg: &PackageJson) {
        let pkg_dir = store_path.join(pkg_name).join(pkg_version);

        // 移除裸名映射
        self.imports.remove(pkg_name);
        // 移除前缀映射
        self.imports.remove(&format!("{}/", pkg_name));

        // 移除 scope
        if let Some(ref mut scopes) = self.scopes {
            let scope_key = format!("{}/{}/{}/", store_path.display(), pkg_name, pkg_version);
            scopes.remove(&scope_key);
        }
    }
}

/// 解析后的依赖信息
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
    pub main: Option<String>,
}

// ============ mohoscripts.com 对接 ============

/// mohoscripts.com 脚本信息
#[derive(Debug, Clone)]
pub struct MohoScript {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub script_type: Option<String>,
    pub version: Option<String>,
    pub moho_version: Option<String>,
    pub downloads: u32,
    pub body_id: u32,
    pub csrf_token: Option<String>,
}

/// Parse a single search results page, returns (results, max_page)
fn parse_search_page(doc: &scraper::Html) -> (Vec<MohoScript>, u32) {
    let card_selector = scraper::Selector::parse(".script-card").unwrap();
    let name_selector = scraper::Selector::parse("a[href^=\"https://mohoscripts.com/script/\"]").unwrap();
    let desc_selector = scraper::Selector::parse(".script-entity-short-description").unwrap();
    let dl_selector = scraper::Selector::parse(".download-count b").unwrap();
    let link_selector = scraper::Selector::parse(".details-button").unwrap();
    let page_selector = scraper::Selector::parse(".pagination-page-number").unwrap();
    
    // Extract max page number
    let max_page: u32 = doc.select(&page_selector)
        .filter_map(|e| e.text().collect::<String>().trim().parse().ok())
        .max()
        .unwrap_or(1);
    
    let mut results = Vec::new();
    
    for card in doc.select(&card_selector) {
        let desc_elem = card.select(&desc_selector).next();
        let dl_elem = card.select(&dl_selector).next();
        let link_elem = card.select(&link_selector).next();
        
        let slug = if let Some(elem) = link_elem {
            let href = elem.value().attr("href").unwrap_or("");
            href.strip_prefix("https://mohoscripts.com/script/")
                .unwrap_or(href)
                .to_string()
        } else {
            String::new()
        };

        let name_elem_filtered = card.select(&name_selector)
            .find(|e| e.value().attr("class").unwrap_or("") != "button details-button");
        let name = if let Some(elem) = name_elem_filtered {
            let n = elem.value().attr("title")
                .map(|s| s.to_string())
                .unwrap_or_else(|| elem.text().collect::<String>());
            n.trim().to_string()
        } else if let Some(elem) = link_elem {
            let href = elem.value().attr("href").unwrap_or("");
            href.strip_prefix("https://mohoscripts.com/script/")
                .unwrap_or(href)
                .replace('-', " ")
        } else {
            continue;
        };

        if slug.is_empty() {
            continue;
        }
        
        let description = desc_elem.map(|e| e.text().collect::<String>().trim().to_string());
        let downloads: u32 = dl_elem
            .and_then(|e| e.text().collect::<String>().trim().parse().ok())
            .unwrap_or(0);
        
        results.push(MohoScript {
            slug,
            name,
            description,
            author: None,
            script_type: None,
            version: None,
            moho_version: None,
            downloads,
            body_id: 0,
            csrf_token: None,
        });
    }
    
    (results, max_page)
}

impl PackageManager {
    /// 从 mohoscripts.com 搜索脚本
    pub fn search_mohoscripts(&self, keyword: &str) -> Result<Vec<MohoScript>> {
        self.search_mohoscripts_paged(keyword, None)
    }

    pub fn search_mohoscripts_paged(&self, keyword: &str, max_pages: Option<usize>) -> Result<Vec<MohoScript>> {
        check_robots_txt()?;
        println!("▶ 搜索 mohoscripts.com: {}", keyword);
        
        let mut all_results = Vec::new();
        let mut page = 1u32;
        
        loop {
            let url = if page == 1 {
                format!("{}/scripts/search/{}", MOHOSCRIPTS_BASE, keyword)
            } else {
                format!("{}/scripts/list/search={}/created:desc/{}", MOHOSCRIPTS_BASE, keyword, page)
            };
            
            moho_throttle();
            let html = ureq::get(&url)
                .set("User-Agent", MOHO_USER_AGENT)
                .timeout(std::time::Duration::from_secs(20))
                .call()
                .map_err(|e| anyhow::anyhow!("搜索请求失败 (第{}页): {}", page, e))?
                .into_string()
                .map_err(|e| anyhow::anyhow!("读取响应失败: {}", e))?;
            
            let doc = scraper::Html::parse_document(&html);
            let (results, max_page) = parse_search_page(&doc);
            let count = results.len();
            all_results.extend(results);
            
            if page == 1 && max_page > 1 {
                let limit = max_pages.unwrap_or(1);
                let fetch_pages = std::cmp::min(max_page as usize, limit);
                if fetch_pages > 1 {
                    println!("  共 {} 页，获取前 {} 页", max_page, fetch_pages);
                }
            }
            
            // Stop if no more results or reached end
            if count == 0 {
                break;
            }
            if let Some(mp) = max_pages {
                if page as usize >= mp {
                    break;
                }
            } else {
                // No max_pages specified = first page only
                break;
            }
            if page as usize >= max_page as usize {
                break;
            }
            page += 1;
        }
        
        Ok(all_results)
    }
    
    /// 获取脚本详情（含 CSRF token）
    pub fn info_mohoscripts(&self, slug: &str) -> Result<MohoScript> {
        check_robots_txt()?;
        println!("▶ 获取详情: {}", slug);
        
        let url = format!("{}/script/{}", MOHOSCRIPTS_BASE, slug);
        moho_throttle();
        let html = ureq::get(&url)
            .set("User-Agent", MOHO_USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .call()
            .map_err(|e| anyhow::anyhow!("请求失败: {}", e))?
            .into_string()
            .map_err(|e| anyhow::anyhow!("读取响应失败: {}", e))?;
        
        let doc = scraper::Html::parse_document(&html);
        
        // 提取 CSRF token
        let csrf_selector = scraper::Selector::parse("input[name=csrf_token]").unwrap();
        let csrf_token = doc.select(&csrf_selector).next()
            .and_then(|e| e.value().attr("value"))
            .map(String::from);
        
        // 提取 script_body_ids[]
        let body_id_selector = scraper::Selector::parse("input[name='script_body_ids[]']").unwrap();
        let body_id = doc.select(&body_id_selector).next()
            .and_then(|e| e.value().attr("value"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        
        // 提取 zip_filename
        let zip_selector = scraper::Selector::parse("input[name=zip_filename]").unwrap();
        let zip_filename = doc.select(&zip_selector).next()
            .and_then(|e| e.value().attr("value"))
            .map(String::from);
        
        // 提取其他元信息
        let desc_selector = scraper::Selector::parse(".script-entity-short-description").unwrap();
        let description = doc.select(&desc_selector).next()
            .map(|e| e.text().collect::<String>().trim().to_string());
        
        let type_selector = scraper::Selector::parse(".script-type a").unwrap();
        let script_type = doc.select(&type_selector).next()
            .map(|e| e.text().collect::<String>().trim().to_string());
        
        let version_selector = scraper::Selector::parse(".script-version b").unwrap();
        let version = doc.select(&version_selector).next()
            .map(|e| e.text().collect::<String>().trim().to_string());
        
        let moho_ver_selector = scraper::Selector::parse(".created-for-moho-version b").unwrap();
        let moho_version = doc.select(&moho_ver_selector).next()
            .map(|e| e.text().collect::<String>().trim().to_string());
        
        Ok(MohoScript {
            slug: slug.to_string(),
            name: zip_filename.unwrap_or(slug.to_string()),
            description,
            author: None,
            script_type,
            version,
            moho_version,
            downloads: 0,
            body_id,
            csrf_token,
        })
    }
    
    /// 从 mohoscripts.com 下载脚本
    pub fn download_mohoscripts(&self, slug: &str) -> Result<PathBuf> {
        // 1. 先 GET 详情页获取 CSRF token 和 session cookie
        let url = format!("{}/script/{}", MOHOSCRIPTS_BASE, slug);
        moho_throttle();
        let response = ureq::get(&url)
            .set("User-Agent", MOHO_USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .call()
            .map_err(|e| anyhow::anyhow!("获取详情页失败: {}", e))?;
        
        // 提取 session cookie
        let session_cookie = response.header("set-cookie")
            .and_then(|c| c.split(';').next())
            .map(String::from)
            .unwrap_or_default();
        
        let html = response.into_string()
            .map_err(|e| anyhow::anyhow!("读取详情页失败: {}", e))?;
        
        // 解析 CSRF token
        let doc = scraper::Html::parse_document(&html);
        let csrf_selector = scraper::Selector::parse("input[name=csrf_token]").unwrap();
        let csrf_token = doc.select(&csrf_selector).next()
            .and_then(|e| e.value().attr("value"))
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("无法获取 CSRF token"))?;
        
        // 提取 script_body_ids[]
        let body_id_selector = scraper::Selector::parse("input[name='script_body_ids[]']").unwrap();
        let body_id: u32 = doc.select(&body_id_selector).next()
            .and_then(|e| e.value().attr("value"))
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("无法获取脚本 ID"))?;
        
        println!("  下载: {} (body_id={}, csrf={})", slug, body_id, &csrf_token[..8]);
        
        // 2. POST 下载（带 session cookie）
        let download_url = format!("{}/downloads/scripts", MOHOSCRIPTS_BASE);
        moho_throttle();
        let mut response = ureq::post(&download_url)
            .set("User-Agent", MOHO_USER_AGENT)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .set("Referer", &url)
            .set("Cookie", &session_cookie)
            .send_string(&format!(
                "zip_filename={}&script_body_ids[]={}&csrf_token={}&submit=Download",
                slug, body_id, csrf_token
            ))
            .map_err(|e| anyhow::anyhow!("下载请求失败: {}", e))?
            .into_reader();;
        
        let mut buf = Vec::new();
        std::io::copy(&mut response, &mut buf)
            .map_err(|e| anyhow::anyhow!("读取下载内容失败: {}", e))?;
        
        // 3. 保存到临时文件
        let temp_dir = std::env::temp_dir();
        let zip_path = temp_dir.join(format!("{}.zip", slug));
        std::fs::write(&zip_path, &buf)?;
        
        println!("  已保存: {} ({} bytes)", zip_path.display(), buf.len());
        
        Ok(zip_path)
    }
    
    /// 安装 mohoscripts.com 脚本（下载 + 包装成 package.json + 安装）
    pub fn install_mohoscripts(&self, slug: &str) -> Result<()> {
        println!("▶ 安装 mohoscripts.com 脚本: {}", slug);
        
        // 1. 下载
        let zip_path = self.download_mohoscripts(slug)?;
        
        // 2. 解压
        let temp_dir = tempfile::tempdir()?;
        let file = std::fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        // 找到根目录前缀
        let mut root_prefix = String::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name();
            if name.ends_with(".lua") {
                // 提取根目录
                root_prefix = name.split('/').next().unwrap_or("").to_string();
                break;
            }
        }
        
        // 解压所有文件
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            
            // 跳过根目录前缀
            let relative = if !root_prefix.is_empty() && name.starts_with(&root_prefix) {
                name.strip_prefix(&format!("{}/", root_prefix)).unwrap_or(&name)
            } else {
                &name
            };
            
            if relative.is_empty() || relative.ends_with('/') { continue; }
            
            let out_path = temp_dir.path().join(relative);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
        
        // 3. 获取脚本信息
        let info = self.info_mohoscripts(slug)?;
        
        // 4. 生成 package.json（包装裸脚本）
        let package_json = PackageJson {
            name: format!("mohoscripts/{}", slug),
            version: info.version.unwrap_or_else(|| "1.0.0".to_string()),
            main: Some("Scripts/Tool/main.lua".to_string()),
            exports: None,
            files: vec!["Scripts/".to_string()],
            description: info.description,
            author: info.author.map(Author::String),
            license: Some("Unknown".to_string()),
            dependencies: None,
            moho: Some(MohoConfig {
                min_version: info.moho_version.clone(),
                max_version: None,
                tools: Some(vec![ToolEntry {
                    id: slug.replace('-', "_"),
                    name: info.name.clone(),
                    group: None,
                }]),
            }),
        };
        
        // 5. 找到 .lua 文件，移动到 Scripts/Tool/main.lua
        let scripts_dir = temp_dir.path().join("Scripts/Tool");
        std::fs::create_dir_all(&scripts_dir)?;
        
        // 查找第一个 .lua 文件作为入口
        for entry in walkdir::WalkDir::new(temp_dir.path()) {
            let entry = entry?;
            if entry.path().extension().map(|e| e == "lua").unwrap_or(false) {
                if entry.path().parent() != Some(scripts_dir.as_path()) {
                    std::fs::copy(entry.path(), scripts_dir.join("main.lua"))?;
                    break;
                }
            }
        }
        
        // 6. 写入 package.json
        let pkg_json_path = temp_dir.path().join("package.json");
        std::fs::write(&pkg_json_path, serde_json::to_string_pretty(&package_json)?)?;
        
        // 7. 打包并安装
        let final_zip = temp_dir.path().join(format!("{}.zip", slug));
        {
            let file = std::fs::File::create(&final_zip)?;
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            
            // package.json
            zip.start_file("package.json", options)?;
            std::io::copy(&mut std::fs::File::open(&pkg_json_path)?, &mut zip)?;
            
            // Scripts/
            add_dir_to_zip(&mut zip, temp_dir.path(), &scripts_dir, &options)?;
            zip.finish()?;
        }
        
        // 8. 安装
        self.install_local(&final_zip)?;
        
        println!("✓ 安装完成: {}", slug);
        Ok(())
    }
}

// ============ 配置文件结构 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkgConfig {
    #[serde(default = "default_registry")]
    pub registry: String,
}

fn default_registry() -> String {
    "https://mirrors.cloud.tencent.com/npm".to_string()
}

impl Default for PkgConfig {
    fn default() -> Self {
        Self { registry: default_registry() }
    }
}

impl PkgConfig {
    pub fn load(base_dir: &Path) -> Result<Self> {
        let config_path = base_dir.join("config.json");
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&config_path)?;
        let config: PkgConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, base_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(base_dir)?;
        let config_path = base_dir.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

// ============ 包管理器 ============

pub struct PackageManager {
    /// 基础目录（com.maohou.moho-mate）
    base_dir: PathBuf,
    /// 包存储目录 (store)
    packages_dir: PathBuf,
    /// 用户 Moho Pro 目录（importmap.json 所在目录）
    user_content_dir: PathBuf,
    /// 用户脚本目录
    user_scripts_dir: PathBuf,
    /// 配置
    config: PkgConfig,
}

impl PackageManager {
    pub fn new() -> Result<Self> {
        let base_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?
            .join("com.maohou.moho-mate");

        let packages_dir = base_dir.join("packages");

        let user_scripts_dir = get_moho_user_scripts_dir()
            .unwrap_or_else(|_| {
                dirs::document_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("moho_user_content/Moho Pro/Scripts")
            });

        // user_content_dir = user_scripts_dir 的父目录（Moho Pro/）
        let user_content_dir = user_scripts_dir.parent()
            .ok_or_else(|| anyhow::anyhow!("无法获取用户内容目录"))?
            .to_path_buf();

        let config = PkgConfig::load(&base_dir)?;

        Ok(Self {
            base_dir,
            packages_dir,
            user_content_dir,
            user_scripts_dir,
            config,
        })
    }

    /// importmap.json 路径
    fn importmap_path(&self) -> PathBuf {
        self.user_content_dir.join("moho-mate.importmap.json")
    }

    // ===== 安装 =====

    pub fn install_local(&self, archive_path: &Path) -> Result<()> {
        println!("▶ 安装: {}", archive_path.display());

        // 1. 解压到临时目录
        let temp_dir = tempfile::tempdir()?;
        extract_archive(archive_path, temp_dir.path())?;

        // 2. 读取 package.json
        let pkg_json_path = temp_dir.path().join("package.json");
        if !pkg_json_path.exists() {
            bail!("压缩包缺少 package.json 文件");
        }
        let pkg = PackageJson::from_file(&pkg_json_path)?;

        // 3. 验证 files（只警告）
        for file in &pkg.files {
            if !temp_dir.path().join(file).exists() {
                eprintln!("  ⚠ 文件不存在: {} (跳过)", file);
            }
        }

        // 4. 安装到 store
        let target_dir = self.packages_dir.join(&pkg.name).join(&pkg.version);
        if target_dir.exists() {
            println!("  包已存在: {}@{}, 跳过复制", pkg.name, pkg.version);
        } else {
            std::fs::create_dir_all(target_dir.parent().unwrap())?;
            copy_dir_all(temp_dir.path(), &target_dir)?;
            println!("✓ 已存储: {}@{}", pkg.name, pkg.version);
        }

        // 5. 解析并安装依赖
        let deps = self.resolve_and_install_deps(&pkg)?;

        // 6. 更新 importmap
        let mut importmap = ImportMap::load(&self.importmap_path())?;
        importmap.add_package(&pkg.name, &pkg.version, &self.packages_dir, &pkg);
        // 为每个依赖也添加到 imports
        for dep in &deps {
            let dep_pkg = PackageJson::from_file(
                &self.packages_dir.join(&dep.name).join(&dep.version).join("package.json")
            )?;
            importmap.add_package(&dep.name, &dep.version, &self.packages_dir, &dep_pkg);
        }
        // 添加 scope（依赖隔离）
        importmap.add_scope(&pkg.name, &pkg.version, &self.packages_dir, &deps);
        // 为依赖也添加 scope
        for dep in &deps {
            let dep_deps = self.resolve_deps(&dep.name, &dep.version)?;
            if !dep_deps.is_empty() {
                importmap.add_scope(&dep.name, &dep.version, &self.packages_dir, &dep_deps);
            }
        }
        importmap.save(&self.importmap_path())?;
        println!("✓ importmap 已更新");

        // 7. 生成引用文件
        self.generate_ref_files(&pkg, &deps)?;

        // 8. 更新 _tool_list.txt
        if let Some(ref moho) = pkg.moho {
            if let Some(ref tools) = moho.tools {
                if !tools.is_empty() {
                    self.update_tool_list(tools, true)?;
                }
            }
        }

        println!("✓ 安装完成");
        Ok(())
    }

    /// 解析依赖并安装缺失的
    fn resolve_and_install_deps(&self, pkg: &PackageJson) -> Result<Vec<ResolvedDep>> {
        let mut deps = Vec::new();
        if let Some(ref dependencies) = pkg.dependencies {
            for (dep_name, version_range) in dependencies {
                let version = version_range.trim_start_matches('^').trim_start_matches('~');
                let dep_dir = self.packages_dir.join(dep_name).join(version);

                if dep_dir.exists() {
                    let dep_pkg = PackageJson::from_file(&dep_dir.join("package.json"))?;
                    deps.push(ResolvedDep {
                        name: dep_name.clone(),
                        version: version.to_string(),
                        main: dep_pkg.get_main_path(),
                    });
                    println!("  ✓ 依赖已存在: {}@{}", dep_name, version);
                } else {
                    // TODO: 从 registry 安装
                    println!("  ⚠ 依赖未安装: {}@{}", dep_name, version);
                }
            }
        }
        Ok(deps)
    }

    /// 解析已安装包的依赖（不安装）
    fn resolve_deps(&self, pkg_name: &str, pkg_version: &str) -> Result<Vec<ResolvedDep>> {
        let pkg_dir = self.packages_dir.join(pkg_name).join(pkg_version);
        let pkg = PackageJson::from_file(&pkg_dir.join("package.json"))?;

        let mut deps = Vec::new();
        if let Some(ref dependencies) = pkg.dependencies {
            for (dep_name, version_range) in dependencies {
                let version = version_range.trim_start_matches('^').trim_start_matches('~');
                let dep_dir = self.packages_dir.join(dep_name).join(version);

                if dep_dir.exists() {
                    let dep_pkg = PackageJson::from_file(&dep_dir.join("package.json"))?;
                    deps.push(ResolvedDep {
                        name: dep_name.clone(),
                        version: version.to_string(),
                        main: dep_pkg.get_main_path(),
                    });
                }
            }
        }
        Ok(deps)
    }

    // ===== 引用文件生成 =====

    /// 生成引用文件（Scripts/ 下的 .lua 文件）
    fn generate_ref_files(&self, pkg: &PackageJson, deps: &[ResolvedDep]) -> Result<()> {
        let pkg_dir = self.packages_dir.join(&pkg.name).join(&pkg.version);

        // 收集所有实际文件（展开目录）
        let mut files_to_process: Vec<PathBuf> = Vec::new();
        for pattern in &pkg.files {
            let full_path = pkg_dir.join(pattern);
            if full_path.is_dir() {
                // 递归收集目录下的文件
                collect_files_recursive(&full_path, &mut files_to_process, &pkg_dir)?;
            } else if full_path.exists() {
                files_to_process.push(full_path);
            }
        }

        for src_path in files_to_process {
            let relative = src_path.strip_prefix(&pkg_dir).unwrap_or(&src_path);
            let script_relative = relative.strip_prefix("Scripts/").unwrap_or(relative);
            let ref_path = self.user_scripts_dir.join(script_relative);

            if let Some(parent) = ref_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            if relative.to_string_lossy().ends_with(".lua") {
                let content = self.generate_lua_ref(pkg, deps, &src_path);
                std::fs::write(&ref_path, content)?;
                println!("  ✓ 引用: {}", script_relative.display());
            } else {
                std::fs::copy(&src_path, &ref_path)?;
                println!("  ✓ 复制: {}", script_relative.display());
            }
        }
        Ok(())
    }

    /// 生成 Lua 引用文件内容（使用 package.preload）
    fn generate_lua_ref(&self, pkg: &PackageJson, deps: &[ResolvedDep], src_path: &Path) -> String {
        let script_path = src_path.display();
        let file_name = src_path.file_name().unwrap().to_string_lossy();

        let mut lines = Vec::new();
        lines.push(format!("-- {} (引用文件)", file_name));
        lines.push(format!("-- 包: {}@{}", pkg.name, pkg.version));
        lines.push("-- 由 moho-mate 自动生成，请勿修改".to_string());
        lines.push(String::new());

        // package.preload 注册
        lines.push("-- 注册依赖到 package.preload".to_string());

        if let Some(ref main) = pkg.main {
            let main_abs = format!("{}/{}/{}/{}", self.packages_dir.display(), pkg.name, pkg.version, main);
            lines.push(format!("package.preload[\"{}\"] = function() return dofile(\"{}\") end", pkg.name, main_abs));
        }

        for dep in deps {
            if let Some(ref main) = dep.main {
                let dep_abs = format!("{}/{}/{}/{}", self.packages_dir.display(), dep.name, dep.version, main);
                lines.push(format!("package.preload[\"{}\"] = function() return dofile(\"{}\") end", dep.name, dep_abs));
            }
        }

        lines.push(String::new());
        lines.push(format!("-- 加载实际脚本"));
        lines.push(format!("return loadfile(\"{}\")()", script_path));

        lines.join("\n")
    }

    // ===== 卸载 =====

    pub fn uninstall(&self, package_name: &str) -> Result<()> {
        println!("▶ 卸载: {}", package_name);

        // 1. 找到已安装的版本
        let versions = self.find_installed_versions(package_name)?;
        if versions.is_empty() {
            bail!("包未安装: {}", package_name);
        }

        // 2. 读取 importmap 获取当前版本
        let importmap = ImportMap::load(&self.importmap_path())?;
        let version = versions.into_iter().next().unwrap(); // 取第一个版本

        let pkg_dir = self.packages_dir.join(package_name).join(&version);
        let pkg = PackageJson::from_file(&pkg_dir.join("package.json"))?;

        // 3. 检查是否有其他包依赖此包
        if let Some(ref scopes) = importmap.scopes {
            for (scope_path, scope_map) in scopes {
                // 跳过自己
                if scope_path.contains(&format!("/{}/{}/", package_name, version)) {
                    continue;
                }
                if scope_map.contains_key(package_name) {
                    println!("  ⚠ 包 {} 被其他包依赖", package_name);
                    // TODO: 确认提示
                }
            }
        }

        // 4. 删除引用文件
        let pkg_dir_path = self.packages_dir.join(package_name).join(&version);
        let mut files_to_remove: Vec<PathBuf> = Vec::new();
        for pattern in &pkg.files {
            let full_path = pkg_dir_path.join(pattern);
            if full_path.is_dir() {
                collect_files_recursive(&full_path, &mut files_to_remove, &pkg_dir_path)?;
            } else if full_path.exists() {
                files_to_remove.push(full_path);
            }
        }
        for src_path in &files_to_remove {
            let relative = src_path.strip_prefix(&pkg_dir_path).unwrap_or(src_path);
            let script_relative = relative.strip_prefix("Scripts/").unwrap_or(relative);
            let ref_path = self.user_scripts_dir.join(script_relative);
            if ref_path.exists() {
                std::fs::remove_file(&ref_path)?;
                println!("  ✓ 删除: {}", script_relative.display());
            }
        }

        // 5. 更新 _tool_list.txt
        if let Some(ref moho) = pkg.moho {
            if let Some(ref tools) = moho.tools {
                let tool_ids: Vec<&str> = tools.iter().map(|t| t.id.as_str()).collect();
                self.update_tool_list_remove(&tool_ids)?;
            }
        }

        // 6. 删除 store 中的包
        std::fs::remove_dir_all(&pkg_dir)?;

        // 7. 更新 importmap
        let mut importmap = importmap;
        importmap.remove_package(package_name, &version, &self.packages_dir, &pkg);
        importmap.save(&self.importmap_path())?;

        println!("✓ 卸载完成");
        Ok(())
    }

    // ===== 列表/查询 =====

    pub fn list(&self) -> Result<Vec<(String, PackageJson)>> {
        let mut packages = Vec::new();
        if !self.packages_dir.exists() {
            return Ok(packages);
        }

        for entry in std::fs::read_dir(&self.packages_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('@') {
                    // @scope/package 格式
                    for scoped in std::fs::read_dir(&path)? {
                        let scoped = scoped?;
                        collect_packages(&scoped.path(), &mut packages)?;
                    }
                } else {
                    // 可能是 prefix/package 格式（如 mohoscripts/ss_count_layers）
                    // 也可能是普通包
                    // 检查是否有版本目录
                    let has_versions = std::fs::read_dir(&path)?
                        .filter_map(|e| e.ok())
                        .any(|e| e.path().is_dir() && e.file_name().to_string_lossy().chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false));
                    
                    if has_versions {
                        // 直接是版本目录，普通包
                        collect_packages(&path, &mut packages)?;
                    } else {
                        // 可能是前缀目录，继续遍历
                        for sub_entry in std::fs::read_dir(&path)? {
                            let sub_entry = sub_entry?;
                            let sub_path = sub_entry.path();
                            if sub_path.is_dir() {
                                collect_packages(&sub_path, &mut packages)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(packages)
    }

    // ===== 辅助方法 =====

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

    // ===== 创建包模板 =====

    pub fn create(&self, name: &str, output_dir: Option<&Path>) -> Result<()> {
        let out = match output_dir {
            Some(d) => d.to_path_buf(),
            None => std::env::current_dir()?.join(name.replace('/', "-")),
        };

        if out.exists() {
            bail!("目录已存在: {:?}", out);
        }

        println!("▶ 创建包模板: {}", name);

        // 目录结构
        std::fs::create_dir_all(out.join("Scripts/Tool"))?;
        std::fs::create_dir_all(out.join("Scripts/Modules"))?;

        // package.json
        let pkg_json = PackageJson {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            main: Some("Scripts/Modules/init.lua".to_string()),
            exports: None,
            files: vec![
                "Scripts/".to_string(),
            ],
            description: Some(format!("{} moho script package", name)),
            author: None,
            license: Some("MIT".to_string()),
            dependencies: None,
            moho: None,
        };
        let json_content = serde_json::to_string_pretty(&pkg_json)?;
        std::fs::write(out.join("package.json"), json_content)?;
        println!("  ✓ package.json");

        // 入口模块
        let _module_name = name.split('/').last().unwrap_or(name).replace("-", "_");
        let init_lua = format!(
"-- {} 入口模块
local M = {{}}

function M.hello()
    print('Hello from {}!')
end

return M
", name, name);
        std::fs::write(out.join("Scripts/Modules/init.lua"), init_lua)?;
        println!("  ✓ Scripts/Modules/init.lua");

        // README
        let readme = format!("# {}\n\nMoho script package.\n\n## 安装\n\n```bash\nmoho-mate pkg install ./{}.zip\n```\n", name, name.replace('/', "-"));
        std::fs::write(out.join("README.md"), readme)?;
        println!("  ✓ README.md");

        println!("\n✓ 包模板已创建: {:?}", out);
        println!("  下一步:");
        println!("    1. 编辑 Scripts/ 下的脚本");
        println!("    2. 编辑 package.json 添加 files/dependencies");
        println!("    3. moho-mate pkg pack {:?}", out);
        Ok(())
    }

    // ===== 打包 =====

    pub fn pack(&self, dir: &Path) -> Result<()> {
        let pkg_json_path = dir.join("package.json");
        if !pkg_json_path.exists() {
            bail!("目录缺少 package.json: {:?}", dir);
        }

        let pkg = PackageJson::from_file(&pkg_json_path)?;
        let output_name = format!("{}-{}.zip", pkg.name.replace('/', "-"), pkg.version);
        let output_path = std::env::current_dir()?.join(&output_name);

        println!("▶ 打包: {}@{}", pkg.name, pkg.version);

        let file = std::fs::File::create(&output_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 始终包含 package.json
        add_file_to_zip(&mut zip, dir, "package.json", &options)?;

        // 包含 files 中指定的文件/目录
        for pattern in &pkg.files {
            let full_path = dir.join(pattern);
            if full_path.is_dir() {
                add_dir_to_zip(&mut zip, dir, &full_path, &options)?;
            } else if full_path.exists() {
                add_file_to_zip(&mut zip, dir, pattern, &options)?;
            } else {
                eprintln!("  ⚠ 跳过不存在的: {}", pattern);
            }
        }

        zip.finish()?;
        println!("✓ 已打包: {}", output_path.display());
        Ok(())
    }

    // ===== 搜索 =====

    #[cfg(feature = "http-client")]
    pub fn search(&self, keyword: &str) -> Result<Vec<SearchResult>> {
        println!("▶ 搜索: {}", keyword);
        println!("  Registry: {}", self.config.registry);

        let url = format!("{}/-/v1/search?text={}&size=20", self.config.registry, keyword);

        let response = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .get(&url)
            .call()
            .map_err(|e| anyhow::anyhow!("搜索请求失败: {}", e))?;

        let body: serde_json::Value = response.into_json()
            .map_err(|e| anyhow::anyhow!("解析响应失败: {}", e))?;

        let mut results = Vec::new();

        if let Some(objects) = body["objects"].as_array() {
            for obj in objects {
                let pkg = &obj["package"];
                let name = pkg["name"].as_str().unwrap_or("").to_string();
                let version = pkg["version"].as_str().unwrap_or("").to_string();
                let description = pkg["description"].as_str().map(String::from);
                let author = pkg["author"].as_str()
                    .or_else(|| pkg["author"]["name"].as_str())
                    .map(String::from);

                results.push(SearchResult {
                    name,
                    version,
                    description,
                    author,
                });
            }
        }

        Ok(results)
    }

    #[cfg(not(feature = "http-client"))]
    pub fn search(&self, keyword: &str) -> Result<Vec<SearchResult>> {
        println!("▶ 搜索: {}", keyword);
        println!("  Registry: {}", self.config.registry);
        bail!("搜索需要 http-client feature。请使用 moho-mate pkg install ./本地文件.zip");
    }

    // ===== 更新 =====

    pub fn update(&self, package_name: Option<&str>) -> Result<()> {
        match package_name {
            Some(name) => {
                println!("▶ 更新: {}", name);
                let versions = self.find_installed_versions(name)?;
                if versions.is_empty() {
                    bail!("包未安装: {}", name);
                }

                // TODO: 从 registry 获取最新版本并安装
                println!("  当前版本: {}", versions.join(", "));
                println!("  ⚠ 从 registry 更新暂未实现");
            }
            None => {
                println!("▶ 更新所有包");
                let packages = self.list()?;
                if packages.is_empty() {
                    println!("  没有已安装的包");
                    return Ok(());
                }

                for (name, pkg) in &packages {
                    println!("  {}@{}", name, pkg.version);
                }
                println!("  ⚠ 批量更新暂未实现");
            }
        }
        Ok(())
    }

    // ===== 清理 =====

    pub fn prune(&self) -> Result<()> {
        println!("▶ 清理无用包");

        let importmap = ImportMap::load(&self.importmap_path())?;

        // 收集 importmap 中引用的所有包路径
        let mut referenced_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

        for (_, target) in &importmap.imports {
            // 从路径提取 store/pkg/name/version/
            if let Some(stripped) = target.strip_prefix(&format!("{}/", self.packages_dir.display())) {
                // 格式: @scope/name/version/... 或 name/version/...
                let parts: Vec<&str> = stripped.splitn(4, '/').collect();
                if parts.len() >= 2 {
                    let pkg_path = if parts[0].starts_with('@') && parts.len() >= 3 {
                        format!("{}/{}/{}", parts[0], parts[1], parts[2])
                    } else {
                        format!("{}/{}", parts[0], parts[1])
                    };
                    referenced_dirs.insert(self.packages_dir.join(&pkg_path));
                }
            }
        }

        // 遍历 store，删除不在 importmap 中的包
        let mut pruned = 0;
        let mut freed_bytes: u64 = 0;

        if self.packages_dir.exists() {
            self.prune_scan(&self.packages_dir, &referenced_dirs, &mut pruned, &mut freed_bytes)?;
        }

        if pruned == 0 {
            println!("✓ 没有需要清理的包");
        } else {
            println!("✓ 已清理 {} 个包，释放 {} 字节", pruned, freed_bytes);
        }
        Ok(())
    }

    fn prune_scan(&self, dir: &Path, referenced: &std::collections::HashSet<PathBuf>, pruned: &mut u32, freed_bytes: &mut u64) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // 检查是否包含 package.json（是版本目录）
                if path.join("package.json").exists() {
                    // 这是一个版本目录
                    if !referenced.contains(&path) {
                        let size = dir_size(&path);
                        println!("  删除: {} ({:.1} KB)", path.display(), size as f64 / 1024.0);
                        std::fs::remove_dir_all(&path)?;
                        *pruned += 1;
                        *freed_bytes += size;
                    }
                } else {
                    // 递归扫描
                    self.prune_scan(&path, referenced, pruned, freed_bytes)?;
                }
            }
        }
        Ok(())
    }

    // ===== getter =====

    pub fn get_config(&self) -> &PkgConfig { &self.config }
    pub fn get_packages_dir(&self) -> &Path { &self.packages_dir }
    pub fn get_user_scripts_dir(&self) -> &Path { &self.user_scripts_dir }

    pub fn set_registry(&mut self, registry: String) -> Result<()> {
        self.config.registry = registry;
        self.config.save(&self.base_dir)?;
        Ok(())
    }
}

// ============ 辅助函数 ============

fn get_moho_user_scripts_dir() -> Result<PathBuf> {
    // macOS: ~/Library/Preferences/Lost Marble/Moho Pro/14/
    // 注意：dirs::config_dir() 返回 Application Support，不是 Preferences
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow::anyhow!("无法获取 HOME"))?;
    let settings_path = PathBuf::from(home)
        .join("Library/Preferences/Lost Marble/Moho Pro/14/Moho Pro14.user.settings");

    if !settings_path.exists() {
        bail!("Moho 设置文件不存在: {:?}", settings_path);
    }

    let content = std::fs::read_to_string(&settings_path)?;
    for line in content.lines() {
        if line.contains("EditableFilesDir") {
            let path = line.split('"').nth(3)
                .ok_or_else(|| anyhow::anyhow!("无法解析 EditableFilesDir"))?;
            return Ok(PathBuf::from(path).join("Scripts"));
        }
    }

    bail!("未找到 EditableFilesDir 配置")
}

fn extract_archive(archive_path: &Path, dest: &Path) -> Result<()> {
    let ext = archive_path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "zip" => {
            let file = std::fs::File::open(archive_path)?;
            let mut archive = zip::ZipArchive::new(file)?;

            let mut pkg_dir_prefix = String::new();
            for i in 0..archive.len() {
                let file = archive.by_index(i)?;
                if file.name().ends_with("package.json") {
                    pkg_dir_prefix = file.name().strip_suffix("package.json").unwrap_or("").to_string();
                    break;
                }
            }

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let name = file.name().to_string();
                let relative_name = if pkg_dir_prefix.is_empty() {
                    name.as_str()
                } else {
                    name.strip_prefix(&pkg_dir_prefix).unwrap_or(&name)
                };

                if relative_name.is_empty() { continue; }
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
        _ => bail!("不支持的归档格式: {}", ext),
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&src_path)?;

        if metadata.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if metadata.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        } else if metadata.is_symlink() {
            let target = std::fs::read_link(&src_path)?;
            let target_path = if target.is_relative() {
                src_path.parent().unwrap().join(&target)
            } else {
                target
            };
            if target_path.exists() && target_path.is_file() {
                std::fs::copy(&target_path, &dst_path)?;
            }
        }
    }
    Ok(())
}

fn collect_packages(pkg_dir: &Path, packages: &mut Vec<(String, PackageJson)>) -> Result<()> {
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

/// 递归收集目录下的所有文件
fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>, _base: &Path) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files, _base)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
}

/// 添加文件到 zip
fn add_file_to_zip(zip: &mut zip::ZipWriter<std::fs::File>, base: &Path, relative: &str, options: &zip::write::FileOptions) -> Result<()> {
    let full_path = base.join(relative);
    zip.start_file(relative, *options)?;
    let mut f = std::fs::File::open(&full_path)?;
    std::io::copy(&mut f, zip)?;
    Ok(())
}

/// 添加目录到 zip（递归）
fn add_dir_to_zip(zip: &mut zip::ZipWriter<std::fs::File>, base: &Path, dir: &Path, options: &zip::write::FileOptions) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(base)?.to_string_lossy().to_string();

        if path.is_dir() {
            add_dir_to_zip(zip, base, &path, options)?;
        } else {
            add_file_to_zip(zip, base, &relative, options)?;
        }
    }
    Ok(())
}

/// 计算目录大小
fn dir_size(path: &Path) -> u64 {
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                size += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                size += meta.len();
            }
        }
    }
    size
}
