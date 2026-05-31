/*
 * moho_ipc_cmd.c - Moho IPC 命令行工具
 *
 * 用法:
 *   moho_ipc_cmd "***"
 *   moho_ipc_cmd -f script.lua
 *   moho_ipc_cmd --status
 *
 * Token 刷新机制:
 *   - 首次使用文件 token (~/.moho_ipc_token)
 *   - 过期时自动调用 HTTP /token/create 获取新 token
 *   - 新 token 写入文件供下次使用
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <libgen.h>
#include <errno.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <time.h>

#define SOCKET_PATH "/tmp/moho_ipc.sock"
#define CMD_DIR "/tmp/moho_ipc_cmds"
#define RECV_BUF_SIZE 16384
#define TOKEN_SERVICE_HOST "127.0.0.1"
#define TOKEN_SERVICE_PORT 9527
#define HTTP_BUF_SIZE 4096
#define CLIENT_ID "moho_ipc_client"

// ========== HTTP Token 服务 ========== 

// 从 HTTP 服务获取新 token
static int fetch_new_token(char *token_out, size_t token_size) {
    int sock;
    struct sockaddr_in addr;
    char request[HTTP_BUF_SIZE];
    char response[HTTP_BUF_SIZE];
    ssize_t len;
    
    // 创建 socket
    sock = socket(AF_INET, SOCK_STREAM, 0);
    if (sock < 0) {
        return 0;
    }
    
    // 设置超时
    struct timeval timeout = {.tv_sec = 5, .tv_usec = 0};
    setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
    setsockopt(sock, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout));
    
    // 连接 Token 服务
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons(TOKEN_SERVICE_PORT);
    addr.sin_addr.s_addr = inet_addr(TOKEN_SERVICE_HOST);
    
    if (connect(sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        close(sock);
        return 0;
    }
    
    // 构造 HTTP POST 请求
    char body[256];
    snprintf(body, sizeof(body), "{\"client_id\":\"%s\",\"user\":\"%s\"}", CLIENT_ID, getenv("USER") ?: "unknown");
    
    snprintf(request, sizeof(request),
        "POST /token/create HTTP/1.1\r\n"
        "Host: %s:%d\r\n"
        "Content-Type: application/json\r\n"
        "Content-Length: %zu\r\n"
        "\r\n"
        "%s",
        TOKEN_SERVICE_HOST, TOKEN_SERVICE_PORT, strlen(body), body);
    
    // 发送请求
    if (send(sock, request, strlen(request), 0) < 0) {
        close(sock);
        return 0;
    }
    
    // 接收响应
    len = recv(sock, response, sizeof(response) - 1, 0);
    close(sock);
    
    if (len <= 0) {
        return 0;
    }
    response[len] = 0;
    
    // 解析 JSON: {"token":"xxx",...}
    // 简单字符串匹配
    char *token_start = strstr(response, "\"token\":\"");
    if (!token_start) {
        return 0;
    }
    token_start += 9;  // 跳过 "token":"
    
    char *token_end = strchr(token_start, '"');
    if (!token_end) {
        return 0;
    }
    
    size_t token_len = token_end - token_start;
    if (token_len >= token_size) {
        token_len = token_size - 1;
    }
    strncpy(token_out, token_start, token_len);
    token_out[token_len] = 0;
    
    return 1;
}

// ========== IPC 命令发送 ==========

static int send_command(const char *cmd, int silent) {
    int sock;
    struct sockaddr_un addr;
    char response[RECV_BUF_SIZE];
    ssize_t len;
    char token[128] = {0};
    char auth_cmd[256];

    // 创建 socket
    sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sock < 0) {
        if (!silent) fprintf(stderr, "✗ Socket 创建失败: %s\n", strerror(errno));
        return 1;
    }

    // 连接
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, SOCKET_PATH, sizeof(addr.sun_path) - 1);

    if (connect(sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        if (!silent) {
            if (errno == ENOENT || errno == ECONNREFUSED) {
                fprintf(stderr, "✗ Moho IPC 未启动\n");
            } else {
                fprintf(stderr, "✗ 连接失败: %s\n", strerror(errno));
            }
        }
        close(sock);
        return 1;
    }

    // 直接从 HTTP 服务获取 token（不读文件）
    if (!fetch_new_token(token, sizeof(token))) {
        if (!silent) fprintf(stderr, "✗ Token 获取失败（Token 服务未运行？）\n");
        close(sock);
        return 1;
    }

    // 发送 auth 命令
    snprintf(auth_cmd, sizeof(auth_cmd), "auth %s", token);
    if (send(sock, auth_cmd, strlen(auth_cmd), 0) < 0) {
        if (!silent) fprintf(stderr, "✗ 发送 auth 失败: %s\n", strerror(errno));
        close(sock);
        return 1;
    }

    // 接收 auth 响应
    len = recv(sock, response, RECV_BUF_SIZE - 1, 0);
    if (len <= 0) {
        if (!silent) fprintf(stderr, "✗ Auth 无响应\n");
        close(sock);
        return 1;
    }
    response[len] = '\0';

    // 检查 auth 结果
    if (strncmp(response, "ok|authenticated", 16) != 0) {
        if (!silent) fprintf(stderr, "✗ Token 验证失败: %s\n", response);
        close(sock);
        return 1;
    }

    // 发送实际命令
    if (send(sock, cmd, strlen(cmd), 0) < 0) {
        if (!silent) fprintf(stderr, "✗ 发送失败: %s\n", strerror(errno));
        close(sock);
        return 1;
    }

    // 接收响应
    len = recv(sock, response, RECV_BUF_SIZE - 1, 0);
    close(sock);

    if (len <= 0) {
        if (!silent) fprintf(stderr, "✗ 无响应\n");
        return 1;
    }

    response[len] = '\0';

    // 解析响应格式: "ok|输出" 或 "error|错误信息"
    if (strncmp(response, "ok|", 3) == 0) {
        if (!silent) {
            printf("%s", response + 3);
            // 确保换行
            if (response[len-1] != '\n') printf("\n");
        }
        return 0;
    } else if (strncmp(response, "error|", 6) == 0) {
        if (!silent) fprintf(stderr, "✗ %s\n", response + 6);
        return 1;
    } else {
        // 旧格式兼容
        if (!silent) printf("%s", response);
        return strncmp(response, "OK", 2) == 0 ? 0 : 1;
    }
}

static int send_file(const char *filepath, int silent) {
    FILE *f;
    char *cmd_content;
    long fsize;
    char cmd_file[256];
    char cmd[512];
    int ret;

    // 读取文件
    f = fopen(filepath, "r");
    if (!f) {
        if (!silent) fprintf(stderr, "✗ 文件不存在: %s\n", filepath);
        return 1;
    }

    fseek(f, 0, SEEK_END);
    fsize = ftell(f);
    fseek(f, 0, SEEK_SET);

    cmd_content = malloc(fsize + 1);
    if (!cmd_content) {
        if (!silent) fprintf(stderr, "✗ 内存分配失败\n");
        fclose(f);
        return 1;
    }

    fread(cmd_content, 1, fsize, f);
    cmd_content[fsize] = '\0';
    fclose(f);

    // 创建命令目录
    mkdir(CMD_DIR, 0755);

    // 写入固定位置
    snprintf(cmd_file, sizeof(cmd_file), "%s/current.lua", CMD_DIR);
    f = fopen(cmd_file, "w");
    if (!f) {
        if (!silent) fprintf(stderr, "✗ 无法写入临时文件\n");
        free(cmd_content);
        return 1;
    }
    fprintf(f, "%s", cmd_content);
    fclose(f);
    free(cmd_content);

    // 发送 dofile 命令
    snprintf(cmd, sizeof(cmd), "dofile(\"%s\")", cmd_file);
    
    if (!silent) printf("[→] 发送文件: %s\n", filepath);
    ret = send_command(cmd, silent);
    
    return ret;
}

static int check_status(int silent) {
    struct stat st;
    
    if (stat(SOCKET_PATH, &st) == 0) {
        if (!silent) {
            printf("✓ IPC 运行中\n");
            printf("  Socket: %s\n", SOCKET_PATH);
        }
        return 0;
    } else {
        if (!silent) {
            printf("✗ IPC 未启动\n");
        }
        return 1;
    }
}

static void print_usage(const char *prog) {
    printf("Moho IPC 命令行工具\n\n");
    printf("用法:\n");
    printf("  %s '<lua_command>'     发送 Lua 命令\n", prog);
    printf("  %s -f <script.lua>      发送 Lua 文件\n", prog);
    printf("  %s --status             检查 IPC 状态\n", prog);
    printf("  %s -q '<cmd>'           静默模式\n", prog);
    printf("\n示例:\n");
    printf("  %s 'print(\"hello\")'\n", prog);
    printf("  %s 'ping()'\n", prog);
    printf("  %s -f script.lua\n", prog);
}

int main(int argc, char **argv) {
    int silent = 0;
    int i = 1;
    
    if (argc < 2) {
        print_usage(argv[0]);
        return 1;
    }
    
    // 解析选项
    if (strcmp(argv[1], "-q") == 0 || strcmp(argv[1], "--quiet") == 0) {
        silent = 1;
        i = 2;
    }
    
    if (i >= argc) {
        print_usage(argv[0]);
        return 1;
    }
    
    // --status
    if (strcmp(argv[i], "--status") == 0 || strcmp(argv[i], "-s") == 0) {
        return check_status(silent);
    }
    
    // --help
    if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
        print_usage(argv[0]);
        return 0;
    }
    
    // -f 文件模式
    if (strcmp(argv[i], "-f") == 0 || strcmp(argv[i], "--file") == 0) {
        if (i + 1 >= argc) {
            fprintf(stderr, "✗ 缺少文件路径\n");
            return 1;
        }
        return send_file(argv[i + 1], silent);
    }
    
    // 命令模式
    return send_command(argv[i], silent);
}
