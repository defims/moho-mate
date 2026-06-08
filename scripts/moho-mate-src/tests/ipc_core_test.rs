//! IPC 核心逻辑测试

use moho_mate::ipc_core::*;
use std::sync::atomic::Ordering;

#[test]
fn test_running_flag_default() {
    // RUNNING 标志默认应该为 false
    let running = RUNNING.load(Ordering::SeqCst);
    assert!(!running, "RUNNING flag should be false by default");
}

#[test]
fn test_command_queue_default() {
    // COMMAND_QUEUE 默认应该为空
    let queue = COMMAND_QUEUE.lock().unwrap();
    assert!(queue.is_empty(), "COMMAND_QUEUE should be empty by default");
}

#[test]
fn test_processed_count_default() {
    // PROCESSED_COUNT 默认应该为 0
    let count = PROCESSED_COUNT.load(Ordering::SeqCst);
    assert_eq!(count, 0, "PROCESSED_COUNT should be 0 by default");
}

#[test]
fn test_get_status() {
    let (running, path, pending, processed) = get_status();
    
    // 初始状态
    assert!(!running, "IPC should not be running initially");
    assert!(!path.is_empty(), "path should not be empty");
    assert_eq!(pending, 0, "pending commands should be 0");
    assert_eq!(processed, 0, "processed commands should be 0");
}

#[test]
fn test_encode_status_default() {
    let (status, status_text, progress, output_path) = encode_status();
    
    // 初始状态应该是 idle (0)
    assert_eq!(status, 0, "encode status should be idle (0)");
    assert_eq!(status_text, "idle", "status_text should be 'idle'");
    assert_eq!(progress, 0, "progress should be 0");
    assert!(output_path.is_empty(), "output_path should be empty");
}

#[test]
fn test_encode_cancel() {
    // 取消编码：如果没有编码任务，返回 false
    let result = encode_cancel();
    // 如果编码状态不是 running(1)，返回 false
    // 这是预期行为：无法取消一个不存在的编码任务
    // 初始状态下编码状态应该是 idle(0)
    assert!(!result, "encode_cancel should return false when no encoding is running");
}
