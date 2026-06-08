//! 配置模块测试

use moho_mate::config::*;

#[test]
fn test_moho_app_path() {
    assert_eq!(MOHO_APP, "/Applications/Moho.app");
}

#[test]
fn test_socket_path() {
    assert_eq!(SOCKET_PATH, "/tmp/moho_ipc.sock");
}

#[test]
fn test_ipc_cmd_dir() {
    assert_eq!(IPC_CMD_DIR, "/tmp/moho_ipc_cmds");
}

#[test]
fn test_moho_config_dir() {
    let path = moho_config_dir();
    assert!(path.to_str().unwrap().contains("Lost Marble"));
    assert!(path.to_str().unwrap().contains("Moho Pro"));
}

#[test]
fn test_scripts_dir() {
    let path = scripts_dir();
    assert!(path.to_str().unwrap().contains(".openclaw"));
    assert!(path.to_str().unwrap().contains("moho-mate"));
    assert!(path.to_str().unwrap().contains("scripts"));
}

#[test]
fn test_ipc_tool_path() {
    let path = ipc_tool_path();
    assert!(path.to_str().unwrap().contains("moho_ipc.lua"));
}

#[test]
fn test_empty_config_template() {
    let path = empty_config_template();
    assert!(path.to_str().unwrap().contains("empty_config"));
}
