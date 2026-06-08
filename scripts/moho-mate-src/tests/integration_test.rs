//! 集成测试

mod test_utils;

use test_utils::*;
use std::path::PathBuf;

#[test]
fn test_project_file_format() {
    let temp = TestTempDir::new("project_format_test");
    let project_path = temp.file("integration.moho");
    
    create_test_moho_project(&project_path);
    
    // 验证文件格式
    assert!(project_path.exists());
    assert!(project_path.extension().unwrap() == "moho");
}

#[test]
fn test_lua_script_execution_mock() {
    let temp = TestTempDir::new("lua_execution_test");
    let script_path = temp.file("integration.lua");
    
    create_test_lua_script(&script_path);
    
    // 验证脚本内容
    let content = temp.read_file("integration.lua");
    assert!(content.contains("moho:FileNew()"));
    assert!(content.contains("moho:FileSave()"));
}

#[test]
fn test_config_path_generation() {
    use moho_mate::config::*;
    
    let config_dir = moho_config_dir();
    assert!(config_dir.to_str().unwrap().contains("Moho Pro"));
    
    let scripts = scripts_dir();
    assert!(scripts.to_str().unwrap().contains("moho-mate"));
    
    let ipc_tool = ipc_tool_path();
    assert!(ipc_tool.to_str().unwrap().contains("moho_ipc.lua"));
}

#[test]
fn test_ipc_state_transitions() {
    use moho_mate::ipc_core::*;
    use std::sync::atomic::Ordering;
    
    // 初始状态
    assert!(!RUNNING.load(Ordering::SeqCst));
    
    // 模拟状态变化（仅测试原子变量操作）
    RUNNING.store(true, Ordering::SeqCst);
    assert!(RUNNING.load(Ordering::SeqCst));
    
    // 恢复初始状态
    RUNNING.store(false, Ordering::SeqCst);
    assert!(!RUNNING.load(Ordering::SeqCst));
}

#[test]
fn test_command_queue_operations() {
    use moho_mate::ipc_core::*;
    
    let mut queue = COMMAND_QUEUE.lock().unwrap();
    
    // 添加命令
    queue.push("test_command_1".to_string());
    queue.push("test_command_2".to_string());
    
    assert_eq!(queue.len(), 2);
    
    // 取出命令
    let cmd1 = queue.pop();
    assert_eq!(cmd1, Some("test_command_2".to_string())); // LIFO
    
    let cmd2 = queue.pop();
    assert_eq!(cmd2, Some("test_command_1".to_string()));
    
    // 队列应该为空
    assert!(queue.is_empty());
}

#[test]
fn test_processed_count_operations() {
    use moho_mate::ipc_core::*;
    use std::sync::atomic::Ordering;
    
    // 重置计数
    PROCESSED_COUNT.store(0, Ordering::SeqCst);
    
    // 增加计数
    PROCESSED_COUNT.fetch_add(1, Ordering::SeqCst);
    assert_eq!(PROCESSED_COUNT.load(Ordering::SeqCst), 1);
    
    PROCESSED_COUNT.fetch_add(5, Ordering::SeqCst);
    assert_eq!(PROCESSED_COUNT.load(Ordering::SeqCst), 6);
    
    // 重置
    PROCESSED_COUNT.store(0, Ordering::SeqCst);
    assert_eq!(PROCESSED_COUNT.load(Ordering::SeqCst), 0);
}

#[test]
fn test_encode_status_transitions() {
    use moho_mate::ipc_core::*;
    use std::sync::atomic::Ordering;
    
    // 重置状态
    ENCODE_STATUS.store(0, Ordering::SeqCst);
    ENCODE_PROGRESS.store(0, Ordering::SeqCst);
    
    // 测试状态转换
    // idle (0) -> running (1)
    ENCODE_STATUS.store(1, Ordering::SeqCst);
    assert_eq!(ENCODE_STATUS.load(Ordering::SeqCst), 1);
    
    // 更新进度
    ENCODE_PROGRESS.store(50, Ordering::SeqCst);
    assert_eq!(ENCODE_PROGRESS.load(Ordering::SeqCst), 50);
    
    // running -> success (2)
    ENCODE_STATUS.store(2, Ordering::SeqCst);
    assert_eq!(ENCODE_STATUS.load(Ordering::SeqCst), 2);
    
    // 重置
    ENCODE_STATUS.store(0, Ordering::SeqCst);
    ENCODE_PROGRESS.store(0, Ordering::SeqCst);
}
