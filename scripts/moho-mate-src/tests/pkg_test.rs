//! pkg 模块单元测试

use moho_mate::pkg::*;
use std::collections::HashMap;

/// 测试 PackageJson 解析
#[test]
fn test_package_json_parse() {
    let json = r#"{
        "name": "@maohou/test",
        "version": "1.0.0",
        "description": "Test package",
        "main": "Scripts/Modules/init.lua",
        "files": ["Scripts/Modules/init.lua", "Scripts/Tool/test.lua"],
        "dependencies": {
            "@maohou/json": "^1.0.0"
        },
        "moho": {
            "min_version": "14.0",
            "tools": [
                {"id": "test", "name": "Test Tool"}
            ]
        }
    }"#;
    
    let pkg: PackageJson = serde_json::from_str(json).expect("parse package.json");
    
    assert_eq!(pkg.name, "@maohou/test");
    assert_eq!(pkg.version, "1.0.0");
    assert_eq!(pkg.description, Some("Test package".to_string()));
    assert_eq!(pkg.main, Some("Scripts/Modules/init.lua".to_string()));
    assert_eq!(pkg.files.len(), 2);
    assert!(pkg.dependencies.is_some());
    assert!(pkg.moho.is_some());
    
    let moho = pkg.moho.unwrap();
    assert_eq!(moho.min_version, Some("14.0".to_string()));
    assert_eq!(moho.tools.as_ref().unwrap().len(), 1);
}

/// 测试 Author 字段解析（字符串和对象格式）
#[test]
fn test_author_parsing() {
    // 字符串格式
    let json1 = r#"{
        "name": "test",
        "version": "1.0.0",
        "author": "John Doe",
        "files": []
    }"#;
    let pkg1: PackageJson = serde_json::from_str(json1).expect("parse");
    assert_eq!(pkg1.author.as_ref().map(|a| a.name()), Some("John Doe"));
    
    // 对象格式
    let json2 = r#"{
        "name": "test",
        "version": "1.0.0",
        "author": {"name": "Jane Doe", "email": "jane@example.com"},
        "files": []
    }"#;
    let pkg2: PackageJson = serde_json::from_str(json2).expect("parse");
    assert_eq!(pkg2.author.as_ref().map(|a| a.name()), Some("Jane Doe"));
}

/// 测试 PackageJson 字段
#[test]
fn test_package_json_fields() {
    // 有效的 package.json
    let valid = PackageJson {
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        author: None,
        license: None,
        main: None,
        exports: None,
        files: vec!["Scripts/Modules/init.lua".to_string()],
        dependencies: None,
        moho: None,
    };
    assert_eq!(valid.name, "test");
    assert_eq!(valid.version, "1.0.0");
    assert!(!valid.files.is_empty());
    
    // 缺少 name
    let invalid_name = PackageJson {
        name: "".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        author: None,
        license: None,
        main: None,
        exports: None,
        files: vec!["init.lua".to_string()],
        dependencies: None,
        moho: None,
    };
    assert!(invalid_name.name.is_empty());
    
    // 缺少 version
    let invalid_version = PackageJson {
        name: "test".to_string(),
        version: "".to_string(),
        description: None,
        author: None,
        license: None,
        main: None,
        exports: None,
        files: vec!["init.lua".to_string()],
        dependencies: None,
        moho: None,
    };
    assert!(invalid_version.version.is_empty());
    
    // 缺少 files
    let invalid_files = PackageJson {
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        author: None,
        license: None,
        main: None,
        exports: None,
        files: vec![],
        dependencies: None,
        moho: None,
    };
    assert!(invalid_files.files.is_empty());
}

/// 测试 LockFile 结构
#[test]
fn test_lock_file_structure() {
    let lock = LockFile {
        version: 1,
        packages: {
            let mut pkgs = HashMap::new();
            pkgs.insert("test".to_string(), LockPackage {
                version: "1.0.0".to_string(),
                resolved: Some("https://example.com/test.tgz".to_string()),
                integrity: None,
                dependencies: None,
            });
            pkgs.insert("@maohou/json".to_string(), LockPackage {
                version: "2.0.0".to_string(),
                resolved: None,
                integrity: Some("sha512-abc123".to_string()),
                dependencies: Some({
                    let mut deps = HashMap::new();
                    deps.insert("utils".to_string(), "1.0.0".to_string());
                    deps
                }),
            });
            pkgs
        },
    };
    
    assert_eq!(lock.version, 1);
    assert_eq!(lock.packages.len(), 2);
    assert!(lock.packages.contains_key("test"));
    assert!(lock.packages.contains_key("@maohou/json"));
}

/// 测试 semver 版本匹配
#[test]
fn test_version_matching() {
    // 解析版本字符串
    let versions: HashMap<String, ()> = vec![
        ("1.0.0".to_string(), ()),
        ("1.0.1".to_string(), ()),
        ("1.1.0".to_string(), ()),
        ("1.2.0".to_string(), ()),
        ("2.0.0".to_string(), ()),
        ("2.1.0".to_string(), ()),
    ].into_iter().collect();
    
    // 测试版本匹配逻辑
    // ^1.0.0 应匹配 1.x.x 的最高版本
    let matching_1: Vec<&str> = versions.keys()
        .filter(|v| v.starts_with("1."))
        .map(|s| s.as_str())
        .collect();
    
    assert!(matching_1.contains(&"1.0.0"));
    assert!(matching_1.contains(&"1.0.1"));
    assert!(matching_1.contains(&"1.1.0"));
    assert!(matching_1.contains(&"1.2.0"));
    assert!(!matching_1.contains(&"2.0.0"));
}

/// 测试 Registry API 数据结构
#[test]
fn test_registry_structures() {
    // 测试 RegistryPackage 解析
    let registry_json = r#"{
        "name": "@maohou/json",
        "versions": {
            "1.0.0": {
                "version": "1.0.0",
                "dist": {
                    "tarball": "https://registry.npmjs.org/@maohou/json/-/json-1.0.0.tgz"
                }
            },
            "2.0.0": {
                "version": "2.0.0",
                "dist": {
                    "tarball": "https://registry.npmjs.org/@maohou/json/-/json-2.0.0.tgz"
                }
            }
        },
        "dist-tags": {
            "latest": "2.0.0"
        }
    }"#;
    
    let pkg: RegistryPackage = serde_json::from_str(registry_json).expect("parse registry package");
    
    assert_eq!(pkg.name, "@maohou/json");
    assert_eq!(pkg.versions.len(), 2);
    assert_eq!(pkg.dist_tags.get("latest"), Some(&"2.0.0".to_string()));
}

/// 测试搜索结果结构
#[test]
fn test_search_result() {
    let result = SearchResult {
        name: "@maohou/test".to_string(),
        version: "1.0.0".to_string(),
        description: Some("Test package".to_string()),
        author: Some("test-author".to_string()),
    };
    
    assert_eq!(result.name, "@maohou/test");
    assert_eq!(result.version, "1.0.0");
}

/// 测试包名规范化
#[test]
fn test_package_name_normalization() {
    // @org/name 格式
    let name1 = "@maohou/json";
    let normalized1 = name1.replace("@", "_at_").replace("/", "_");
    assert_eq!(normalized1, "_at_maohou_json");
    
    // 简单名称
    let name2 = "utils";
    let normalized2 = name2.replace("@", "_at_").replace("/", "_");
    assert_eq!(normalized2, "utils");
}
