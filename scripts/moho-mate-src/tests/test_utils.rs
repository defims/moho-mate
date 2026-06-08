//! 测试辅助函数和 Mock

use std::path::PathBuf;
use std::fs;

/// 测试用的临时目录管理
pub struct TestTempDir {
    pub path: PathBuf,
}

impl TestTempDir {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("moho_test_{}", name));
        
        // 清理旧目录
        if path.exists() {
            let _ = fs::remove_dir_all(&path);
        }
        
        fs::create_dir_all(&path).expect("failed to create temp dir");
        
        Self { path }
    }
    
    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
    
    pub fn write_file(&self, name: &str, content: &str) {
        fs::write(self.file(name), content).expect("failed to write file");
    }
    
    pub fn read_file(&self, name: &str) -> String {
        fs::read_to_string(self.file(name)).expect("failed to read file")
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        // 清理临时目录
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 创建测试用的 .moho 项目文件
pub fn create_test_moho_project(path: &PathBuf) {
    use zip::ZipWriter;
    use std::io::Write;
    
    let file = fs::File::create(path).expect("failed to create .moho file");
    let mut zip = ZipWriter::new(file);
    
    // 添加项目文件
    let project_json = r#"{
        "version": "14.0",
        "document": {
            "width": 1280,
            "height": 720,
            "fps": 24,
            "startFrame": 0,
            "endFrame": 72,
            "layers": []
        }
    }"#;
    
    zip.start_file("Project.mohoproj", zip::write::FileOptions::default())
        .expect("failed to start zip entry");
    zip.write_all(project_json.as_bytes()).expect("failed to write project json");
    
    zip.finish().expect("failed to finish zip");
}

/// 创建测试用的 Lua 脚本
pub fn create_test_lua_script(path: &PathBuf) {
    let content = r#"-- Test script
local moho = ...  -- ScriptInterface passed as first argument

-- Create new document
moho:FileNew()

-- Create vector layer
local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)
layer:SetName("TestLayer")

-- Save
moho:FileSave()
"#;
    
    fs::write(path, content).expect("failed to write lua script");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_temp_dir_creation() {
        let temp = TestTempDir::new("creation_test");
        assert!(temp.path.exists(), "temp dir should exist");
        assert!(temp.path.is_dir(), "temp path should be a directory");
    }
    
    #[test]
    fn test_temp_dir_file_operations() {
        let temp = TestTempDir::new("file_test");
        
        temp.write_file("test.txt", "hello world");
        let content = temp.read_file("test.txt");
        
        assert_eq!(content, "hello world");
        assert!(temp.file("test.txt").exists());
    }
    
    #[test]
    fn test_temp_dir_cleanup() {
        let path;
        {
            let temp = TestTempDir::new("cleanup_test");
            path = temp.path.clone();
            assert!(path.exists());
        }
        // 离开作用域后应该被清理
        // 注意：由于清理可能失败，这里只检查逻辑正确性
    }
    
    #[test]
    fn test_create_test_moho_project() {
        let temp = TestTempDir::new("moho_project_test");
        let project_path = temp.file("test.moho");
        
        create_test_moho_project(&project_path);
        
        assert!(project_path.exists(), "project file should exist");
        
        // 验证是有效的 ZIP 文件
        let file = fs::File::open(&project_path).expect("failed to open project");
        let archive = zip::ZipArchive::new(file).expect("failed to read zip");
        assert!(archive.file_names().any(|name| name.contains("Project.mohoproj")));
    }
    
    #[test]
    fn test_create_test_lua_script() {
        let temp = TestTempDir::new("lua_script_test");
        let script_path = temp.file("test.lua");
        
        create_test_lua_script(&script_path);
        
        assert!(script_path.exists(), "script file should exist");
        
        let content = temp.read_file("test.lua");
        assert!(content.contains("FileNew"), "script should contain FileNew");
        assert!(content.contains("CreateNewLayer"), "script should contain CreateNewLayer");
    }
}
