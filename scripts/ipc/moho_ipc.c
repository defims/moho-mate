/*
 * moho_ipc.c - Moho IPC with CFSocket + ScriptInterfaceHelper
 *
 * 原理：
 * 1. CFSocket 收到命令后调用 Lua 的 ipc_execute()
 * 2. ipc_execute() 内部使用 MOHO.ScriptInterfaceHelper 获取 moho
 * 3. 官方推荐的"敏感资源"管理方式，稳定不崩溃
 *
 * 用法：
 * local ipc = require("moho_ipc")
 * ipc.start()  -- 启动 IPC 服务
 *
 * 发送命令：
 * nc -U /tmp/moho_ipc.sock <<< 'print("hello")'
 * moho-mate ipc send 'ping()'
 */

#include <lua.h>
#include <lauxlib.h>
#include <CoreFoundation/CoreFoundation.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <fcntl.h>
#include <string.h>
#include <stdio.h>
#include <stdarg.h>

#define SOCKET_PATH "/tmp/moho_ipc.sock"
#define CMD_SIZE 8192
#define RESP_SIZE 16384  // 响应缓冲区大小

static lua_State *g_L = NULL;
static char g_response[RESP_SIZE];  // 存储响应
static CFSocketRef g_listen_socket = NULL;
static CFSocketRef g_client_socket = NULL;
static CFRunLoopSourceRef g_listen_source = NULL;
static CFRunLoopSourceRef g_client_source = NULL;
static int g_call_count = 0;
static int g_error_count = 0;

// 日志
static void log_msg(const char *fmt, ...) {
    FILE *f = fopen("/tmp/moho_ipc.log", "a");
    if (f) {
        fprintf(f, "[ipc] ");
        va_list args;
        va_start(args, fmt);
        vfprintf(f, fmt, args);
        va_end(args);
        fclose(f);
    }
}

// 执行命令（通过 Lua 的 ipc_execute，内部使用 ScriptInterfaceHelper）
// 返回响应字符串（存储在 g_response）
static const char* execute_via_helper(const char *cmd) {
    if (g_L == NULL) {
        log_msg("✗ g_L is NULL\n");
        return "error|g_L is NULL";
    }

    // 获取 ipc_execute 函数
    lua_getglobal(g_L, "ipc_execute");
    if (!lua_isfunction(g_L, -1)) {
        log_msg("✗ ipc_execute not found\n");
        lua_pop(g_L, 1);
        return "error|ipc_execute not found";
    }

    // 推入命令参数
    lua_pushstring(g_L, cmd);

    g_call_count++;
    int ret = lua_pcall(g_L, 1, 1, 0);

    if (ret != 0) {
        log_msg("✗ lua_pcall failed (ret=%d): %s\n", ret, lua_tostring(g_L, -1));
        lua_pop(g_L, 1);
        g_error_count++;
        return "error|lua_pcall failed";
    }

    // 获取返回值并存储到 g_response
    const char *result = lua_tostring(g_L, -1);
    if (result) {
        strncpy(g_response, result, RESP_SIZE - 1);
        g_response[RESP_SIZE - 1] = 0;
    } else {
        strcpy(g_response, "ok|(nil)");
    }
    lua_pop(g_L, 1);

    log_msg("ipc_execute returned: %.100s (calls=%d, errors=%d)\n",
            g_response, g_call_count, g_error_count);

    return g_response;
}

// 客户端 socket 回调
static void client_callback(CFSocketRef s, CFSocketCallBackType type,
                            CFDataRef addr, const void *data, void *info) {
    if (type != kCFSocketReadCallBack) return;

    int fd = CFSocketGetNative(s);
    char buf[CMD_SIZE];
    ssize_t n = read(fd, buf, CMD_SIZE - 1);

    if (n > 0) {
        buf[n] = 0;
        log_msg("收到命令 (%zd bytes): %.60s...\n", n, buf);

        // 通过 ipc_execute 执行（内部使用 ScriptInterfaceHelper）
        const char *response = execute_via_helper(buf);

        // 发送完整响应
        int resp_len = strlen(response);
        write(fd, response, resp_len);
        write(fd, "\n", 1);
    } else if (n == 0 || (n < 0 && errno != EAGAIN)) {
        log_msg("客户端断开\n");
        if (g_client_socket) {
            CFSocketInvalidate(g_client_socket);
            CFRelease(g_client_socket);
            g_client_socket = NULL;
        }
        if (g_client_source) {
            CFRelease(g_client_source);
            g_client_source = NULL;
        }
    }
}

// 监听 socket 回调
static void listen_callback(CFSocketRef s, CFSocketCallBackType type,
                            CFDataRef addr, const void *data, void *info) {
    if (type != kCFSocketAcceptCallBack) return;

    int client_fd = *(int *)data;
    log_msg("新连接: fd=%d\n", client_fd);

    // 关闭旧连接
    if (g_client_socket) {
        CFSocketInvalidate(g_client_socket);
        CFRelease(g_client_socket);
        g_client_socket = NULL;
    }
    if (g_client_source) {
        CFRelease(g_client_source);
        g_client_source = NULL;
    }

    // 设置非阻塞
    int flags = fcntl(client_fd, F_GETFL, 0);
    fcntl(client_fd, F_SETFL, flags | O_NONBLOCK);

    // 创建客户端 CFSocket
    CFSocketContext ctx = {0, NULL, NULL, NULL, NULL};
    g_client_socket = CFSocketCreateWithNative(
        kCFAllocatorDefault, client_fd,
        kCFSocketReadCallBack, client_callback, &ctx
    );

    if (!g_client_socket) {
        log_msg("✗ 创建客户端 CFSocket 失败\n");
        close(client_fd);
        return;
    }

    // 添加到 RunLoop
    g_client_source = CFSocketCreateRunLoopSource(
        kCFAllocatorDefault, g_client_socket, 0
    );
    CFRunLoopAddSource(CFRunLoopGetCurrent(), g_client_source, kCFRunLoopDefaultMode);

    log_msg("✓ 客户端已注册到 RunLoop\n");
}

// Lua API: start()
static int l_start(lua_State *L) {
    log_msg("=== IPC start ===\n");

    // 保存 lua_State
    g_L = L;

    if (g_listen_socket) {
        lua_pushboolean(L, 1);
        lua_pushstring(L, "already running");
        return 2;
    }

    unlink(SOCKET_PATH);

    int sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sock < 0) {
        lua_pushboolean(L, 0);
        lua_pushstring(L, "socket() failed");
        return 2;
    }

    int flags = fcntl(sock, F_GETFL, 0);
    fcntl(sock, F_SETFL, flags | O_NONBLOCK);

    int opt = 1;
    setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, SOCKET_PATH, sizeof(addr.sun_path) - 1);

    if (bind(sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        close(sock);
        lua_pushboolean(L, 0);
        lua_pushstring(L, "bind() failed");
        return 2;
    }

    if (listen(sock, 5) < 0) {
        close(sock);
        lua_pushboolean(L, 0);
        lua_pushstring(L, "listen() failed");
        return 2;
    }

    // 创建 CFSocket
    CFSocketContext ctx = {0, NULL, NULL, NULL, NULL};
    g_listen_socket = CFSocketCreateWithNative(
        kCFAllocatorDefault, sock,
        kCFSocketAcceptCallBack, listen_callback, &ctx
    );

    if (!g_listen_socket) {
        close(sock);
        lua_pushboolean(L, 0);
        lua_pushstring(L, "CFSocketCreateWithNative failed");
        return 2;
    }

    // 添加到 RunLoop
    g_listen_source = CFSocketCreateRunLoopSource(
        kCFAllocatorDefault, g_listen_socket, 0
    );
    CFRunLoopAddSource(CFRunLoopGetCurrent(), g_listen_source, kCFRunLoopDefaultMode);

    log_msg("✓ IPC 服务启动: %s (fd=%d)\n", SOCKET_PATH, sock);
    log_msg("✓ RunLoop: %p (主=%p)\n", CFRunLoopGetCurrent(), CFRunLoopGetMain());

    lua_pushboolean(L, 1);
    lua_pushstring(L, SOCKET_PATH);
    return 2;
}

// Lua API: stop()
static int l_stop(lua_State *L) {
    log_msg("=== IPC stop ===\n");

    if (g_client_socket) {
        CFRunLoopRemoveSource(CFRunLoopGetCurrent(), g_client_source, kCFRunLoopDefaultMode);
        CFSocketInvalidate(g_client_socket);
        CFRelease(g_client_socket);
        g_client_socket = NULL;
        CFRelease(g_client_source);
        g_client_source = NULL;
    }

    if (g_listen_socket) {
        CFRunLoopRemoveSource(CFRunLoopGetCurrent(), g_listen_source, kCFRunLoopDefaultMode);
        CFSocketInvalidate(g_listen_socket);
        CFRelease(g_listen_socket);
        g_listen_socket = NULL;
        CFRelease(g_listen_source);
        g_listen_source = NULL;
    }

    g_L = NULL;
    unlink(SOCKET_PATH);
    log_msg("✓ IPC 服务停止\n");
    lua_pushboolean(L, 1);
    return 1;
}

// Lua API: status()
static int l_status(lua_State *L) {
    lua_pushboolean(L, g_listen_socket ? 1 : 0);
    lua_pushstring(L, SOCKET_PATH);
    lua_pushboolean(L, g_client_socket ? 1 : 0);
    lua_pushinteger(L, g_call_count);
    lua_pushinteger(L, g_error_count);
    return 5;
}

// Lua API: check() - 兼容 LayerScript 版 API（返回 nil）
static int l_check(lua_State *L) {
    lua_pushnil(L);
    return 1;
}

// Lua API: poll() - 兼容旧 API
static int l_poll(lua_State *L) {
    lua_pushinteger(L, 0);
    return 1;
}

// 模块注册
static const luaL_Reg funcs[] = {
    {"start", l_start},
    {"stop", l_stop},
    {"status", l_status},
    {"check", l_check},
    {"poll", l_poll},
    {NULL, NULL}
};

int luaopen_moho_ipc(lua_State *L) {
    luaL_newlib(L, funcs);
    log_msg("模块加载 (ScriptInterfaceHelper 执行版)\n");
    return 1;
}