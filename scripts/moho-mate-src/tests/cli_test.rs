//! 命令行解析测试

use clap::Parser;
use moho_mate::*;

// 测试用的 CLI 结构（简化版）
#[derive(Parser, Debug)]
#[command(name = "moho-mate")]
struct TestCli {
    #[command(subcommand)]
    command: Option<TestCommands>,
}

#[derive(clap::Subcommand, Debug)]
enum TestCommands {
    Start {
        project: Option<String>,
        script: Option<String>,
        #[arg(short = 't', long, default_value = "3600")]
        timeout: u32,
    },
    Call {
        code: Option<String>,
        #[arg(short, long)]
        file: Option<String>,
    },
    Quit,
    Status,
}

#[test]
fn test_cli_start_default_timeout() {
    let cli = TestCli::parse_from(["moho-mate", "start"]);
    
    match cli.command {
        Some(TestCommands::Start { timeout, .. }) => {
            assert_eq!(timeout, 3600, "default timeout should be 3600");
        }
        _ => panic!("expected Start command"),
    }
}

#[test]
fn test_cli_start_custom_timeout() {
    let cli = TestCli::parse_from(["moho-mate", "start", "-t", "7200"]);
    
    match cli.command {
        Some(TestCommands::Start { timeout, .. }) => {
            assert_eq!(timeout, 7200, "custom timeout should be 7200");
        }
        _ => panic!("expected Start command"),
    }
}

#[test]
fn test_cli_call_with_code() {
    let cli = TestCli::parse_from(["moho-mate", "call", "print('hello')"]);
    
    match cli.command {
        Some(TestCommands::Call { code, .. }) => {
            assert_eq!(code, Some("print('hello')".to_string()));
        }
        _ => panic!("expected Call command"),
    }
}

#[test]
fn test_cli_call_with_file() {
    let cli = TestCli::parse_from(["moho-mate", "call", "-f", "script.lua"]);
    
    match cli.command {
        Some(TestCommands::Call { file, .. }) => {
            assert_eq!(file, Some("script.lua".to_string()));
        }
        _ => panic!("expected Call command"),
    }
}

#[test]
fn test_cli_quit() {
    let cli = TestCli::parse_from(["moho-mate", "quit"]);
    
    match cli.command {
        Some(TestCommands::Quit) => {}
        _ => panic!("expected Quit command"),
    }
}

#[test]
fn test_cli_status() {
    let cli = TestCli::parse_from(["moho-mate", "status"]);
    
    match cli.command {
        Some(TestCommands::Status) => {}
        _ => panic!("expected Status command"),
    }
}
