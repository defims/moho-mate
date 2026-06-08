//! IPC 核心实现
//!
//! Socket 服务、命令处理、FFmpeg 编码、播放控制
//!
//! ⚠️ 所有 Lua 命令都在 Main Thread 执行
//!   - macOS: 通过 CFRunLoop
//!   - Windows: 通过 WSAAsyncSelect + 隐藏窗口消息循环
//! 这样可以安全调用 Moho 的 GUI API（FileNew, FileSaveAs, FileRender 等）

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;
use std::ptr;
use std::os::raw::c_int;

// 导出公共状态变量供测试使用
pub use std::sync::atomic::{AtomicBool as PubAtomicBool, AtomicI32 as PubAtomicI32, AtomicUsize as PubAtomicUsize};

#[cfg(unix)]
use std::os::unix::net::{UnixStream, UnixListener};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use tracing::{info, warn, error};

use crate::lua_ffi::*;

// ========== Windows Named Pipe FFI ==========
#[cfg(target_os = "windows")]
mod win_pipe {
    use std::ffi::c_void;
    use std::os::raw::c_int;
    
    // Windows 句柄类型
    pub type HANDLE = *mut c_void;
    pub type LPSECURITY_ATTRIBUTES = *mut c_void;
    pub type LPWSTR = *mut u16;
    pub type LPDWORD = *mut u32;
    pub type LPOVERLAPPED = *mut c_void;
    
    // 命名管道常量
    pub const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
    pub const PIPE_TYPE_MESSAGE: u32 = 0x00000004;
    pub const PIPE_READMODE_MESSAGE: u32 = 0x00000002;
    pub const PIPE_WAIT: u32 = 0x00000000;
    pub const PIPE_UNLIMITED_INSTANCES: u32 = 255;
    pub const NMPWAIT_USE_DEFAULT_WAIT: u32 = 0x00000000;
    
    // 文件访问权限
    pub const GENERIC_READ: u32 = 0x80000000;
    pub const GENERIC_WRITE: u32 = 0x40000000;
    pub const OPEN_EXISTING: u32 = 3;
    
    extern "system" {
        // 创建命名管道
        pub fn CreateNamedPipeW(
            lpName: LPWSTR,
            dwOpenMode: u32,
            dwPipeMode: u32,
            nMaxInstances: u32,
            nOutBufferSize: u32,
            nInBufferSize: u32,
            nDefaultTimeOut: u32,
            lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
        ) -> HANDLE;
        
        // 连接命名管道
        pub fn ConnectNamedPipe(
            hNamedPipe: HANDLE,
            lpOverlapped: LPOVERLAPPED,
        ) -> i32;
        
        // 断开命名管道
        pub fn DisconnectNamedPipe(hNamedPipe: HANDLE) -> i32;
        
        // 读取
        pub fn ReadFile(
            hFile: HANDLE,
            lpBuffer: *mut c_void,
            nNumberOfBytesToRead: u32,
            lpNumberOfBytesRead: LPDWORD,
            lpOverlapped: LPOVERLAPPED,
        ) -> i32;
        
        // 写入
        pub fn WriteFile(
            hFile: HANDLE,
            lpBuffer: *const c_void,
            nNumberOfBytesToWrite: u32,
            lpNumberOfBytesWritten: LPDWORD,
            lpOverlapped: LPOVERLAPPED,
        ) -> i32;
        
        // 关闭句柄
        pub fn CloseHandle(hObject: HANDLE) -> i32;
        
        // 获取客户端 PID
        pub fn GetNamedPipeClientProcessId(
            Pipe: HANDLE,
            ClientProcessId: LPDWORD,
        ) -> i32;
        
        // 打开进程
        pub fn OpenProcess(
            dwDesiredAccess: u32,
            bInheritHandle: i32,
            dwProcessId: u32,
        ) -> HANDLE;
        
        // 获取进程映像路径
        pub fn QueryFullProcessImageNameW(
            hProcess: HANDLE,
            dwFlags: u32,
            lpExeName: LPWSTR,
            lpdwSize: LPDWORD,
        ) -> i32;
        
        // 刷新缓冲区
        pub fn FlushFileBuffers(hFile: HANDLE) -> i32;
    }
    
    // 进程访问权限
    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
}

// ========== Windows Module FFI (GetModuleHandleExW + GetModuleFileNameW) ==========
#[cfg(target_os = "windows")]
mod win_module {
    use std::ffi::c_void;
    use std::os::raw::c_int;
    use super::win_pipe::HANDLE;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    
    // GetModuleHandleEx 标志
    pub const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x00000004;
    pub const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 0x00000002;
    
    extern "system" {
        // 根据地址获取模块句柄
        pub fn GetModuleHandleExW(
            dwFlags: u32,
            lpModuleName: *const u16,
            phModule: *mut HANDLE,
        ) -> i32;
        
        // 获取模块文件名
        pub fn GetModuleFileNameW(
            hModule: HANDLE,
            lpFilename: *mut u16,
            nSize: u32,
        ) -> u32;
    }
    
    /// 获取当前代码所在的模块路径（类似 macOS 的 dladdr）
    /// 即使被其他进程 LoadLibrary 加载，也能返回 moho-mate 自己的路径
    pub fn get_module_path() -> Option<String> {
        unsafe {
            let mut h_module: HANDLE = std::ptr::null_mut();
            
            // 使用当前函数的地址获取模块句柄
            let ret = GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                get_module_path as *const u16,  // 函数地址
                &mut h_module,
            );
            
            if ret == 0 || h_module.is_null() {
                return None;
            }
            
            // 获取模块路径
            let mut buffer = [0u16; 1024];
            let len = GetModuleFileNameW(h_module, buffer.as_mut_ptr(), buffer.len() as u32);
            
            if len == 0 {
                return None;
            }
            
            // 转换为 Rust String
            Some(OsString::from_wide(&buffer[..len as usize]).to_string_lossy().to_string())
        }
    }
}

// ========== Windows Window FFI (WSAAsyncSelect + 隐藏窗口) ==========
#[cfg(target_os = "windows")]
mod win_window {
    use std::ffi::c_void;
    use std::os::raw::c_int;
    use super::win_pipe::HANDLE;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    
    // Windows 消息常量
    pub const WM_USER: u32 = 0x0400;
    pub const WM_SOCKET: u32 = WM_USER + 100;
    pub const WM_PIPE: u32 = WM_USER + 101;
    
    // WSA 事件常量
    pub const FD_READ: i32 = 0x01;
    pub const FD_WRITE: i32 = 0x02;
    pub const FD_ACCEPT: i32 = 0x08;
    pub const FD_CONNECT: i32 = 0x10;
    pub const FD_CLOSE: i32 = 0x20;
    
    // 窗口样式
    pub const WS_OVERLAPPED: u32 = 0x00000000;
    pub const HWND_MESSAGE: HWND = std::ptr::null_mut();
    
    // 窗口消息类型
    pub type HWND = *mut c_void;
    pub type HMENU = *mut c_void;
    pub type HINSTANCE = *mut c_void;
    pub type WPARAM = usize;
    pub type LPARAM = isize;
    pub type LRESULT = isize;
    pub type WNDPROC = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;
    
    #[repr(C)]
    pub struct WNDCLASSW {
        pub style: u32,
        pub lpfnWndProc: WNDPROC,
        pub cbClsExtra: i32,
        pub cbWndExtra: i32,
        pub hInstance: HINSTANCE,
        pub hIcon: HANDLE,
        pub hCursor: HANDLE,
        pub hbrBackground: HANDLE,
        pub lpszMenuName: *const u16,
        pub lpszClassName: *const u16,
    }
    
    extern "system" {
        // 注册窗口类
        pub fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> u16;
        
        // 创建窗口
        pub fn CreateWindowExW(
            dwExStyle: u32,
            lpClassName: *const u16,
            lpWindowName: *const u16,
            dwStyle: u32,
            x: i32,
            y: i32,
            nWidth: i32,
            nHeight: i32,
            hWndParent: HWND,
            hMenu: HMENU,
            hInstance: HINSTANCE,
            lpParam: *const c_void,
        ) -> HWND;
        
        // 默认窗口过程
        pub fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        
        // 销毁窗口
        pub fn DestroyWindow(hWnd: HWND) -> i32;
        
        // 获取当前线程 ID
        pub fn GetCurrentThreadId() -> u32;
        
        // 获取模块句柄
        pub fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
    }
    
    // Winsock FFI
    pub type SOCKET = usize;
    
    extern "system" {
        // WSAAsyncSelect - 将 socket 事件绑定到窗口消息
        pub fn WSAAsyncSelect(
            s: SOCKET,
            hWnd: HWND,
            wMsg: u32,
            lEvent: i32,
        ) -> c_int;
        
        // 获取 WSA 错误
        pub fn WSAGetLastError() -> c_int;
    }
}

// ========== CFRunLoop FFI (macOS) ==========
#[cfg(target_os = "macos")]
mod cf {
    use std::ffi::c_void;
    use std::os::raw::c_int;
    use std::ptr;
    
    // CF 类型
    pub type CFAllocatorRef = *mut c_void;
    pub type CFRunLoopRef = *mut c_void;
    pub type CFRunLoopSourceRef = *mut c_void;
    pub type CFRunLoopTimerRef = *mut c_void;
    pub type CFSocketRef = *mut c_void;
    pub type CFDataRef = *mut c_void;
    
    // CFSocket 回调类型
    pub type CFSocketCallBack = extern "C" fn(
        CFSocketRef,
        CFSocketCallBackType,
        CFDataRef,
        *const c_void,
        *mut c_void,
    );
    
    // CFSocket 回调类型常量
    pub const kCFSocketNoCallBack: i32 = 0;
    pub const kCFSocketReadCallBack: i32 = 1;
    pub const kCFSocketAcceptCallBack: i32 = 2;
    pub const kCFSocketDataCallBack: i32 = 3;
    pub const kCFSocketConnectCallBack: i32 = 4;
    
    pub type CFSocketCallBackType = i32;
    
    extern "C" {
        // CFAllocator
        pub static kCFAllocatorDefault: CFAllocatorRef;
        
        // CFRunLoop
        pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        pub fn CFRunLoopGetMain() -> CFRunLoopRef;
        pub fn CFRunLoopRun();
        pub fn CFRunLoopStop(rl: CFRunLoopRef);
        pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: *const c_void);
        pub fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: *const c_void);
        pub fn CFRunLoopAddTimer(rl: CFRunLoopRef, timer: CFRunLoopTimerRef, mode: *const c_void);
        pub fn CFRunLoopRemoveTimer(rl: CFRunLoopRef, timer: CFRunLoopTimerRef, mode: *const c_void);
        
        // CFSocket
        pub fn CFSocketCreateWithNative(
            allocator: CFAllocatorRef,
            sock: c_int,
            callBackTypes: i32,
            callout: CFSocketCallBack,
            context: *mut CFSocketContext,
        ) -> CFSocketRef;
        pub fn CFSocketCreateRunLoopSource(
            allocator: CFAllocatorRef,
            s: CFSocketRef,
            order: i32,
        ) -> CFRunLoopSourceRef;
        pub fn CFSocketGetNative(s: CFSocketRef) -> c_int;
        pub fn CFSocketInvalidate(s: CFSocketRef);
        pub fn CFSocketEnableCallBacks(s: CFSocketRef, callBackTypes: i32);
        
        // CFRelease
        pub fn CFRelease(cf: *const c_void);
        
        // CFData
        pub fn CFDataGetBytes(theData: CFDataRef, range: CFRange, buffer: *mut u8);
        pub fn CFDataGetLength(theData: CFDataRef) -> i64;
    }
    
    // CFRange
    #[repr(C)]
    pub struct CFRange {
        pub location: i64,
        pub length: i64,
    }
    
    // CFSocketContext
    #[repr(C)]
    pub struct CFSocketContext {
        pub version: i32,
        pub info: *mut c_void,
        pub retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
        pub release: Option<extern "C" fn(*const c_void)>,
        pub copyDescription: Option<extern "C" fn(*const c_void) -> *mut c_void>,
    }
    
    /// 获取 kCFRunLoopDefaultMode
    pub fn get_default_mode() -> *const c_void {
        // kCFRunLoopDefaultMode 是一个 CFStringRef
        // 我们需要通过 dlsym 获取
        unsafe {
            let handle = libc::dlopen(ptr::null(), libc::RTLD_NOW);
            if handle.is_null() {
                return ptr::null();
            }
            let sym = libc::dlsym(
                handle,
                std::ffi::CString::new("kCFRunLoopDefaultMode").unwrap().as_ptr(),
            );
            libc::dlclose(handle);
            if !sym.is_null() {
                *(sym as *const *const c_void)
            } else {
                ptr::null()
            }
        }
    }
}

// ========== 配置 ==========

#[cfg(unix)]
const SOCKET_PATH: &str = "/tmp/moho_ipc.sock";
#[cfg(target_os = "windows")]
const PIPE_NAME: &str = "\\\\.\\pipe\\moho_ipc";

const LOG_FILE: &str = "/tmp/moho_ipc.log";

#[cfg(target_os = "windows")]
fn get_log_file() -> String {
    std::env::temp_dir().join("moho_ipc.log").to_string_lossy().to_string()
}

#[cfg(not(target_os = "windows"))]
fn get_log_file() -> &'static str {
    LOG_FILE
}

// ========== 调用者验证 (macOS) ==========
#[cfg(target_os = "macos")]
mod peercred {
    use std::os::raw::c_int;
    use std::ffi::c_void;
    
    // macOS 的 LOCAL_PEERPID 定义
    pub const LOCAL_PEERPID: c_int = 0x002;
    
    // xucred 结构（简化版）
    #[repr(C)]
    pub struct xucred {
        pub cr_version: u_int,
        pub cr_uid: uid_t,
        pub cr_ngroups: libc::c_short,
        pub cr_groups: [gid_t; 16],
        pub _cr_unused: *mut c_void,
    }
    
    pub type u_int = c_int;
    pub type uid_t = u32;
    pub type gid_t = u32;
    
    extern "C" {
        pub fn getsockopt(
            socket: c_int,
            level: c_int,
            optname: c_int,
            optval: *mut c_void,
            optlen: *mut u32,
        ) -> c_int;
        
        // macOS 获取进程路径
        pub fn proc_pidpath(pid: c_int, buffer: *mut i8, buffersize: u32) -> c_int;
        
        // dladdr 获取共享库/可执行文件路径
        pub fn dladdr(addr: *const c_void, info: *mut Dl_info) -> c_int;
    }
    
    // Dl_info 结构（用于 dladdr）
    #[repr(C)]
    pub struct Dl_info {
        pub dli_fname: *const i8,   // 文件路径
        pub dli_fbase: *mut c_void, // 基址
        pub dli_sname: *const i8,   // 符号名
        pub dli_saddr: *mut c_void, // 符号地址
    }
    
    /// 获取 socket 对端的 PID
    pub fn get_peer_pid(fd: c_int) -> Option<i32> {
        unsafe {
            let mut pid: i32 = 0;
            let mut len = std::mem::size_of::<i32>() as u32;
            
            let ret = getsockopt(
                fd,
                libc::SOL_LOCAL,
                LOCAL_PEERPID,
                &mut pid as *mut i32 as *mut c_void,
                &mut len,
            );
            
            if ret == 0 && len == std::mem::size_of::<i32>() as u32 {
                Some(pid)
            } else {
                None
            }
        }
    }
    
    /// 获取 PID 对应的可执行文件路径
    pub fn get_pid_path(pid: i32) -> Option<String> {
        unsafe {
            let mut buffer = [0i8; 1024];
            let ret = proc_pidpath(pid, buffer.as_mut_ptr(), buffer.len() as u32);
            
            if ret > 0 {
                // 找到 null 终止符位置
                let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                Some(String::from_utf8_lossy(&buffer[..end].iter().map(|&c| c as u8).collect::<Vec<_>>()).to_string())
            } else {
                None
            }
        }
    }
    
    /// 获取当前代码所在的文件路径（通过 dladdr）
    /// 即使被其他进程 dlopen，也能返回原始可执行文件路径
    pub fn get_module_path() -> Option<String> {
        unsafe {
            let mut info: Dl_info = std::mem::zeroed();
            // 使用当前函数的地址来获取所在文件
            let ret = dladdr(get_module_path as *const c_void, &mut info);
            
            if ret != 0 && !info.dli_fname.is_null() {
                let cstr = std::ffi::CStr::from_ptr(info.dli_fname);
                Some(cstr.to_string_lossy().to_string())
            } else {
                None
            }
        }
    }
}

/// 验证调用者是否是启动 IPC 的 moho-mate
fn verify_caller(fd: c_int) -> bool {
    #[cfg(target_os = "macos")]
    {
        // 获取启动者路径
        let owner_path = match IPC_OWNER_PATH.lock() {
            Ok(owner) => owner.clone(),
            Err(_) => {
                log_msg("✗ 无法获取启动者路径");
                return false;
            }
        };
        
        // 未注册则拒绝
        if owner_path.is_empty() {
            log_msg("✗ 启动者路径未设置，拒绝连接");
            return false;
        }
        
        // 获取客户端 PID
        let peer_pid = match peercred::get_peer_pid(fd) {
            Some(pid) => pid,
            None => {
                log_msg("✗ 无法获取调用者 PID");
                return false;
            }
        };
        
        log_msg(&format!("调用者 PID: {}", peer_pid));
        
        // 获取客户端可执行路径
        let peer_path = match peercred::get_pid_path(peer_pid) {
            Some(path) => path,
            None => {
                log_msg("✗ 无法获取调用者路径");
                return false;
            }
        };
        
        log_msg(&format!("调用者路径: {}", peer_path));
        log_msg(&format!("模块路径: {}", owner_path));
        
        // 比较路径是否相同
        if peer_path == owner_path {
            log_msg("✓ 调用者验证通过");
            true
        } else {
            log_msg(&format!("✗ 拒绝连接: {} != {}", peer_path, owner_path));
            false
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // Windows: 使用命名管道句柄验证
        verify_caller_win(fd as win_pipe::HANDLE)
    }
    
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        // 其他平台暂不验证
        true
    }
}

// ========== Windows 验证调用者 ==========
#[cfg(target_os = "windows")]
fn verify_caller_win(pipe_handle: win_pipe::HANDLE) -> bool {
    use win_pipe::*;
    use win_module::get_module_path;
    
    unsafe {
        // 1. 获取客户端 PID
        let mut client_pid: u32 = 0;
        let ret = GetNamedPipeClientProcessId(pipe_handle, &mut client_pid);
        
        if ret == 0 {
            log_msg("✗ 无法获取调用者 PID");
            return false;
        }
        
        log_msg(&format!("调用者 PID: {}", client_pid));
        
        // 2. 打开进程
        let h_process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, client_pid);
        
        if h_process.is_null() {
            log_msg("✗ 无法打开进程");
            return false;
        }
        
        // 3. 获取进程路径
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let ret = QueryFullProcessImageNameW(h_process, 0, buffer.as_mut_ptr(), &mut size);
        
        CloseHandle(h_process);
        
        if ret == 0 {
            log_msg("✗ 无法获取调用者路径");
            return false;
        }
        
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        let peer_path = OsString::from_wide(&buffer[..size as usize]).to_string_lossy().to_string();
        
        log_msg(&format!("调用者路径: {}", peer_path));
        
        // 4. 获取自己（moho-mate）的路径
        let owner_path = match get_module_path() {
            Some(p) => p,
            None => {
                log_msg("✗ 无法获取模块路径");
                return false;
            }
        };
        
        log_msg(&format!("模块路径: {}", owner_path));
        
        // 5. 比较路径
        if peer_path == owner_path {
            log_msg("✓ 调用者验证通过");
            true
        } else {
            log_msg(&format!("✗ 拒绝连接: {} != {}", peer_path, owner_path));
            false
        }
    }
}

// ========== 全局状态 ==========

pub static RUNNING: AtomicBool = AtomicBool::new(false);
pub static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static ERROR_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static PROCESSED_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static COMMAND_QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());

// 启动 IPC 的 moho-mate 可执行路径（用于调用者验证）
static IPC_OWNER_PATH: Mutex<String> = Mutex::new(String::new());

// 编码状态
pub static ENCODE_STATUS: AtomicI32 = AtomicI32::new(0); // 0=idle, 1=running, 2=success, 3=error
pub static ENCODE_PROGRESS: AtomicI32 = AtomicI32::new(0); // 0-100 (百分比 * 100)
pub static ENCODE_ERROR: Mutex<String> = Mutex::new(String::new()); // 错误消息

// Socket 和线程句柄
#[cfg(unix)]
static SOCKET_LISTENER: Mutex<Option<UnixListener>> = Mutex::new(None);
static SOCKET_THREAD: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

// Windows: 命名管道句柄和隐藏窗口
#[cfg(target_os = "windows")]
static mut G_PIPE_HANDLE: Option<win_pipe::HANDLE> = None;
#[cfg(target_os = "windows")]
static mut G_CLIENT_PIPE: Option<win_pipe::HANDLE> = None;
#[cfg(target_os = "windows")]
static mut G_HIDDEN_WINDOW: Option<win_window::HWND> = None;

// Lua state（仅在主线程使用）
static mut LUA_STATE: Option<*mut std::ffi::c_void> = None;

// ========== 日志 ==========

fn log_msg(msg: &str) {
    println!("{}", msg);

    let log_path = get_log_file();
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = writeln!(f, "{}", msg);
    }
}

// ========== IPC 服务 ==========

/// 启动 IPC 服务
/// 自动获取当前模块所在的可执行文件路径作为启动者路径
pub fn ipc_start(L: lua_State, _owner_path: Option<String>) -> (bool, String) {
    log_msg("=== IPC start ===");

    // 保存 Lua state（仅在主线程）
    unsafe {
        LUA_STATE = Some(L);
    }

    // 通过 dladdr/get_module_path 获取当前模块所在的可执行文件路径
    #[cfg(target_os = "macos")]
    {
        if let Some(module_path) = peercred::get_module_path() {
            if let Ok(mut owner) = IPC_OWNER_PATH.lock() {
                *owner = module_path.clone();
                log_msg(&format!("IPC 模块路径 (dladdr): {}", module_path));
            }
        } else {
            log_msg("⚠ 无法获取模块路径");
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        if let Some(module_path) = win_module::get_module_path() {
            if let Ok(mut owner) = IPC_OWNER_PATH.lock() {
                *owner = module_path.clone();
                log_msg(&format!("IPC 模块路径 (GetModuleHandleExW): {}", module_path));
            }
        } else {
            log_msg("⚠ 无法获取模块路径");
        }
    }

    if RUNNING.load(Ordering::SeqCst) {
        #[cfg(unix)]
        return (true, SOCKET_PATH.to_string());
        #[cfg(target_os = "windows")]
        return (true, PIPE_NAME.to_string());
    }

    #[cfg(unix)]
    {
        // 删除旧 socket
        let _ = fs::remove_file(SOCKET_PATH);

        // 创建 socket
        let listener = match UnixListener::bind(SOCKET_PATH) {
            Ok(l) => l,
            Err(e) => {
                log_msg(&format!("✗ bind() failed: {}", e));
                return (false, format!("bind() failed: {}", e));
            }
        };

        // 设置权限
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o600));
        }

        log_msg(&format!("✓ IPC 服务启动: {}", SOCKET_PATH));

        RUNNING.store(true, Ordering::SeqCst);

        // 获取原生 socket fd
        let listener_fd = unsafe { libc::dup(listener.as_raw_fd()) };

        // 存储 listener
        if let Ok(mut l) = SOCKET_LISTENER.lock() {
            *l = Some(listener);
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: 使用 CFRunLoop（在 Main Thread 执行回调）
            setup_cfrunloop_socket(listener_fd);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: 使用命名管道 + 隐藏窗口 + WSAAsyncSelect
        setup_win_ipc();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // 其他 Unix 平台: 使用线程
        let handle = thread::spawn(move || {
            listen_loop();
        });

        if let Ok(mut th) = SOCKET_THREAD.lock() {
            *th = Some(handle);
        }
    }

    #[cfg(unix)]
    return (true, SOCKET_PATH.to_string());
    #[cfg(target_os = "windows")]
    return (true, PIPE_NAME.to_string());
}

// ========== CFRunLoop Socket (macOS) ==========

#[cfg(target_os = "macos")]
static mut G_LISTEN_SOCKET: Option<cf::CFSocketRef> = None;
#[cfg(target_os = "macos")]
static mut G_LISTEN_SOURCE: Option<cf::CFRunLoopSourceRef> = None;
#[cfg(target_os = "macos")]
static mut G_CLIENT_SOCKET: Option<cf::CFSocketRef> = None;
#[cfg(target_os = "macos")]
static mut G_CLIENT_SOURCE: Option<cf::CFRunLoopSourceRef> = None;

/// 设置 CFRunLoop socket（macOS）
#[cfg(target_os = "macos")]
fn setup_cfrunloop_socket(listener_fd: c_int) {
    use cf::*;
    
    log_msg("设置 CFRunLoop socket...");
    
    unsafe {
        // 创建 CFSocketContext
        let mut context = CFSocketContext {
            version: 0,
            info: ptr::null_mut(),
            retain: None,
            release: None,
            copyDescription: None,
        };
        
        // 创建监听 socket
        let listen_socket = CFSocketCreateWithNative(
            kCFAllocatorDefault,
            listener_fd,
            kCFSocketAcceptCallBack,
            listen_callback,
            &mut context,
        );
        
        if listen_socket.is_null() {
            log_msg("✗ 创建监听 CFSocket 失败");
            return;
        }
        
        // 创建 RunLoop Source
        let listen_source = CFSocketCreateRunLoopSource(
            kCFAllocatorDefault,
            listen_socket,
            0,
        );
        
        if listen_source.is_null() {
            log_msg("✗ 创建监听 RunLoop Source 失败");
            CFRelease(listen_socket as *const std::ffi::c_void);
            return;
        }
        
        // 获取当前 RunLoop（应该是 Main Thread 的）
        let runloop = CFRunLoopGetCurrent();
        let main_runloop = CFRunLoopGetMain();
        
        log_msg(&format!("RunLoop: {:?} (Main: {:?})", runloop, main_runloop));
        
        // 添加到 RunLoop
        let mode = get_default_mode();
        CFRunLoopAddSource(runloop, listen_source, mode);
        
        log_msg("✓ 监听 socket 已添加到 RunLoop");
        
        // 保存全局引用
        G_LISTEN_SOCKET = Some(listen_socket);
        G_LISTEN_SOURCE = Some(listen_source);
    }
}

/// 监听 socket 回调（接受连接）
#[cfg(target_os = "macos")]
extern "C" fn listen_callback(
    s: cf::CFSocketRef,
    callback_type: cf::CFSocketCallBackType,
    _addr: cf::CFDataRef,
    data: *const std::ffi::c_void,
    _info: *mut std::ffi::c_void,
) {
    use cf::*;
    
    if callback_type != kCFSocketAcceptCallBack {
        return;
    }
    
    // data 指向客户端 fd
    let client_fd = unsafe { *(data as *const c_int) };
    log_msg(&format!("新连接: fd={}", client_fd));
    
    // 验证调用者（必须是 moho-mate 本身）
    if !verify_caller(client_fd) {
        log_msg("✗ 拒绝连接：调用者验证失败");
        unsafe { libc::close(client_fd); }
        return;
    }
    
    // 关闭旧连接
    cleanup_client_socket();
    
    // 设置非阻塞
    unsafe {
        libc::fcntl(client_fd, libc::F_SETFL, libc::O_NONBLOCK);
    }
    
    unsafe {
        // 创建客户端 CFSocket
        let mut context = CFSocketContext {
            version: 0,
            info: ptr::null_mut(),
            retain: None,
            release: None,
            copyDescription: None,
        };
        
        let client_socket = CFSocketCreateWithNative(
            kCFAllocatorDefault,
            client_fd,
            kCFSocketReadCallBack,
            client_callback,
            &mut context,
        );
        
        if client_socket.is_null() {
            log_msg("✗ 创建客户端 CFSocket 失败");
            libc::close(client_fd);
            return;
        }
        
        // 创建 RunLoop Source
        let client_source = CFSocketCreateRunLoopSource(
            kCFAllocatorDefault,
            client_socket,
            0,
        );
        
        if client_source.is_null() {
            log_msg("✗ 创建客户端 RunLoop Source 失败");
            CFRelease(client_socket as *const std::ffi::c_void);
            return;
        }
        
        // 添加到 RunLoop
        let runloop = CFRunLoopGetCurrent();
        let mode = get_default_mode();
        CFRunLoopAddSource(runloop, client_source, mode);
        
        log_msg("✓ 客户端已添加到 RunLoop");
        
        // 保存全局引用
        G_CLIENT_SOCKET = Some(client_socket);
        G_CLIENT_SOURCE = Some(client_source);
    }
}


#[cfg(target_os = "macos")]
extern "C" fn client_callback(
    s: cf::CFSocketRef,
    callback_type: cf::CFSocketCallBackType,
    _addr: cf::CFDataRef,
    _data: *const std::ffi::c_void,
    _info: *mut std::ffi::c_void,
) {
    use cf::*;
    
    if callback_type != kCFSocketReadCallBack {
        return;
    }
    
    let fd = unsafe { CFSocketGetNative(s) };
    
    // 读取命令（循环读取直到 ---END--- 或 EOF）
    let mut buf = [0u8; 65536];  // 增大缓冲区
    let mut total = 0;
    
    loop {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().add(total) as *mut std::ffi::c_void, buf.len() - total) };
        
        if n > 0 {
            total += n as usize;
            // 检查是否包含 ---END--- 标记（多行命令结束）
            if buf[..total].windows(9).any(|w| w == b"---END---") {
                // 移除 ---END--- 标记
                if let Some(pos) = buf[..total].windows(9).position(|w| w == b"---END---") {
                    total = pos;  // 截断到 ---END--- 之前
                }
                break;
            }
            // 缓冲区满
            if total >= buf.len() {
                break;
            }
        } else if n == 0 {
            // EOF - 客户端关闭了写入端
            break;
        } else {
            // 错误
            let err = unsafe { *libc::__error() };
            if err != libc::EAGAIN {
                log_msg(&format!("读取错误: {}", err));
                cleanup_client_socket();
                return;
            }
            // EAGAIN - 需要等待更多数据
            break;
        }
    }
    
    if total > 0 {
        let cmd = String::from_utf8_lossy(&buf[..total]);
        let cmd = cmd.trim();
        log_msg(&format!("收到命令: {}", cmd));
        
        // 执行命令（在 Main Thread！）
        let response = execute_command(cmd);
        
        // 发送响应
        let resp_bytes = response.as_bytes();
        unsafe {
            libc::write(fd, resp_bytes.as_ptr() as *mut std::ffi::c_void, resp_bytes.len());
            libc::write(fd, b"\n".as_ptr() as *const std::ffi::c_void, 1);
        }
        
        log_msg(&format!("响应: {}", response));
        CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        
        // 响应发送后，清理客户端 socket
        // 客户端会在收到响应后关闭连接
        log_msg("响应已发送，等待客户端断开");
    } else {
        // 空命令或 EOF，清理连接
        log_msg("客户端断开");
        cleanup_client_socket();
    }
}

fn cleanup_client_socket() {
    #[cfg(target_os = "macos")]
    {
        use cf::*;
        
        unsafe {
            if let Some(sock) = G_CLIENT_SOCKET {
                CFSocketInvalidate(sock);
                CFRelease(sock as *const std::ffi::c_void);
                G_CLIENT_SOCKET = None;
            }
            if let Some(src) = G_CLIENT_SOURCE {
                let runloop = CFRunLoopGetCurrent();
                let mode = get_default_mode();
                CFRunLoopRemoveSource(runloop, src, mode);
                CFRelease(src as *const std::ffi::c_void);
                G_CLIENT_SOURCE = None;
            }
        }
    }
}

/// 清理 CFRunLoop socket（macOS）
#[cfg(target_os = "macos")]
fn cleanup_cfrunloop_socket() {
    use cf::*;
    
    cleanup_client_socket();
    
    unsafe {
        if let Some(sock) = G_LISTEN_SOCKET {
            CFSocketInvalidate(sock);
            CFRelease(sock as *const std::ffi::c_void);
            G_LISTEN_SOCKET = None;
        }
        if let Some(src) = G_LISTEN_SOURCE {
            let runloop = CFRunLoopGetCurrent();
            let mode = get_default_mode();
            CFRunLoopRemoveSource(runloop, src, mode);
            CFRelease(src as *const std::ffi::c_void);
            G_LISTEN_SOURCE = None;
        }
    }
}

/// 停止 IPC 服务
pub fn ipc_stop() -> bool {
    log_msg("=== IPC stop ===");

    RUNNING.store(false, Ordering::SeqCst);

    #[cfg(target_os = "macos")]
    {
        // macOS: 清理 CFRunLoop socket
        cleanup_cfrunloop_socket();
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: 清理命名管道和窗口
        cleanup_win_ipc();
    }

    // 关闭 socket
    #[cfg(unix)]
    if let Ok(mut l) = SOCKET_LISTENER.lock() {
        *l = None;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // 其他平台: 等待线程结束
        if let Ok(mut th) = SOCKET_THREAD.lock() {
            if let Some(handle) = th.take() {
                let _ = handle.join();
            }
        }
    }

    // 清理 Lua state
    unsafe {
        LUA_STATE = None;
    }

    #[cfg(unix)]
    let _ = fs::remove_file(SOCKET_PATH);
    
    log_msg("✓ IPC 服务停止");

    true
}

// ========== Windows IPC 实现 ==========
#[cfg(target_os = "windows")]
fn setup_win_ipc() {
    use win_pipe::*;
    use win_window::*;
    use std::os::windows::ffi::OsStrExt;
    
    log_msg("设置 Windows IPC...");
    
    unsafe {
        // 1. 创建隐藏窗口
        let h_instance = GetModuleHandleW(ptr::null());
        
        let class_name = std::ffi::OsStr::new("MohoIpcWindow")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        
        let wnd_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        
        let atom = RegisterClassW(&wnd_class);
        if atom == 0 {
            log_msg("✗ 注册窗口类失败");
            return;
        }
        
        // 创建 message-only window（不可见）
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            0,
            0, 0, 0, 0,
            HWND_MESSAGE,  // Message-only window
            ptr::null_mut(),
            h_instance,
            ptr::null(),
        );
        
        if hwnd.is_null() {
            log_msg("✗ 创建隐藏窗口失败");
            return;
        }
        
        G_HIDDEN_WINDOW = Some(hwnd);
        log_msg(&format!("✓ 隐藏窗口已创建: {:?}", hwnd));
        
        // 2. 创建命名管道
        let pipe_name: Vec<u16> = std::ffi::OsStr::new(PIPE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        let pipe_handle = CreateNamedPipeW(
            pipe_name.as_ptr() as *mut u16,
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            8192,
            8192,
            0,
            ptr::null_mut(),
        );
        
        if pipe_handle.is_null() {
            log_msg("✗ 创建命名管道失败");
            return;
        }
        
        G_PIPE_HANDLE = Some(pipe_handle);
        log_msg(&format!("✓ 命名管道已创建: {}", PIPE_NAME));
        
        // 3. 开始监听连接（异步）
        // 注意: Windows 命名管道需要调用 ConnectNamedPipe 等待客户端
        // 这里我们创建一个后台线程来监听，然后通过 PostMessage 通知主线程
        let hwnd_usize = hwnd as usize;
        let pipe_handle_usize = pipe_handle as usize;
        thread::spawn(move || {
            listen_pipe_connections(hwnd_usize as win_window::HWND, pipe_handle_usize as win_pipe::HANDLE);
        });
    }
    
    RUNNING.store(true, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
fn listen_pipe_connections(hwnd: win_window::HWND, pipe_handle: win_pipe::HANDLE) {
    use win_pipe::*;
    
    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }
        
        unsafe {
            // 等待客户端连接
            let ret = ConnectNamedPipe(pipe_handle, ptr::null_mut());
            
            if ret != 0 {
                log_msg("✓ 客户端已连接");
                
                // 通知主线程有新连接（通过窗口消息）
                // wParam = pipe_handle, lParam = event_type (1=connect)
                extern "system" {
                    pub fn PostMessageW(hWnd: win_window::HWND, Msg: u32, wParam: usize, lParam: isize) -> i32;
                }
                PostMessageW(hwnd, win_window::WM_PIPE, pipe_handle as usize, 1);
            }
        }
        
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "windows")]
extern "system" fn wnd_proc(
    hwnd: win_window::HWND,
    msg: u32,
    wparam: win_window::WPARAM,
    lparam: win_window::LPARAM,
) -> win_window::LRESULT {
    use win_pipe::*;
    use win_window::*;
    
    if msg == WM_PIPE {
        let pipe_handle = wparam as HANDLE;
        let event_type = lparam as i32;
        
        if event_type == 1 {
            // 新连接
            handle_pipe_connect(pipe_handle);
        } else if event_type == 2 {
            // 数据可读
            handle_pipe_data(pipe_handle);
        }
        
        return 0;
    }
    
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(target_os = "windows")]
fn handle_pipe_connect(pipe_handle: win_pipe::HANDLE) {
    log_msg("处理管道连接...");
    
    // 验证调用者
    if !verify_caller_win(pipe_handle) {
        log_msg("✗ 验证失败，断开连接");
        unsafe {
            win_pipe::DisconnectNamedPipe(pipe_handle);
            win_pipe::CloseHandle(pipe_handle);
        }
        return;
    }
    
    log_msg("✓ 验证通过");
    
    // 保存客户端管道
    unsafe {
        G_CLIENT_PIPE = Some(pipe_handle);
    }
    
    // 创建后台线程读取数据
    let hwnd = unsafe { G_HIDDEN_WINDOW.unwrap() };
    let hwnd_usize = hwnd as usize;
    let pipe_handle_usize = pipe_handle as usize;
    thread::spawn(move || {
        read_pipe_loop(hwnd_usize as win_window::HWND, pipe_handle_usize as win_pipe::HANDLE);
    });
}

#[cfg(target_os = "windows")]
fn read_pipe_loop(hwnd: win_window::HWND, pipe_handle: win_pipe::HANDLE) {
    use win_pipe::*;
    
    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }
        
        unsafe {
            let mut buf = [0u8; 65536];
            let mut bytes_read: u32 = 0;
            
            let ret = ReadFile(
                pipe_handle,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                buf.len() as u32,
                &mut bytes_read,
                ptr::null_mut(),
            );
            
            if ret != 0 && bytes_read > 0 {
                // 通知主线程处理数据
                extern "system" {
                    pub fn PostMessageW(hWnd: win_window::HWND, Msg: u32, wParam: usize, lParam: isize) -> i32;
                }
                
                // 注意: 这里简化处理，实际应该通过共享内存传递数据
                // 暂时直接在后台线程处理（需要验证安全性）
                let data = String::from_utf8_lossy(&buf[..bytes_read as usize]);
                let response = execute_command(data.trim());
                
                // 发送响应
                let mut bytes_written: u32 = 0;
                WriteFile(
                    pipe_handle,
                    response.as_ptr() as *const std::ffi::c_void,
                    response.len() as u32,
                    &mut bytes_written,
                    ptr::null_mut(),
                );
                FlushFileBuffers(pipe_handle);
                
                CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            } else {
                // 连接关闭或错误
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn handle_pipe_data(pipe_handle: win_pipe::HANDLE) {
    // 数据处理在 read_pipe_loop 中完成
}

#[cfg(target_os = "windows")]
fn cleanup_win_ipc() {
    use win_pipe::*;
    use win_window::*;
    
    unsafe {
        // 关闭客户端管道
        if let Some(pipe) = G_CLIENT_PIPE {
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
            G_CLIENT_PIPE = None;
        }
        
        // 关闭监听管道
        if let Some(pipe) = G_PIPE_HANDLE {
            CloseHandle(pipe);
            G_PIPE_HANDLE = None;
        }
        
        // 销毁窗口
        if let Some(hwnd) = G_HIDDEN_WINDOW {
            DestroyWindow(hwnd);
            G_HIDDEN_WINDOW = None;
        }
    }
}

/// 监听循环
#[cfg(unix)]
fn listen_loop() {
    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }

        // 获取 listener
        let listener = match SOCKET_LISTENER.lock() {
            Ok(l) => l,
            Err(_) => break,
        };

        let listener = match listener.as_ref() {
            Some(l) => l,
            None => break,
        };

        // 设置非阻塞
        let _ = listener.set_nonblocking(true);

        match listener.accept() {
            Ok((stream, _addr)) => {
                drop(listener); // 释放锁
                handle_client(stream);
            }
            Err(e) => {
                drop(listener); // 释放锁
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    warn!("accept error: {}", e);
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// 处理客户端连接
#[cfg(unix)]
fn handle_client(mut stream: UnixStream) {
    log_msg("新连接");

    // 设置超时
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    // 读取命令
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        Ok(_) => {
            log_msg("空命令，关闭连接");
            return;
        }
        Err(e) => {
            log_msg(&format!("读取错误: {}", e));
            return;
        }
    };

    let cmd = String::from_utf8_lossy(&buf[..n]);
    let cmd = cmd.trim();

    log_msg(&format!("收到命令: {}", cmd));

    // 执行命令
    let response = execute_command(cmd);

    // 发送响应
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(b"\n");

    log_msg(&format!("响应: {}", response));
    CALL_COUNT.fetch_add(1, Ordering::SeqCst);
}

/// 执行 Lua 命令
/// 
/// ⚠️ macOS: 通过 CFRunLoop 回调，在 Main Thread 执行
/// ⚠️ 其他平台: 在 socket 线程执行
fn execute_command(cmd: &str) -> String {
    execute_command_inner(cmd)
}

/// 内部执行命令
fn execute_command_inner(cmd: &str) -> String {
    // 获取 Lua state
    let L = unsafe { LUA_STATE };

    let L = match L {
        Some(l) => l,
        None => return "error|no Lua state".to_string(),
    };

    if cmd.is_empty() {
        return "error|empty command".to_string();
    }
    
    // 特殊命令处理
    if cmd == "status" {
        let (running, path, calls, errors) = get_status();
        return format!("running={} path={} calls={} errors={}", running, path, calls, errors);
    }

    unsafe {
        // 创建输出捕获包装
        // 使用长字符串 [[ ]] 避免转义问题
        let wrapped = format!(
            r#"local _output = {{}}
local _print = print
print = function(...) table.insert(_output, table.concat({{...}}, "\t")) end
local _ok, _err = pcall(function()
{}
end)
print = _print
if not _ok then return "error|" .. tostring(_err) end
local _result = table.concat(_output, "\n")
if _result and _result ~= "" then return _result else return "ok" end"#,
            cmd
        );
        
        let c_wrapped = match CString::new(wrapped) {
            Ok(c) => c,
            Err(_) => return "error|invalid wrapped command".to_string(),
        };

        // 加载
        let ret = luaL_loadstring(L, c_wrapped.as_ptr());
        if ret != 0 {
            let err = to_string(L, -1).unwrap_or("").to_string();
            ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
            return format!("error|{}", err);
        }

        // 执行（返回 1 个值）
        let ret = lua_pcall(L, 0, 1, 0);
        if ret != 0 {
            let err = to_string(L, -1).unwrap_or("").to_string();
            ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
            format!("error|{}", err)
        } else {
            // 获取返回值
            let result = to_string(L, -1).unwrap_or("ok").to_string();
            lua_pop(L, 1);
            result
        }
    }
}

// ========== 编码 API ==========

pub fn encode_video(input: &str, output: &str, fps: i32, crf: i32, _codec: &str) -> (bool, String) {
    info!("encode_video: {} -> {} (fps={}, crf={})", input, output, fps, crf);

    if ENCODE_STATUS.load(Ordering::SeqCst) == 1 {
        return (false, "编码正在进行中".to_string());
    }

    ENCODE_STATUS.store(1, Ordering::SeqCst);
    ENCODE_PROGRESS.store(0, Ordering::SeqCst);

    let input = input.to_string();
    let output = output.to_string();

    thread::spawn(move || {
        let result = do_encode(&input, &output, fps, crf);

        match result {
            Ok(_) => {
                ENCODE_STATUS.store(2, Ordering::SeqCst);
                info!("编码完成: {}", output);
            }
            Err(e) => {
                ENCODE_STATUS.store(3, Ordering::SeqCst);
                let err_msg = format!("{}", e);
                if let Ok(mut error) = ENCODE_ERROR.lock() {
                    *error = err_msg;
                }
                error!("编码失败: {}", e);
            }
        }
    });

    (true, "编码已启动".to_string())
}

fn do_encode(input: &str, output: &str, fps: i32, crf: i32) -> anyhow::Result<()> {
    use std::process::Command;

    // 检测输出格式
    let output_ext = if output.ends_with(".gif") {
        "gif"
    } else if output.ends_with(".apng") || output.ends_with(".png") {
        "apng"
    } else {
        "mp4"
    };

    info!("编码格式: {}", output_ext);

    // macOS: 优先使用内置 FFmpeg（自定义 FFI）
    #[cfg(all(target_os = "macos", feature = "ffmpeg-builtin"))]
    {
        let check_result = crate::encode_native::check_ffmpeg_available();
        eprintln!("[DEBUG] check_ffmpeg_available: {}", check_result);
        eprintln!("[DEBUG] input: {}, output: {}", input, output);
        eprintln!("[DEBUG] output_ext: {}", output_ext);

        if check_result {
            eprintln!("[DEBUG] 使用 Moho 内置 FFmpeg (自定义 FFI)");
            info!("使用 Moho 内置 FFmpeg (自定义 FFI)");
            return crate::encode_native::encode_with_builtin_ffmpeg(input, output, fps, crf);
        }
    }

    // 回退到系统 ffmpeg
    let ffmpeg_path = which::which("ffmpeg").ok();
    
    let ffmpeg = match ffmpeg_path {
        Some(f) => f,
        None => {
            anyhow::bail!(
                "未找到 ffmpeg。请选择以下方案之一：\n\
                  方案 A - 安装系统 ffmpeg（推荐）：\n\
                    brew install ffmpeg\n\
                  方案 B - 使用 C 版本 IPC 模式：\n\
                    cp moho-mate.c.bak moho-mate\n\
                    moho-mate start project.moho\n\
                    moho-mate encode ..."
            );
        }
    };
    
    info!("使用系统 ffmpeg: {:?}", ffmpeg);
    
    let result = if output_ext == "gif" {
        // GIF: 使用 libavfilter 调色板优化
        info!("GIF 调色板优化: palettegen + paletteuse");
        Command::new(&ffmpeg)
            .args([
                "-y",
                "-framerate", &fps.to_string(),
                "-i", input,
                "-vf", "fps=24,scale=320:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5",
                "-loop", "0",
                output,
            ])
            .output()
    } else if output_ext == "apng" {
        // APNG: 动画 PNG，无损 + 透明
        let actual_output = if output.ends_with(".apng") {
            output.replace(".apng", ".png")
        } else {
            output.to_string()
        };
        
        info!("APNG 编码: {}", actual_output);
        Command::new(&ffmpeg)
            .args([
                "-y",
                "-framerate", &fps.to_string(),
                "-i", input,
                "-plays", "0",
                "-f", "apng",
                &actual_output,
            ])
            .output()
    } else {
        // MP4: 使用 mpeg4 编码器
        info!("MP4 编码: mpeg4, crf={}", crf);
        Command::new(&ffmpeg)
            .args([
                "-y",
                "-framerate", &fps.to_string(),
                "-i", input,
                "-c:v", "mpeg4",
                "-q:v", &crf.to_string(),
                "-pix_fmt", "yuv420p",
                output,
            ])
            .output()
    };

    ENCODE_PROGRESS.store(10000, Ordering::SeqCst);

    match result {
        Ok(output_result) => {
            if output_result.status.success() {
                info!("编码完成: {}", output);
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output_result.stderr);
                anyhow::bail!("FFmpeg 错误: {}", stderr);
            }
        }
        Err(e) => {
            anyhow::bail!("FFmpeg 执行失败: {}", e);
        }
    }
}
pub fn encode_status() -> (i32, &'static str, i32, String) {
    let status = ENCODE_STATUS.load(Ordering::SeqCst);
    let progress = ENCODE_PROGRESS.load(Ordering::SeqCst);

    let status_text = match status {
        0 => "idle",
        1 => "running",
        2 => "success",
        3 => "error",
        _ => "unknown",
    };
    
    let error_msg = if let Ok(error) = ENCODE_ERROR.lock() {
        error.clone()
    } else {
        String::new()
    };

    (status, status_text, progress, error_msg)
}

pub fn encode_cancel() -> bool {
    if ENCODE_STATUS.load(Ordering::SeqCst) == 1 {
        ENCODE_STATUS.store(0, Ordering::SeqCst);
        true
    } else {
        false
    }
}

// ========== 状态查询 ==========

pub fn get_status() -> (bool, String, usize, usize) {
    let running = RUNNING.load(Ordering::SeqCst);
    let calls = CALL_COUNT.load(Ordering::SeqCst);
    let errors = ERROR_COUNT.load(Ordering::SeqCst);

    #[cfg(unix)]
    let path = SOCKET_PATH.to_string();
    #[cfg(target_os = "windows")]
    let path = PIPE_NAME.to_string();

    (running, path, calls, errors)
}
