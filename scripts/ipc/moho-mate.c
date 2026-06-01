/*
 * moho-mate.c - Moho 命令行工具(统一版:CLI + Lua 模块)
 *
 * 功能:
 *   1. 命令行工具: start, call, quit, status, render, encode
 *   2. Lua 模块: require("moho_ipc") 提供 IPC 服务
 *
 * 用法:
 *   moho-mate start [project.moho] [script.lua]
 *   moho-mate call '<lua>'
 *   moho-mate quit
 *   moho-mate status
 *   moho-mate render project.moho -f PNG -o output
 *   moho-mate encode input output --fps 24
 */

// ========== 标准库 ==========
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <stdarg.h>
#include <fcntl.h>
#include <errno.h>
#include <dirent.h>
#include <libgen.h>
#include <time.h>
#include <spawn.h>
#include <signal.h>
#include <pthread.h>
#include <dispatch/dispatch.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/time.h>
#include <netinet/in.h>
#include <arpa/inet.h>

// ========== Lua ==========
#include <lua.h>
#include <lauxlib.h>

// ========== macOS ==========
#include <CoreFoundation/CoreFoundation.h>

// ========== libcurl(已移除)==========

// ========== FFmpeg ==========
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
#include <libswscale/swscale.h>
#include <libavfilter/avfilter.h>
#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>

// ========== 配置 ==========
#define MOHO_APP "/Applications/Moho.app"
#define SOCKET_PATH "/tmp/moho_ipc.sock"
#define IPC_SOCKET "/tmp/moho_ipc.sock"
#define IPC_CMD_DIR "/tmp/moho_ipc_cmds"
#define IPC_TOOL "/Users/def/.openclaw/workspace/skills/moho-mate/scripts/ipc/moho_ipc.lua"
#define MOHO_CONFIG_DIR "/Users/def/Library/Preferences/Lost Marble/Moho Pro/14"
#define SCRIPTS_DIR "/Users/def/.openclaw/workspace/skills/moho-mate/scripts"
#define IPC_CONFIG_BACKUP "/tmp/moho_ipc_config_backup"
#define IPC_BACKUP_PID_FILE "/tmp/moho_ipc_backup.pid"
#define EMPTY_CONFIG_TEMPLATE "/Users/def/.openclaw/workspace/skills/moho-mate/scripts/ipc/empty_config"

#define CMD_SIZE 8192
#define RESP_SIZE 16384
#define OUTPUT_SIZE 16384
#define HTTP_BUF_SIZE 4096

// ========== 配置管理(IPC 自动备份/恢复) ==========

// IPC 配置备份(启动前)
static int ipc_config_backup(void) {
    // 备份到固定目录(不区分 PID)
    char backup_dir[512];
    snprintf(backup_dir, sizeof(backup_dir), "%s", IPC_CONFIG_BACKUP);

    // 清理旧备份
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "rm -rf \"%s\" 2>/dev/null || true", backup_dir);
    system(cmd);

    snprintf(cmd, sizeof(cmd), "mkdir -p \"%s\"", backup_dir);
    system(cmd);

    snprintf(cmd, sizeof(cmd), "cp -R \"%s\"/ \"%s\"/", MOHO_CONFIG_DIR, backup_dir);
    int ret = system(cmd);

    if (ret == 0) {
        printf("✓ 配置已备份: %s\n", backup_dir);
        // 写入 PID 文件(标记 IPC 会话)
        FILE *f = fopen(IPC_BACKUP_PID_FILE, "w");
        if (f) {
            fprintf(f, "%d\n", getpid());
            fclose(f);
        }
    } else {
        fprintf(stderr, "✗ 配置备份失败\n");
    }
    return ret;
}

// IPC 使用空配置(清空 autosave)
static int ipc_config_use_empty(void) {
    char autosave_dir[512];
    snprintf(autosave_dir, sizeof(autosave_dir), "%s/Autosave", MOHO_CONFIG_DIR);

    char cmd[512];
    snprintf(cmd, sizeof(cmd), "rm -rf \"%s\"/* 2>/dev/null || true", autosave_dir);
    system(cmd);

    printf("✓ Autosave 已清空\n");

    char template_path[512];
    snprintf(template_path, sizeof(template_path), "%s", EMPTY_CONFIG_TEMPLATE);

    struct stat st;
    if (stat(template_path, &st) == 0 && S_ISDIR(st.st_mode)) {
        snprintf(cmd, sizeof(cmd), "cp -R \"%s\"/ \"%s\"/", template_path, MOHO_CONFIG_DIR);
        system(cmd);
        printf("✓ 空配置模板已应用\n");
    }

    return 0;
}

// IPC 配置恢复(退出后)
static int ipc_config_restore(void) {
    char backup_dir[512];
    snprintf(backup_dir, sizeof(backup_dir), "%s", IPC_CONFIG_BACKUP);

    // 检查 PID 文件(确认 IPC 会话)
    FILE *f = fopen(IPC_BACKUP_PID_FILE, "r");
    if (!f) {
        printf("⚠ 无 IPC 会话标记,跳过恢复\n");
        return 0;
    }
    fclose(f);

    struct stat st;
    if (stat(backup_dir, &st) != 0) {
        printf("⚠ 无配置备份,跳过恢复\n");
        unlink(IPC_BACKUP_PID_FILE);
        return 0;
    }

    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "rm -rf \"%s\"/* 2>/dev/null || true", MOHO_CONFIG_DIR);
    system(cmd);

    snprintf(cmd, sizeof(cmd), "cp -R \"%s\"/ \"%s\"/", backup_dir, MOHO_CONFIG_DIR);
    int ret = system(cmd);

    if (ret == 0) {
        printf("✓ 配置已恢复\n");
        snprintf(cmd, sizeof(cmd), "rm -rf \"%s\"", backup_dir);
        system(cmd);
        unlink(IPC_BACKUP_PID_FILE);
    } else {
        fprintf(stderr, "✗ 配置恢复失败\n");
    }
    return ret;
}

// ========== IPC 辅助函数 ==========

// ========== IPC 客户端 ==========

static int ipc_send_raw(const char *cmd) {
    int sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sock < 0) {
        fprintf(stderr, "✗ Socket 创建失败\n");
        return 1;
    }

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, IPC_SOCKET, sizeof(addr.sun_path) - 1);

    if (connect(sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        close(sock);
        fprintf(stderr, "✗ IPC 连接失败(服务未启动?)\n");
        return 1;
    }

    // 发送命令
    send(sock, cmd, strlen(cmd), 0);
    send(sock, "\n", 1, 0);

    // 接收响应
    char resp[1024];
    int n = recv(sock, resp, sizeof(resp) - 1, 0);
    close(sock);

    if (n > 0) {
        resp[n] = 0;
        // 跳过开头的空白字符
        char *p = resp;
        while (*p == '\n' || *p == '\r' || *p == ' ') p++;
        // 去除尾部换行
        int len = strlen(p);
        while (len > 0 && (p[len-1] == '\n' || p[len-1] == '\r')) p[--len] = 0;

        if (strncmp(p, "ok|", 3) == 0) {
            printf("%s\n", p + 3);
            return 0;
        } else {
            fprintf(stderr, "✗ %s\n", p);
            return 1;
        }
    }

    return 0;
}

static int ipc_send(const char *cmd) {
    return ipc_send_raw(cmd);
}

static int ipc_send_file(const char *filepath) {
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "dofile(\"%s\")", filepath);
    return ipc_send_raw(cmd);
}

static int ipc_send_multiline(const char *code) {
    // 固定文件名,每次覆盖写入(moho-mate 单线程顺序执行)
    mkdir(IPC_CMD_DIR, 0755);

    const char *tmpfile = IPC_CMD_DIR "/cmd.lua";

    FILE *f = fopen(tmpfile, "w");
    if (!f) {
        fprintf(stderr, "✗ 无法写入临时文件\n");
        return 1;
    }
    fprintf(f, "%s", code);
    fclose(f);

    return ipc_send_file(tmpfile);
}

static int ipc_check_running(void) {
    struct stat st;
    return stat(IPC_SOCKET, &st) == 0;
}

static int auto_start_ipc(void) {
    if (ipc_check_running()) return 0;

    printf("▶ IPC 未启动,自动启动...\n");

    // 启动 Moho IPC
    char moho_path[512];
    snprintf(moho_path, sizeof(moho_path), "%s/Contents/MacOS/Moho", MOHO_APP);

    // 创建临时 wrapper
    mkdir(IPC_CMD_DIR, 0755);
    char wrapper_path[256];
    snprintf(wrapper_path, sizeof(wrapper_path), "%s/wrapper.lua", IPC_CMD_DIR);

    FILE *f = fopen(wrapper_path, "w");
    if (!f) {
        fprintf(stderr, "✗ 无法创建 wrapper\n");
        return 1;
    }

    // 写入 IPC 启动代码
    fprintf(f, "dofile(\"%s\")\n", IPC_TOOL);
    fclose(f);

    // 启动 Moho(用 open 命令)
    char open_cmd[1024];
    snprintf(open_cmd, sizeof(open_cmd), "open -a Moho --args \"%s\"", wrapper_path);
    system(open_cmd);
    printf("Moho 已启动\n");

    // 等待 socket
    for (int i = 0; i < 30; i++) {
        sleep(1);
        if (ipc_check_running()) {
            printf("✓ IPC 已启动\n");
            sleep(1);  // 等 Moho 完全就绪
            return 0;
        }
    }

    fprintf(stderr, "✗ IPC 启动超时\n");
    return 1;
}

// ========== start ==========

static int cmd_start(int argc, char **argv) {
    char *project = NULL;
    char *script = NULL;
    int timeout = 3600;

    // 解析参数
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--timeout") == 0 || strcmp(argv[i], "-t") == 0) {
            if (i + 1 < argc) timeout = atoi(argv[++i]);
        } else if (argv[i][0] != '-') {
            if (!project && strstr(argv[i], ".moho")) {
                project = argv[i];
            } else if (!script) {
                script = argv[i];
            }
        }
    }

    printf("▶ 启动 IPC 服务\n");
    printf("  超时: %d 秒\n", timeout);
    if (project) printf("  项目: %s\n", project);
    if (script) printf("  脚本: %s\n", script);

    // 杀掉旧 Moho
    system("pkill -9 Moho 2>/dev/null || true");
    unlink(IPC_SOCKET);
    sleep(1);

    // 备份配置 + 使用空配置
    ipc_config_backup();
    ipc_config_use_empty();

    // 启动 Moho IPC
    char moho_path[512];
    snprintf(moho_path, sizeof(moho_path), "%s/Contents/MacOS/Moho", MOHO_APP);

    mkdir(IPC_CMD_DIR, 0755);
    char wrapper_path[256];
    snprintf(wrapper_path, sizeof(wrapper_path), "%s/wrapper.lua", IPC_CMD_DIR);

    FILE *f = fopen(wrapper_path, "w");
    if (!f) {
        fprintf(stderr, "✗ 无法创建 wrapper\n");
        return 1;
    }

    // 写入 IPC 启动代码(使用 ipc.lua 模板)
    // 设置变量供 ipc.lua 使用
    fprintf(f, "IPC_DIR = \"%s\"\n", SCRIPTS_DIR);
    fprintf(f, "USER_PROJECT = \"%s\"\n", project ? project : "");
    fprintf(f, "USER_SCRIPT = \"%s\"\n", script ? script : "");
    fprintf(f, "IPC_TIMEOUT = %d\n", timeout);
    
    // ⚠️ 生成启动令牌（防止其他脚本启动 IPC）
    srand((unsigned int)time(NULL) ^ (unsigned int)getpid());
    char token[64];
    snprintf(token, sizeof(token), "%ld_%d_%d", time(NULL), getpid(), rand() % 10000);
    fprintf(f, "IPC_START_TOKEN = \"%s\"\n", token);
    
    // 写入令牌文件（只有 moho-mate 可读）
    FILE *token_file = fopen("/tmp/moho_ipc_token", "w");
    if (token_file) {
        chmod("/tmp/moho_ipc_token", 0600);  // 只有所有者可读写
        fprintf(token_file, "%s\n", token);
        fclose(token_file);
        printf("✓ IPC 令牌已创建\n");
    }
    fprintf(f, "dofile(\"%s\")\n", IPC_TOOL);
    fclose(f);

    // 启动 Moho(用 open 命令)
    char open_cmd[1024];
    snprintf(open_cmd, sizeof(open_cmd), "open -a Moho --args \"%s\"", wrapper_path);
    system(open_cmd);
    printf("Moho 已启动\n");

    // 等待 socket
    printf("等待 IPC socket...\n");
    for (int i = 0; i < 30; i++) {
        sleep(1);
        if (ipc_check_running()) {
            printf("✓ IPC 服务已启动\n");
            printf("\n发送命令: moho-mate call '<lua>'\n");
            printf("关闭 Moho: moho-mate call 'moho_ipc.quit()'\n");
            return 0;
        }
    }

    fprintf(stderr, "✗ IPC socket 未创建\n");
    return 1;
}

// ========== call ==========

static int cmd_call(int argc, char **argv) {
    char *cmd = NULL;
    char *file = NULL;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-f") == 0 || strcmp(argv[i], "--file") == 0) {
            if (i + 1 < argc) file = argv[++i];
        } else if (argv[i][0] != '-') {
            if (!cmd) cmd = argv[i];
        }
    }

    if (file) {
        if (access(file, R_OK) != 0) {
            fprintf(stderr, "✗ 文件不存在: %s\n", file);
            return 1;
        }
        auto_start_ipc();
        printf("▶ 发送 Lua 文件: %s\n", file);
        return ipc_send_file(file);
    }

    if (!cmd) {
        fprintf(stderr, "用法: moho-mate call '<lua>' 或 -f script.lua\n");
        return 1;
    }

    auto_start_ipc();

    // 判断是否多行
    if (strstr(cmd, "\n")) {
        return ipc_send_multiline(cmd);
    } else {
        return ipc_send(cmd);
    }
}

// ========== quit ==========

static int cmd_quit(void) {
    if (!ipc_check_running()) {
        printf("Moho 未运行\n");
        // 即使 Moho 未运行,也尝试恢复配置
        ipc_config_restore();
        return 0;
    }
    printf("▶ 退出 Moho\n");
    int ret = ipc_send("moho_ipc.quit()");

    // 等待 socket 断开(最多 10 秒)
    for (int i = 0; i < 10; i++) {
        sleep(1);
        if (!ipc_check_running()) {
            printf("✓ Moho 已退出\n");
            break;
        }
    }

    // 恢复原有配置
    ipc_config_restore();

    return ret;
}

// ========== status ==========

static int cmd_status(void) {
    if (ipc_check_running()) {
        printf("✓ IPC 运行中\n");
        printf("  Socket: %s\n", IPC_SOCKET);
    } else {
        printf("✗ IPC 未启动\n");
        printf("先用: moho-mate start\n");
    }
    return 0;
}

// ========== encode ==========

static int cmd_encode(int argc, char **argv) {
    char *input = NULL;
    char *output = NULL;
    int fps = 24;
    int crf = 23;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--fps") == 0 && i + 1 < argc) {
            fps = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--crf") == 0 && i + 1 < argc) {
            crf = atoi(argv[++i]);
        } else if (argv[i][0] != '-') {
            if (!input) input = argv[i];
            else if (!output) output = argv[i];
        }
    }

    if (!input || !output) {
        fprintf(stderr, "用法: moho-mate encode <input> <output> [--fps 24] [--crf 23]\n");
        fprintf(stderr, "格式: .mp4, .gif, .apng\n");
        return 1;
    }

    // 判断输出格式
    int is_gif = (strstr(output, ".gif") != NULL);
    int is_apng = (strstr(output, ".apng") != NULL);

    // APNG 实际输出路径(标准后缀是 .png)
    char actual_output[512];
    strncpy(actual_output, output, sizeof(actual_output) - 1);
    actual_output[sizeof(actual_output) - 1] = '\0';

    if (is_apng) {
        // APNG 标准后缀是 .png
        size_t len = strlen(actual_output);
        if (len > 5 && strcmp(actual_output + len - 5, ".apng") == 0) {
            actual_output[len - 5] = '.';
            actual_output[len - 4] = 'p';
            actual_output[len - 3] = 'n';
            actual_output[len - 2] = 'g';
            actual_output[len - 1] = '\0';
        }
        printf("▶ 编码 APNG(动画 PNG,无损 + 透明)\n");
    } else if (is_gif) {
        printf("▶ 编码 GIF(libavfilter 调色板优化)\n");
    } else {
        printf("▶ 编码 MP4(内置 FFmpeg)\n");
    }
    printf("  输入: %s\n", input);
    if (is_apng && strcmp(output, actual_output) != 0) {
        printf("  输出: %s(APNG 使用标准 PNG 后缀)\n", actual_output);
    } else {
        printf("  输出: %s\n", output);
    }
    printf("  帧率: %d fps\n", fps);

    auto_start_ipc();

    // 发送编码命令(同步等待完成)
    char lua_cmd[CMD_SIZE];
    snprintf(lua_cmd, sizeof(lua_cmd),
        "local ipc = require('moho_ipc')\n"
        "local ok, err = ipc.encode_video(\"%s\", \"%s\", %d, %d, \"mpeg4\")\n"
        "if not ok then\n"
        "  print('✗ 编码启动失败: ' .. tostring(err))\n"
        "  return\n"
        "end\n"
        "-- 等待编码完成\n"
        "local max_wait = 300\n"
        "local waited = 0\n"
        "while waited < max_wait do\n"
        "  local s = ipc.encode_status()\n"
        "  if s.status == 2 then\n"
        "    print('✓ 编码完成: %s')\n"
        "    break\n"
        "  elseif s.status == 3 then\n"
        "    print('✗ 编码失败: ' .. tostring(s.error_msg))\n"
        "    break\n"
        "  end\n"
        "  os.execute('sleep 1')\n"
        "  waited = waited + 1\n"
        "end\n"
        "if waited >= max_wait then\n"
        "  print('✗ 编码超时')\n"
        "end",
        input, output, fps, crf, actual_output);

    return ipc_send_multiline(lua_cmd);
}

// ========== render ==========



static int cmd_render(int argc, char **argv) {
    char *project = NULL;
    char *format = "PNG";
    char *ext = "png";  // 文件扩展名
    char *output = NULL;
    int start_frame = 0;
    int end_frame = 72;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-f") == 0 && i + 1 < argc) {
            format = argv[++i];
            // 根据格式设置扩展名
            if (strcmp(format, "JPEG") == 0 || strcmp(format, "JPG") == 0) {
                ext = "jpg";
            } else if (strcmp(format, "BMP") == 0) {
                ext = "bmp";
            } else if (strcmp(format, "TGA") == 0) {
                ext = "tga";
            } else {
                ext = "png";
            }
        } else if (strcmp(argv[i], "-o") == 0 && i + 1 < argc) {
            output = argv[++i];
        } else if (strcmp(argv[i], "--start") == 0 && i + 1 < argc) {
            start_frame = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--end") == 0 && i + 1 < argc) {
            end_frame = atoi(argv[++i]);
        } else if (argv[i][0] != '-') {
            if (!project) project = argv[i];
        }
    }

    if (!project) {
        fprintf(stderr, "用法: moho-mate render <project.moho> [-f PNG|JPEG|MP4|GIF|APNG] [-o output]\n");
        return 1;
    }

    if (access(project, R_OK) != 0) {
        fprintf(stderr, "✗ 项目不存在: %s\n", project);
        return 1;
    }

    int is_video = (strcmp(format, "MP4") == 0 || strcmp(format, "GIF") == 0 || strcmp(format, "APNG") == 0 || strcmp(format, "QT") == 0);

    if (is_video) {
        const char *format_name = strcmp(format, "APNG") == 0 ? "APNG(动画 PNG)" : format;
        printf("▶ 渲染 + 编码: %s\n", format_name);
    }

    printf("▶ 渲染项目: %s\n", project);
    printf("  格式: %s\n", format);
    printf("  帧范围: %d-%d\n", start_frame, end_frame);
    if (output) printf("  输出: %s\n", output);

    auto_start_ipc();

    // 打开项目
    char open_cmd[512];
    snprintf(open_cmd, sizeof(open_cmd), "moho:FileOpen(\"%s\")", project);
    ipc_send(open_cmd);

    // 渲染命令
    char lua_cmd[CMD_SIZE];
    char output_path[512];

    // 视频格式:确保输出路径有正确后缀
    if (output) {
        snprintf(output_path, sizeof(output_path), "%s", output);
        // 检查是否需要添加后缀
        if (is_video) {
            size_t len = strlen(output_path);
            int has_suffix = 0;
            if (strcmp(format, "GIF") == 0 && len > 4 && strcmp(output_path + len - 4, ".gif") == 0) has_suffix = 1;
            if (strcmp(format, "MP4") == 0 && len > 4 && strcmp(output_path + len - 4, ".mp4") == 0) has_suffix = 1;
            if (strcmp(format, "APNG") == 0 && (len > 5 && strcmp(output_path + len - 5, ".apng") == 0 || len > 4 && strcmp(output_path + len - 4, ".png") == 0)) has_suffix = 1;
            if (strcmp(format, "QT") == 0 && len > 4 && strcmp(output_path + len - 4, ".mov") == 0) has_suffix = 1;

            if (!has_suffix) {
                // 自动添加后缀
                const char *suffix = strcmp(format, "APNG") == 0 ? ".png" :
                                     strcmp(format, "QT") == 0 ? ".mov" :
                                     strcmp(format, "GIF") == 0 ? ".gif" : ".mp4";
                snprintf(output_path + len, sizeof(output_path) - len, "%s", suffix);
                printf("  输出路径已修正: %s\n", output_path);
            }
        }
    } else {
        // 从项目名生成输出名
        char *base = strrchr(project, '/');
        base = base ? base + 1 : project;
        char name[256];
        strncpy(name, base, sizeof(name));
        char *dot = strrchr(name, '.');
        if (dot) *dot = '\0';

        if (is_video) {
            // APNG 输出后缀是 .png,其他用格式名
            const char *ext = strcmp(format, "APNG") == 0 ? "png" : format;
            snprintf(output_path, sizeof(output_path), "/tmp/%s.%s", name, ext);
        } else {
            snprintf(output_path, sizeof(output_path), "/tmp/%s", name);
        }
    }

    // 视频格式需要临时 PNG 目录
    char png_dir[512];
    if (is_video) {
        snprintf(png_dir, sizeof(png_dir), "/tmp/moho_render_frames_%d", getpid());
    } else {
        snprintf(png_dir, sizeof(png_dir), "%s", output_path);
    }

    // 创建输出目录
    char mkdir_cmd[512];
    snprintf(mkdir_cmd, sizeof(mkdir_cmd), "mkdir -p \"%s\"", png_dir);
    system(mkdir_cmd);

    // IPC 渲染 PNG
    snprintf(lua_cmd, sizeof(lua_cmd),
        "local ipc = require('moho_ipc')\n"
        "-- 使用全局 moho 对象(IPC 环境已设置)\n"
        "local moho = _G.moho\n"
        "if not moho then\n"
        "  local helper = MOHO.ScriptInterfaceHelper:new_local()\n"
        "  moho = helper:MohoObject()\n"
        "end\n"
        "local output_dir = \"%s\"\n"
        "for f = %d, %d do\n"
        "  moho:SetCurFrame(f, true)\n"
        "  local frame_path = output_dir .. \"/frame_\" .. string.format(\"%%05d\", f) .. \".%s\"\n"
        "  moho:FileRender(frame_path)\n"
        "end\n"
        "print('✓ 渲染完成: ' .. (%d - %d + 1) .. ' 帧')",
        png_dir, start_frame, end_frame, ext, end_frame, start_frame);

    int ret = ipc_send_multiline(lua_cmd);

    if (ret != 0) {
        fprintf(stderr, "✗ 渲染失败\n");
        return ret;
    }

    // 视频格式:调用 encode 编码
    if (is_video) {
        printf("✓ 序列已保存到: %s\n", png_dir);

        // 根据格式选择编码器
        const char *codec = "mpeg4";
        const char *format_lower = "mp4";
        if (strcmp(format, "GIF") == 0) {
            codec = "gif";
            format_lower = "gif";
        } else if (strcmp(format, "APNG") == 0) {
            codec = "apng";
            format_lower = "png";  // APNG 输出后缀是 .png
        } else if (strcmp(format, "MP4") == 0) {
            codec = "mpeg4";
            format_lower = "mp4";
        }

        printf("▶ 编码 %s: %s\n", format, output_path);

        // Lua 脚本:同步等待编码完成
        char encode_cmd[2048];
        snprintf(encode_cmd, sizeof(encode_cmd),
            "local ipc = require('moho_ipc')\n"
            "local input = \"%s/frame_%%05d.png\"\n"
            "local output = \"%s\"\n"
            "local fps = 24\n"
            "local ok, err = ipc.encode_video(input, output, fps, 23, \"%s\")\n"
            "if not ok then\n"
            "  print('✗ 编码启动失败: ' .. tostring(err))\n"
            "  return\n"
            "end\n"
            "-- 同步等待编码完成(最多 300 秒)\n"
            "local max_wait = 300\n"
            "local waited = 0\n"
            "while waited < max_wait do\n"
            "  local s = ipc.encode_status()\n"
            "  if s.status == 2 then\n"
            "    print('✓ 编码完成: ' .. output)\n"
            "    break\n"
            "  elseif s.status == 3 then\n"
            "    print('✗ 编码失败: ' .. tostring(s.error_msg))\n"
            "    break\n"
            "  end\n"
            "  os.execute('sleep 1')\n"
            "  waited = waited + 1\n"
            "  if waited %% 10 == 0 then\n"
            "    print('  等待 ' .. waited .. ' 秒...')\n"
            "  end\n"
            "end\n"
            "if waited >= max_wait then\n"
            "  print('✗ 编码超时')\n"
            "end",
            png_dir, output_path, codec);

        ret = ipc_send_multiline(encode_cmd);

        // 清理临时 PNG
        printf("▶ 清理临时帧...\n");
        char cleanup_cmd[256];
        snprintf(cleanup_cmd, sizeof(cleanup_cmd), "rm -rf \"%s\"", png_dir);
        system(cleanup_cmd);

        // 检查输出文件是否存在
        if (access(output_path, F_OK) == 0) {
            printf("✓ 视频已保存到: %s\n", output_path);
        } else {
            fprintf(stderr, "✗ 输出文件不存在: %s\n", output_path);
            ret = 1;
        }
    } else {
        printf("✓ 序列已保存到: %s\n", output_path);
    }

    return ret;
}

// ========== draw(IPC 模式)==========

static int cmd_draw(int argc, char **argv) {
    char *shape = argc > 1 ? argv[1] : "circle";

    // 检查支持的形状
    if (strcmp(shape, "circle") != 0 && strcmp(shape, "bunny") != 0 && strcmp(shape, "puppy") != 0) {
        fprintf(stderr, "✗ 未知形状: %s\n", shape);
        fprintf(stderr, "可用形状: circle, bunny, puppy\n");
        return 1;
    }

    printf("▶ 绘制形状: %s\n", shape);
    printf("⚠️ draw 只绘制,不保存。请手动 Cmd+S\n");

    auto_start_ipc();

    // 使用 draw_ipc.lua 脚本(不保存)
    char lua_cmd[256];
    snprintf(lua_cmd, sizeof(lua_cmd),
        "local home = os.getenv('HOME')\n"
        "dofile(home .. '/.openclaw/workspace/skills/moho-mate/scripts/draw_ipc.lua')\n"
        "draw_shape('%s')", shape);

    printf("▶ IPC 绘制中...\n");
    int ret = ipc_send_multiline(lua_cmd);

    if (ret == 0) {
        printf("✓ 已绘制 %s,请手动保存\n", shape);
    } else {
        fprintf(stderr, "✗ 绘制失败\n");
    }

    return ret;
}

// ========== inspect ==========

static int cmd_inspect(int argc, char **argv) {
    char *project = argc > 1 ? argv[1] : NULL;

    if (!project) {
        fprintf(stderr, "用法: moho-mate inspect <project.moho>\n");
        return 1;
    }

    if (access(project, R_OK) != 0) {
        fprintf(stderr, "✗ 项目不存在: %s\n", project);
        return 1;
    }

    printf("=== 项目信息 ===\n");
    printf("  文件: %s\n", project);

    // 解析 .moho 文件(XML 格式)
    FILE *f = fopen(project, "r");
    if (!f) {
        fprintf(stderr, "✗ 无法读取文件\n");
        return 1;
    }

    char line[1024];
    int layer_count = 0;
    int bone_count = 0;
    int mesh_count = 0;
    int start_frame = 0;
    int end_frame = 72;
    int fps = 24;

    while (fgets(line, sizeof(line), f)) {
        // 统计图层
        if (strstr(line, "<layer")) layer_count++;

        // 统计骨骼
        if (strstr(line, "<bone")) bone_count++;

        // 统计 mesh
        if (strstr(line, "<mesh")) mesh_count++;

        // 提取帧范围
        char *start_match = strstr(line, "start_frame=\"");
        if (start_match) {
            start_frame = atoi(start_match + 14);
        }

        char *end_match = strstr(line, "end_frame=\"");
        if (end_match) {
            end_frame = atoi(end_match + 12);
        }

        char *fps_match = strstr(line, "fps=\"");
        if (fps_match) {
            fps = atoi(fps_match + 5);
        }
    }
    fclose(f);

    printf("\n=== 内容统计 ===\n");
    printf("  图层数: %d\n", layer_count);
    printf("  骨骼数: %d\n", bone_count);
    printf("  Mesh数: %d\n", mesh_count);
    printf("\n=== 动画设置 ===\n");
    printf("  帧范围: %d - %d\n", start_frame, end_frame);
    printf("  帧率: %d fps\n", fps);
    printf("  时长: %.2f 秒\n", (float)(end_frame - start_frame + 1) / fps);

    return 0;
}

// ========== config ==========

static int cmd_config(int argc, char **argv) {
    char *action = argc > 1 ? argv[1] : "list";

    if (strcmp(action, "list") == 0) {
        printf("=== Moho 配置目录 ===\n");
        printf("  路径: %s\n\n", MOHO_CONFIG_DIR);

        DIR *dir = opendir(MOHO_CONFIG_DIR);
        if (!dir) {
            fprintf(stderr, "✗ 无法访问配置目录\n");
            return 1;
        }

        struct dirent *entry;
        while ((entry = readdir(dir)) != NULL) {
            if (entry->d_name[0] == '.') continue;

            char path[512];
            snprintf(path, sizeof(path), "%s/%s", MOHO_CONFIG_DIR, entry->d_name);

            struct stat st;
            if (stat(path, &st) == 0) {
                char time_str[64];
                strftime(time_str, sizeof(time_str), "%Y-%m-%d %H:%M", localtime(&st.st_mtime));
                printf("  %s  (%s, %ld bytes)\n", entry->d_name, time_str, st.st_size);
            }
        }
        closedir(dir);

    } else if (strcmp(action, "backup") == 0) {
        printf("▶ 备份 Moho 配置\n");

        char backup_dir[512];
        time_t now = time(NULL);
        struct tm *t = localtime(&now);
        snprintf(backup_dir, sizeof(backup_dir), "/tmp/moho_config_backup_%04d%02d%02d_%02d%02d",
                 t->tm_year + 1900, t->tm_mon + 1, t->tm_mday, t->tm_hour, t->tm_min);

        char cmd[1024];
        snprintf(cmd, sizeof(cmd), "mkdir -p \"%s\" && cp -R \"%s\"/ \"%s\"/",
                 backup_dir, MOHO_CONFIG_DIR, backup_dir);
        system(cmd);

        printf("✓ 已备份到: %s\n", backup_dir);

    } else if (strcmp(action, "restore") == 0) {
        printf("▶ 恢复 Moho 配置\n");

        // 找最新的备份
        char cmd[1024];
        snprintf(cmd, sizeof(cmd), "ls -dt /tmp/moho_config_backup_* 2>/dev/null | head -1");

        FILE *fp = popen(cmd, "r");
        if (!fp) {
            fprintf(stderr, "✗ 无可用备份\n");
            return 1;
        }

        char backup_dir[512];
        if (fgets(backup_dir, sizeof(backup_dir), fp) == NULL) {
            pclose(fp);
            fprintf(stderr, "✗ 无可用备份\n");
            return 1;
        }
        pclose(fp);

        // 去换行
        backup_dir[strcspn(backup_dir, "\n")] = 0;

        printf("  源: %s\n", backup_dir);

        snprintf(cmd, sizeof(cmd), "cp -R \"%s\"/* \"%s\"/", backup_dir, MOHO_CONFIG_DIR);
        system(cmd);

        printf("✓ 已恢复\n");

    } else {
        fprintf(stderr, "用法: moho-mate config list|backup|restore\n");
        return 1;
    }

    return 0;
}

// ========== 主入口 ==========

static void print_usage(const char *prog) {
    printf("moho-mate - Moho 命令行工具\n\n");
    printf("用法:\n");
    printf("  %s start [project.moho] [script.lua]    启动 IPC 服务\n", prog);
    printf("  %s call '<lua>'                          发送 Lua 命令\n", prog);
    printf("  %s call -f script.lua                    发送 Lua 文件\n", prog);
    printf("  %s quit                                  退出 Moho\n", prog);
    printf("  %s status                                IPC 状态\n", prog);
    printf("  %s render project.moho [-f PNG] [-o out] 渲染项目\n", prog);
    printf("  %s encode input output [--fps 24]        编码视频\n", prog);
    printf("  %s draw <shape>                    绘制形状(不保存)\n", prog);
    printf("  %s inspect <project.moho>                查看项目\n", prog);
    printf("  %s config list|backup|restore           配置管理\n", prog);
}

int main(int argc, char **argv) {
    if (argc < 2) {
        print_usage(argv[0]);
        return 1;
    }

    char *cmd = argv[1];

    if (strcmp(cmd, "start") == 0) {
        return cmd_start(argc - 1, argv + 1);
    } else if (strcmp(cmd, "call") == 0) {
        return cmd_call(argc - 1, argv + 1);
    } else if (strcmp(cmd, "quit") == 0) {
        return cmd_quit();
    } else if (strcmp(cmd, "status") == 0) {
        return cmd_status();
    } else if (strcmp(cmd, "encode") == 0) {
        return cmd_encode(argc - 1, argv + 1);
    } else if (strcmp(cmd, "render") == 0) {
        return cmd_render(argc - 1, argv + 1);
    } else if (strcmp(cmd, "draw") == 0) {
        return cmd_draw(argc - 1, argv + 1);
    } else if (strcmp(cmd, "inspect") == 0) {
        return cmd_inspect(argc - 1, argv + 1);
    } else if (strcmp(cmd, "config") == 0) {
        return cmd_config(argc - 1, argv + 1);
    } else if (strcmp(cmd, "--help") == 0 || strcmp(cmd, "-h") == 0) {
        print_usage(argv[0]);
        return 0;
    } else {
        fprintf(stderr, "✗ 未知命令: %s\n", cmd);
        print_usage(argv[0]);
        return 1;
    }
}
// ========== Lua 模块实现 ==========

static lua_State *g_L = NULL;
static char g_response[RESP_SIZE];  // 存储响应
static char g_output_buffer[OUTPUT_SIZE];  // 捕获输出
static size_t g_output_len = 0;
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

// Token 验证函数已移除

// 捕获输出的 print hook
static int capture_print(lua_State *L) {
    int n = lua_gettop(L);
    luaL_Buffer b;
    luaL_buffinit(L, &b);
    for (int i = 1; i <= n; i++) {
        if (i > 1) luaL_addchar(&b, '\t');
        luaL_addstring(&b, luaL_tolstring(L, i, NULL));
        lua_pop(L, 1);
    }
    luaL_pushresult(&b);
    const char *str = lua_tostring(L, -1);

    // 写入输出缓冲
    size_t len = strlen(str);
    if (g_output_len + len + 1 < OUTPUT_SIZE) {
        if (g_output_len > 0) {
            g_output_buffer[g_output_len++] = '\n';
        }
        memcpy(g_output_buffer + g_output_len, str, len);
        g_output_len += len;
        g_output_buffer[g_output_len] = 0;
    }

    // 也输出到原始 print
    lua_getglobal(L, "_original_print");
    if (lua_isfunction(L, -1)) {
        lua_pushvalue(L, -2);  // str
        lua_pcall(L, 1, 0, 0);
    } else {
        lua_pop(L, 1);
    }

    return 0;
}

// 执行命令 (直接在 C 中实现,不依赖 Lua 的 ipc_execute)
static const char* execute_via_helper(const char *cmd) {
    if (g_L == NULL) {
        log_msg("✗ g_L is NULL\n");
        return "error|g_L is NULL";
    }

    g_call_count++;
    g_output_len = 0;
    g_output_buffer[0] = 0;

    // 1. 获取 MOHO.ScriptInterfaceHelper
    lua_getglobal(g_L, "MOHO");
    if (!lua_istable(g_L, -1)) {
        log_msg("✗ MOHO not found\n");
        lua_pop(g_L, 1);
        g_error_count++;
        return "error|MOHO not found";
    }

    lua_getfield(g_L, -1, "ScriptInterfaceHelper");
    if (!lua_istable(g_L, -1)) {
        log_msg("✗ ScriptInterfaceHelper not found\n");
        lua_pop(g_L, 2);
        g_error_count++;
        return "error|ScriptInterfaceHelper not found";
    }

    // 2. 创建 helper 实例: helper = MOHO.ScriptInterfaceHelper:new_local()
    lua_getfield(g_L, -1, "new_local");
    if (!lua_isfunction(g_L, -1)) {
        log_msg("✗ new_local not found\n");
        lua_pop(g_L, 3);
        g_error_count++;
        return "error|new_local not found";
    }

    // 调用 new_local(ScriptInterfaceHelper)
    lua_pushvalue(g_L, -2);  // self = ScriptInterfaceHelper
    if (lua_pcall(g_L, 1, 1, 0) != 0) {
        log_msg("✗ new_local failed: %s\n", lua_tostring(g_L, -1));
        lua_pop(g_L, 3);
        g_error_count++;
        return "error|new_local failed";
    }

    // 栈: MOHO, ScriptInterfaceHelper, helper
    // helper 实例在栈顶

    // 3. 获取 moho 对象: helper:MohoObject()
    lua_getfield(g_L, -1, "MohoObject");
    if (!lua_isfunction(g_L, -1)) {
        log_msg("✗ MohoObject not found\n");
        lua_pop(g_L, 4);
        g_error_count++;
        return "error|MohoObject not found";
    }

    lua_pushvalue(g_L, -2);  // self = helper
    if (lua_pcall(g_L, 1, 1, 0) != 0) {
        log_msg("✗ MohoObject failed: %s\n", lua_tostring(g_L, -1));
        lua_pop(g_L, 4);
        g_error_count++;
        return "error|MohoObject failed";
    }

    // 栈: MOHO, ScriptInterfaceHelper, helper, moho

    // 4. 设置全局 moho
    lua_setglobal(g_L, "moho");
    // 栈: MOHO, ScriptInterfaceHelper, helper

    // 保存 helper 到 registry (用于后续清理)
    lua_setfield(g_L, LUA_REGISTRYINDEX, "_ipc_helper");
    // 栈: MOHO, ScriptInterfaceHelper

    lua_pop(g_L, 2);  // pop MOHO and ScriptInterfaceHelper

    // 5. 清空栈,确保干净状态
    lua_pop(g_L, lua_gettop(g_L));

    // 6. 设置 print hook 捕获输出
    lua_getglobal(g_L, "print");
    lua_setglobal(g_L, "_original_print");
    lua_pushcfunction(g_L, capture_print);
    lua_setglobal(g_L, "print");

    // ⚠️ 闭包方案：创建沙盒环境表
    // 1. 创建环境表（用 metatable 继承 _G）
    lua_newtable(g_L);  // 栈: env
    
    // 2. 设置 metatable（__index = _G）
    lua_newtable(g_L);  // 栈: env, mt
    lua_getglobal(g_L, "_G");  // 栈: env, mt, _G
    lua_setfield(g_L, -2, "__index");  // mt.__index = _G
    lua_setmetatable(g_L, -2);  // env.metatable = mt，栈: env
    
    // 3. 复制 moho
    lua_getglobal(g_L, "moho");
    lua_setfield(g_L, -2, "moho");
    
    // 4. 复制 print
    lua_getglobal(g_L, "print");
    lua_setfield(g_L, -2, "print");
    
    // 5. 复制 moho_ipc（从 registry）
    lua_getfield(g_L, LUA_REGISTRYINDEX, "_ipc_module");
    if (lua_istable(g_L, -1)) {
        lua_setfield(g_L, -2, "moho_ipc");
        log_msg("✓ moho_ipc 已加载到环境表\n");
    } else {
        lua_pop(g_L, 1);
        log_msg("⚠️ _ipc_module not found\n");
    }
    
    // 环境表现在在栈顶（位置 -1）
    
    // 7. 编译命令为函数
    log_msg("编译命令...\n");
    int load_ret = luaL_loadstring(g_L, cmd);  // 栈: env, func
    
    if (load_ret != 0) {
        log_msg("✗ 编译失败: %s\n", lua_tostring(g_L, -1));
        snprintf(g_response, RESP_SIZE, "error|%s", lua_tostring(g_L, -1));
        lua_pop(g_L, 2);  // 清空环境和错误
        g_error_count++;
        return g_response;
    }
    
    // 8. 设置函数的 _ENV upvalue
    log_msg("设置环境表作为 _ENV...\n");
    // 栈: env, func
    lua_pushvalue(g_L, -2);  // 复制 env 到栈顶，栈: env, func, env_copy
    const char *uvname = lua_setupvalue(g_L, -2, 1);  // 设置 func 的第一个 upvalue
    log_msg("setupvalue 返回: %s\n", uvname ? uvname : "(null)");
    // 栈: env, func
    
    // 9. 执行函数
    log_msg("执行前栈大小: %d\n", lua_gettop(g_L));
    int ret = lua_pcall(g_L, 0, 0, 0);
    log_msg("执行后栈大小: %d\n", lua_gettop(g_L));

    // 8. 保存返回值
    char retval[512] = "";
    int nresults = lua_gettop(g_L);
    if (nresults > 0) {
        const char *s = lua_tostring(g_L, -1);
        if (s) {
            strncpy(retval, s, sizeof(retval) - 1);
        }
    }

    // 9. 恢复 print + 清理环境表
    lua_pop(g_L, lua_gettop(g_L));  // 清空栈(包括环境表)
    lua_getglobal(g_L, "_original_print");
    lua_setglobal(g_L, "print");

    // ⚠️ 不需要删除全局 moho_ipc(从未设置全局)

    if (ret != 0) {
        log_msg("✗ 执行错误: %s\n", lua_tostring(g_L, -1));
        const char *err = lua_tostring(g_L, -1);
        snprintf(g_response, RESP_SIZE, "error|%s", err ? err : "unknown");
        g_error_count++;
        return g_response;
    }

    // 10. 返回结果
    if (retval[0]) {
        snprintf(g_response, RESP_SIZE, "ok|%s", retval);
    } else if (g_output_len > 0) {
        snprintf(g_response, RESP_SIZE, "ok|%s", g_output_buffer);
    } else {
        strcpy(g_response, "ok|(无输出)");
    }

    log_msg("✓ 执行成功 (calls=%d, errors=%d)\n", g_call_count, g_error_count);
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
        // 去除尾部换行
        while (n > 0 && (buf[n-1] == '\n' || buf[n-1] == '\r')) {
            buf[--n] = 0;
        }
        log_msg("收到命令 (%zd bytes): %.60s...\n", n, buf);

        // 执行命令
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
    
    // ⚠️ 验证启动令牌（防止其他脚本调用）
    FILE *token_file = fopen("/tmp/moho_ipc_token", "r");
    if (!token_file) {
        log_msg("✗ 启动拒绝：令牌文件不存在\n");
        lua_pushboolean(L, 0);
        lua_pushstring(L, "启动拒绝：令牌文件不存在");
        return 2;
    }
    
    char expected_token[128] = "";
    if (fgets(expected_token, sizeof(expected_token), token_file)) {
        // 去除换行
        int len = strlen(expected_token);
        if (len > 0 && expected_token[len-1] == '\n') expected_token[len-1] = 0;
    }
    fclose(token_file);
    
    // 检查 Lua 中的令牌
    lua_getglobal(L, "IPC_START_TOKEN");
    const char *token = lua_tostring(L, -1);
    lua_pop(L, 1);
    
    if (!token || strcmp(token, expected_token) != 0) {
        log_msg("✗ 启动拒绝：令牌验证失败\n");
        log_msg("  期望: %s\n", expected_token);
        log_msg("  收到: %s\n", token ? token : "(null)");
        lua_pushboolean(L, 0);
        lua_pushstring(L, "启动拒绝：令牌验证失败");
        return 2;
    }
    
    log_msg("✓ 令牌验证通过\n");

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

// Lua API: check() - 兼容 LayerScript 版 API(返回 nil)
static int l_check(lua_State *L) {
    lua_pushnil(L);
    return 1;
}

// Lua API: poll() - 兼容旧 API
static int l_poll(lua_State *L) {
    lua_pushinteger(L, 0);
    return 1;
}

// ========== Encode API (FFmpeg) ==========

static char g_encode_input[512] = {0};
static char g_encode_output[512] = {0};
static int g_encode_fps = 24;
static int g_encode_crf = 23;
static char g_encode_codec[32] = "mpeg4";
static volatile int g_encode_status = 0;  // 0=idle, 1=running, 2=success, 3=error
static volatile float g_encode_progress = 0.0f;
static char g_encode_error[256] = {0};
static pthread_t g_encode_thread = 0;

// 前向声明
static void* encode_apng_thread(void *arg);
static void* encode_gif_thread(void *arg);
static void* encode_mp4_thread(void *arg);

// ========== Playback 状态 ==========
static volatile int g_play_status = 0;  // 0=stopped, 1=playing, 2=paused
static volatile int g_play_current_frame = 0;
static volatile int g_play_start_frame = 0;
static volatile int g_play_end_frame = 72;
static volatile int g_play_fps = 24;
static CFRunLoopTimerRef g_play_timer = NULL;
static CFRunLoopSourceRef g_play_source = NULL;

// APNG 编码(使用 FFmpeg APNG 编码器)
// 输出标准 .png 后缀(APNG 是 PNG 的动画扩展)
static void* encode_apng_thread(void *arg) {
    AVFormatContext *fmt_ctx = NULL;
    AVCodecContext *codec_ctx = NULL;
    AVStream *stream = NULL;
    const AVCodec *codec = NULL;
    AVFrame *frame = NULL, *png_frame = NULL;
    AVPacket *pkt = NULL, *png_pkt = NULL;
    AVFormatContext *png_fmt = NULL;
    AVCodecContext *png_codec = NULL;
    const AVCodec *png_decoder = NULL;

    int ret, frame_count = 0;
    char png_path[512];
    char output_path[512];
    int input_width = 0, input_height = 0;

    // APNG 输出标准 .png 后缀
    strncpy(output_path, g_encode_output, sizeof(output_path) - 1);
    size_t output_len = strlen(output_path);
    if (output_len > 5 && strcmp(output_path + output_len - 5, ".apng") == 0) {
        // 将 .apng 改为 .png
        output_path[output_len - 5] = '.';
        output_path[output_len - 4] = 'p';
        output_path[output_len - 3] = 'n';
        output_path[output_len - 2] = 'g';
        output_path[output_len - 1] = '\0';
        log_msg("[encode] APNG 输出改为标准后缀: %s\n", output_path);
    }

    log_msg("[encode] APNG 开始编码: %s -> %s\n", g_encode_input, output_path);

    // === 第一步:读取第一帧获取分辨率 ===
    snprintf(png_path, sizeof(png_path), g_encode_input, 0);
    if (access(png_path, R_OK) != 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "找不到第一帧: %s", png_path);
        g_encode_status = 3;
        return NULL;
    }

    png_fmt = NULL;
    ret = avformat_open_input(&png_fmt, png_path, NULL, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法读取第一帧");
        g_encode_status = 3;
        return NULL;
    }

    avformat_find_stream_info(png_fmt, NULL);

    int video_stream = -1;
    for (unsigned int i = 0; i < png_fmt->nb_streams; i++) {
        if (png_fmt->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
            video_stream = i;
            break;
        }
    }

    if (video_stream >= 0) {
        AVCodecParameters *png_par = png_fmt->streams[video_stream]->codecpar;
        input_width = png_par->width;
        input_height = png_par->height;
        log_msg("[encode] APNG 输入分辨率: %dx%d\n", input_width, input_height);
    }
    avformat_close_input(&png_fmt);

    if (input_width <= 0 || input_height <= 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法检测输入分辨率");
        g_encode_status = 3;
        return NULL;
    }

    // === 第二步:创建 APNG 编码器 ===
    codec = avcodec_find_encoder(AV_CODEC_ID_APNG);
    if (!codec) {
        snprintf(g_encode_error, sizeof(g_encode_error), "找不到 APNG 编码器");
        g_encode_status = 3;
        return NULL;
    }

    ret = avformat_alloc_output_context2(&fmt_ctx, NULL, "apng", output_path);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建输出上下文");
        g_encode_status = 3;
        return NULL;
    }

    stream = avformat_new_stream(fmt_ctx, NULL);
    if (!stream) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建流");
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    codec_ctx = avcodec_alloc_context3(codec);
    if (!codec_ctx) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建编码器上下文");
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    codec_ctx->width = input_width;
    codec_ctx->height = input_height;
    codec_ctx->time_base = (AVRational){1, g_encode_fps};
    codec_ctx->framerate = (AVRational){g_encode_fps, 1};
    codec_ctx->pix_fmt = AV_PIX_FMT_RGBA;  // APNG 使用 RGBA

    // APNG 特定设置
    // plays: 0 = 无限循环, 1+ = 播放次数
    av_opt_set_int(codec_ctx, "plays", 0, 0);  // 无限循环

    if (fmt_ctx->oformat->flags & AVFMT_GLOBALHEADER) {
        codec_ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    }

    ret = avcodec_open2(codec_ctx, codec, NULL);
    if (ret < 0) {
        char errbuf[128];
        av_strerror(ret, errbuf, sizeof(errbuf));
        snprintf(g_encode_error, sizeof(g_encode_error), "无法打开 APNG 编码器: %s", errbuf);
        log_msg("[encode] APNG 编码器打开失败: %s\n", g_encode_error);
        avcodec_free_context(&codec_ctx);
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    avcodec_parameters_from_context(stream->codecpar, codec_ctx);

    if (!(fmt_ctx->oformat->flags & AVFMT_NOFILE)) {
        ret = avio_open(&fmt_ctx->pb, output_path, AVIO_FLAG_WRITE);
        if (ret < 0) {
            snprintf(g_encode_error, sizeof(g_encode_error), "无法打开输出文件");
            avcodec_free_context(&codec_ctx);
            avformat_free_context(fmt_ctx);
            g_encode_status = 3;
            return NULL;
        }
    }

    ret = avformat_write_header(fmt_ctx, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法写 APNG 文件头");
        avcodec_free_context(&codec_ctx);
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    // === 第三步:读取 PNG 序列并编码 ===
    pkt = av_packet_alloc();
    frame = av_frame_alloc();  // 输出帧
    png_frame = av_frame_alloc();  // 输入帧

    // 创建帧缓冲(APNG 需要 RGBA)
    frame->format = AV_PIX_FMT_RGBA;
    frame->width = input_width;
    frame->height = input_height;
    av_frame_get_buffer(frame, 0);

    // 创建图像转换上下文(PNG 可能不是 RGBA)
    struct SwsContext *sws_ctx = NULL;

    int input_frame = 0;

    while (1) {
        snprintf(png_path, sizeof(png_path), g_encode_input, input_frame);

        if (access(png_path, R_OK) != 0) {
            break;  // 没有更多帧
        }

        png_fmt = NULL;
        ret = avformat_open_input(&png_fmt, png_path, NULL, NULL);
        if (ret < 0) {
            log_msg("[encode] 无法读取: %s\n", png_path);
            input_frame++;
            continue;
        }

        avformat_find_stream_info(png_fmt, NULL);

        video_stream = -1;
        for (unsigned int i = 0; i < png_fmt->nb_streams; i++) {
            if (png_fmt->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
                video_stream = i;
                break;
            }
        }

        if (video_stream >= 0) {
            AVCodecParameters *png_par = png_fmt->streams[video_stream]->codecpar;
            png_decoder = avcodec_find_decoder(png_par->codec_id);
            png_codec = avcodec_alloc_context3(png_decoder);
            avcodec_parameters_to_context(png_codec, png_par);
            avcodec_open2(png_codec, png_decoder, NULL);

            png_pkt = av_packet_alloc();

            while (av_read_frame(png_fmt, png_pkt) >= 0) {
                if (png_pkt->stream_index == video_stream) {
                    ret = avcodec_send_packet(png_codec, png_pkt);
                    if (ret >= 0) {
                        ret = avcodec_receive_frame(png_codec, png_frame);
                        if (ret >= 0) {
                            // 创建转换上下文(根据实际输入格式)
                            if (!sws_ctx) {
                                sws_ctx = sws_getContext(
                                    png_frame->width, png_frame->height, png_frame->format,
                                    input_width, input_height, AV_PIX_FMT_RGBA,
                                    SWS_BILINEAR, NULL, NULL, NULL
                                );
                            }

                            // 确保帧缓冲可写(关键!)
                            ret = av_frame_make_writable(frame);
                            if (ret < 0) {
                                log_msg("[encode] 无法使帧可写\n");
                                continue;
                            }

                            // 转换到 RGBA
                            sws_scale(sws_ctx, png_frame->data, png_frame->linesize,
                                     0, png_frame->height, frame->data, frame->linesize);

                            // 设置帧属性
                            frame->pts = input_frame;

                            // 编码为 APNG 帧
                            ret = avcodec_send_frame(codec_ctx, frame);
                            if (ret >= 0) {
                                while (avcodec_receive_packet(codec_ctx, pkt) >= 0) {
                                    av_packet_rescale_ts(pkt, codec_ctx->time_base, stream->time_base);
                                    pkt->stream_index = stream->index;
                                    av_interleaved_write_frame(fmt_ctx, pkt);
                                }
                            }
                        }
                    }
                }
                av_packet_unref(png_pkt);
            }

            av_packet_free(&png_pkt);
            avcodec_free_context(&png_codec);
        }

        avformat_close_input(&png_fmt);
        input_frame++;
        frame_count++;
        g_encode_progress = (float)frame_count / (frame_count + 50.0f);
    }

    // 清理转换上下文
    if (sws_ctx) sws_freeContext(sws_ctx);

    // 刷新编码器
    avcodec_send_frame(codec_ctx, NULL);
    while (avcodec_receive_packet(codec_ctx, pkt) >= 0) {
        av_packet_rescale_ts(pkt, codec_ctx->time_base, stream->time_base);
        pkt->stream_index = stream->index;
        av_interleaved_write_frame(fmt_ctx, pkt);
    }

    av_write_trailer(fmt_ctx);

    log_msg("[encode] APNG 编码完成: %d 帧 -> %s\n", frame_count, output_path);
    g_encode_status = 2;
    g_encode_progress = 1.0f;

    // 清理
    if (pkt) av_packet_free(&pkt);
    if (frame) av_frame_free(&frame);
    if (png_frame) av_frame_free(&png_frame);
    if (codec_ctx) avcodec_free_context(&codec_ctx);
    if (fmt_ctx) {
        if (!(fmt_ctx->oformat->flags & AVFMT_NOFILE)) {
            avio_closep(&fmt_ctx->pb);
        }
        avformat_free_context(fmt_ctx);
    }

    if (g_encode_status == 3) {
        log_msg("[encode] APNG 编码失败: %s\n", g_encode_error);
    }

    return NULL;
}

// GIF 编码(使用 Moho 内置 FFmpeg + libavfilter palettegen/paletteuse)
static void* encode_gif_thread(void *arg) {
    AVFormatContext *fmt_ctx = NULL;
    AVCodecContext *codec_ctx = NULL;
    AVStream *stream = NULL;
    const AVCodec *codec = NULL;
    AVFrame *frame = NULL, *png_frame = NULL;
    AVPacket *pkt = NULL, *png_pkt = NULL;
    AVFormatContext *png_fmt = NULL;
    AVCodecContext *png_codec = NULL;
    const AVCodec *png_decoder = NULL;

    // libavfilter
    AVFilterGraph *filter_graph = NULL;
    AVFilterContext *buffersrc_ctx = NULL;
    AVFilterContext *buffersink_ctx = NULL;
    AVFilterInOut *outputs = NULL;
    AVFilterInOut *inputs = NULL;

    int ret, frame_count = 0;
    char png_path[512];
    int input_width = 0, input_height = 0;

    log_msg("[encode] GIF 开始编码: %s -> %s\n", g_encode_input, g_encode_output);

    // === 第一步:读取第一帧获取分辨率 ===
    snprintf(png_path, sizeof(png_path), g_encode_input, 0);
    if (access(png_path, R_OK) != 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "找不到第一帧: %s", png_path);
        g_encode_status = 3;
        return NULL;
    }

    png_fmt = NULL;
    ret = avformat_open_input(&png_fmt, png_path, NULL, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法读取第一帧");
        g_encode_status = 3;
        return NULL;
    }

    avformat_find_stream_info(png_fmt, NULL);

    int video_stream = -1;
    for (unsigned int i = 0; i < png_fmt->nb_streams; i++) {
        if (png_fmt->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
            video_stream = i;
            break;
        }
    }

    if (video_stream >= 0) {
        AVCodecParameters *png_par = png_fmt->streams[video_stream]->codecpar;
        input_width = png_par->width;
        input_height = png_par->height;
        log_msg("[encode] GIF 输入分辨率: %dx%d\n", input_width, input_height);
    }
    avformat_close_input(&png_fmt);

    if (input_width <= 0 || input_height <= 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法检测输入分辨率");
        g_encode_status = 3;
        return NULL;
    }

    // === 第二步:创建 GIF 编码器 ===
    codec = avcodec_find_encoder(AV_CODEC_ID_GIF);
    if (!codec) {
        snprintf(g_encode_error, sizeof(g_encode_error), "找不到 GIF 编码器");
        g_encode_status = 3;
        return NULL;
    }

    ret = avformat_alloc_output_context2(&fmt_ctx, NULL, NULL, g_encode_output);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建输出上下文");
        g_encode_status = 3;
        return NULL;
    }

    stream = avformat_new_stream(fmt_ctx, NULL);
    if (!stream) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建流");
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    codec_ctx = avcodec_alloc_context3(codec);
    if (!codec_ctx) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建编码器上下文");
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    codec_ctx->width = input_width;
    codec_ctx->height = input_height;
    codec_ctx->time_base = (AVRational){1, g_encode_fps};
    codec_ctx->framerate = (AVRational){g_encode_fps, 1};
    codec_ctx->pix_fmt = AV_PIX_FMT_PAL8;  // GIF 使用调色板格式

    if (fmt_ctx->oformat->flags & AVFMT_GLOBALHEADER) {
        codec_ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    }

    ret = avcodec_open2(codec_ctx, codec, NULL);
    if (ret < 0) {
        char errbuf[128];
        av_strerror(ret, errbuf, sizeof(errbuf));
        snprintf(g_encode_error, sizeof(g_encode_error), "无法打开 GIF 编码器: %s", errbuf);
        log_msg("[encode] GIF 编码器打开失败: %s\n", g_encode_error);
        avcodec_free_context(&codec_ctx);
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    avcodec_parameters_from_context(stream->codecpar, codec_ctx);

    if (!(fmt_ctx->oformat->flags & AVFMT_NOFILE)) {
        ret = avio_open(&fmt_ctx->pb, g_encode_output, AVIO_FLAG_WRITE);
        if (ret < 0) {
            snprintf(g_encode_error, sizeof(g_encode_error), "无法打开输出文件");
            avcodec_free_context(&codec_ctx);
            avformat_free_context(fmt_ctx);
            g_encode_status = 3;
            return NULL;
        }
    }

    ret = avformat_write_header(fmt_ctx, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法写 GIF 文件头");
        avcodec_free_context(&codec_ctx);
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    // === 第三步:创建 libavfilter 管道 ===
    filter_graph = avfilter_graph_alloc();
    if (!filter_graph) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建滤镜图");
        goto gif_cleanup;
    }

    // buffersrc: 输入 RGBA 帧
    const AVFilter *buffersrc = avfilter_get_by_name("buffer");
    char args[512];
    snprintf(args, sizeof(args), "video_size=%dx%d:pix_fmt=%d:time_base=%d/%d",
             input_width, input_height, AV_PIX_FMT_RGBA, 1, g_encode_fps);
    ret = avfilter_graph_create_filter(&buffersrc_ctx, buffersrc, "in",
                                        args, NULL, filter_graph);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建 buffersrc");
        goto gif_cleanup;
    }

    // buffersink: 输出 PAL8 帧
    const AVFilter *buffersink = avfilter_get_by_name("buffersink");
    ret = avfilter_graph_create_filter(&buffersink_ctx, buffersink, "out",
                                        NULL, NULL, filter_graph);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建 buffersink");
        goto gif_cleanup;
    }

    // 设置输出像素格式
    enum AVPixelFormat pix_fmts[] = { AV_PIX_FMT_PAL8, AV_PIX_FMT_NONE };
    ret = av_opt_set_int_list(buffersink_ctx, "pix_fmts", pix_fmts,
                              AV_PIX_FMT_NONE, AV_OPT_SEARCH_CHILDREN);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法设置输出格式");
        goto gif_cleanup;
    }

    // 使用 avfilter_graph_parse 解析滤镜链
    outputs = avfilter_inout_alloc();
    inputs = avfilter_inout_alloc();

    outputs->name = av_strdup("in");
    outputs->filter_ctx = buffersrc_ctx;
    outputs->pad_idx = 0;
    outputs->next = NULL;

    inputs->name = av_strdup("out");
    inputs->filter_ctx = buffersink_ctx;
    inputs->pad_idx = 0;
    inputs->next = NULL;

    // 滤镜链: format=rgb24,split[s0][s1];[s0]palettegen=stats_mode=full[p];[s1][p]paletteuse=dither=bayer
    char filter_desc[512];
    snprintf(filter_desc, sizeof(filter_desc),
             "format=rgb24,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5");

    ret = avfilter_graph_parse_ptr(filter_graph, filter_desc, &inputs, &outputs, NULL);
    if (ret < 0) {
        char errbuf[128];
        av_strerror(ret, errbuf, sizeof(errbuf));
        snprintf(g_encode_error, sizeof(g_encode_error), "无法解析滤镜链: %s", errbuf);
        log_msg("[encode] 滤镜链解析失败: %s (desc=%s)\n", g_encode_error, filter_desc);
        avfilter_inout_free(&inputs);
        avfilter_inout_free(&outputs);
        goto gif_cleanup;
    }

    ret = avfilter_graph_config(filter_graph, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法配置滤镜图");
        avfilter_inout_free(&inputs);
        avfilter_inout_free(&outputs);
        goto gif_cleanup;
    }

    avfilter_inout_free(&inputs);
    avfilter_inout_free(&outputs);

    log_msg("[encode] GIF 滤镜管道已创建: %s\n", filter_desc);

    // === 第四步:读取 PNG 序列并推入滤镜管道 ===
    pkt = av_packet_alloc();
    frame = av_frame_alloc();   // 输出帧(PAL8)
    png_frame = av_frame_alloc();  // 输入帧(RGBA)

    int input_frame = 0;

    // 第一阶段:将所有帧推入滤镜管道
    while (1) {
        snprintf(png_path, sizeof(png_path), g_encode_input, input_frame);

        if (access(png_path, R_OK) != 0) {
            break;
        }

        png_fmt = NULL;
        ret = avformat_open_input(&png_fmt, png_path, NULL, NULL);
        if (ret < 0) {
            log_msg("[encode] 无法读取: %s\n", png_path);
            input_frame++;
            continue;
        }

        avformat_find_stream_info(png_fmt, NULL);

        video_stream = -1;
        for (unsigned int i = 0; i < png_fmt->nb_streams; i++) {
            if (png_fmt->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
                video_stream = i;
                break;
            }
        }

        if (video_stream >= 0) {
            AVCodecParameters *png_par = png_fmt->streams[video_stream]->codecpar;
            png_decoder = avcodec_find_decoder(png_par->codec_id);
            png_codec = avcodec_alloc_context3(png_decoder);
            avcodec_parameters_to_context(png_codec, png_par);
            avcodec_open2(png_codec, png_decoder, NULL);

            png_pkt = av_packet_alloc();

            while (av_read_frame(png_fmt, png_pkt) >= 0) {
                if (png_pkt->stream_index == video_stream) {
                    ret = avcodec_send_packet(png_codec, png_pkt);
                    if (ret >= 0) {
                        ret = avcodec_receive_frame(png_codec, png_frame);
                        if (ret >= 0) {
                            // 推入滤镜管道
                            png_frame->pts = input_frame;
                            ret = av_buffersrc_add_frame_flags(buffersrc_ctx, png_frame, 0);
                            if (ret < 0) {
                                log_msg("[encode] 无法推入滤镜: frame %d\n", input_frame);
                            }
                        }
                    }
                }
                av_packet_unref(png_pkt);
            }

            av_packet_free(&png_pkt);
            avcodec_free_context(&png_codec);
        }

        avformat_close_input(&png_fmt);
        input_frame++;
        g_encode_progress = (float)input_frame / (input_frame + 100.0f) * 0.5f;  // 前50%进度
    }

    int total_frames = input_frame;
    log_msg("[encode] 共读取 %d 帧,开始生成调色板...\n", total_frames);

    // 刷新滤镜管道(让 palettegen 生成调色板)
    ret = av_buffersrc_add_frame_flags(buffersrc_ctx, NULL, AV_BUFFERSRC_FLAG_PUSH);
    if (ret < 0) {
        log_msg("[encode] 无法刷新滤镜管道\n");
    }

    // 第二阶段:从滤镜管道拉取处理后的帧并编码
    frame_count = 0;
    while ((ret = av_buffersink_get_frame(buffersink_ctx, frame)) >= 0) {
        frame->pts = frame_count;

        // 编码
        ret = avcodec_send_frame(codec_ctx, frame);
        if (ret >= 0) {
            while (avcodec_receive_packet(codec_ctx, pkt) >= 0) {
                av_packet_rescale_ts(pkt, codec_ctx->time_base, stream->time_base);
                pkt->stream_index = stream->index;
                av_interleaved_write_frame(fmt_ctx, pkt);
            }
        }
        frame_count++;
        av_frame_unref(frame);
        g_encode_progress = 0.5f + (float)frame_count / total_frames * 0.5f;  // 后50%进度
    }

    // 刷新编码器
    avcodec_send_frame(codec_ctx, NULL);
    while (avcodec_receive_packet(codec_ctx, pkt) >= 0) {
        av_packet_rescale_ts(pkt, codec_ctx->time_base, stream->time_base);
        pkt->stream_index = stream->index;
        av_interleaved_write_frame(fmt_ctx, pkt);
    }

    av_write_trailer(fmt_ctx);

    log_msg("[encode] GIF 编码完成: %d 帧 -> %s\n", frame_count, g_encode_output);
    g_encode_status = 2;
    g_encode_progress = 1.0f;

gif_cleanup:
    if (filter_graph) avfilter_graph_free(&filter_graph);
    if (pkt) av_packet_free(&pkt);
    if (frame) av_frame_free(&frame);
    if (png_frame) av_frame_free(&png_frame);
    if (codec_ctx) avcodec_free_context(&codec_ctx);
    if (fmt_ctx) {
        if (!(fmt_ctx->oformat->flags & AVFMT_NOFILE)) {
            avio_closep(&fmt_ctx->pb);
        }
        avformat_free_context(fmt_ctx);
    }

    if (g_encode_status == 3) {
        log_msg("[encode] GIF 编码失败: %s\n", g_encode_error);
    }

    return NULL;
}

// MP4 编码(使用 FFmpeg 库)
static void* encode_mp4_thread(void *arg) {
    AVFormatContext *fmt_ctx = NULL;
    AVCodecContext *codec_ctx = NULL;
    AVStream *stream = NULL;
    const AVCodec *codec = NULL;
    AVFrame *frame = NULL, *png_frame = NULL;
    AVPacket *pkt = NULL, *png_pkt = NULL;
    struct SwsContext *sws_ctx = NULL;
    AVFormatContext *png_fmt = NULL;
    AVCodecContext *png_codec = NULL;
    const AVCodec *png_decoder = NULL;

    int ret, frame_count = 0;
    char png_path[512];
    int input_width = 0, input_height = 0;
    enum AVPixelFormat input_pix_fmt = AV_PIX_FMT_RGBA;

    // 检测输出格式(根据扩展名)
    int is_gif = 0;
    size_t output_len = strlen(g_encode_output);
    if (output_len > 4 && strcmp(g_encode_output + output_len - 4, ".gif") == 0) {
        is_gif = 1;
    }

    log_msg("[encode] 开始编码: %s -> %s (格式: %s)\n",
            g_encode_input, g_encode_output, is_gif ? "GIF" : "MP4");

    // === 第一步:读取第一帧 PNG 获取分辨率 ===
    snprintf(png_path, sizeof(png_path), g_encode_input, 0);
    if (access(png_path, R_OK) != 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "找不到第一帧: %s", png_path);
        g_encode_status = 3;
        return NULL;
    }

    ret = avformat_open_input(&png_fmt, png_path, NULL, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法读取第一帧");
        g_encode_status = 3;
        return NULL;
    }

    avformat_find_stream_info(png_fmt, NULL);

    int video_stream = -1;
    for (unsigned int i = 0; i < png_fmt->nb_streams; i++) {
        if (png_fmt->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
            video_stream = i;
            break;
        }
    }

    if (video_stream >= 0) {
        AVCodecParameters *png_par = png_fmt->streams[video_stream]->codecpar;
        input_width = png_par->width;
        input_height = png_par->height;
        input_pix_fmt = AV_PIX_FMT_RGBA;
        log_msg("[encode] 输入分辨率: %dx%d\n", input_width, input_height);
    }
    avformat_close_input(&png_fmt);

    if (input_width <= 0 || input_height <= 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法检测输入分辨率");
        g_encode_status = 3;
        return NULL;
    }

    // === 第二步:创建输出编码器 ===
    enum AVCodecID codec_id = is_gif ? AV_CODEC_ID_GIF : AV_CODEC_ID_MPEG4;
    enum AVPixelFormat output_pix_fmt = is_gif ? AV_PIX_FMT_RGB8 : AV_PIX_FMT_YUV420P;

    codec = avcodec_find_encoder(codec_id);
    if (!codec) {
        snprintf(g_encode_error, sizeof(g_encode_error), "找不到编码器: %s",
                 is_gif ? "GIF" : "MPEG4");
        g_encode_status = 3;
        return NULL;
    }

    // 创建输出上下文
    ret = avformat_alloc_output_context2(&fmt_ctx, NULL, NULL, g_encode_output);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建输出上下文");
        g_encode_status = 3;
        return NULL;
    }

    stream = avformat_new_stream(fmt_ctx, NULL);
    if (!stream) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建流");
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    codec_ctx = avcodec_alloc_context3(codec);
    if (!codec_ctx) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法创建编码器上下文");
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    // 设置编码参数(使用实际输入分辨率)
    codec_ctx->width = input_width;
    codec_ctx->height = input_height;
    codec_ctx->time_base = (AVRational){1, g_encode_fps};
    codec_ctx->framerate = (AVRational){g_encode_fps, 1};
    codec_ctx->pix_fmt = output_pix_fmt;

    if (!is_gif) {
        codec_ctx->bit_rate = 400000;
    }

    if (fmt_ctx->oformat->flags & AVFMT_GLOBALHEADER) {
        codec_ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    }

    ret = avcodec_open2(codec_ctx, codec, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法打开编码器");
        avcodec_free_context(&codec_ctx);
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    avcodec_parameters_from_context(stream->codecpar, codec_ctx);

    if (!(fmt_ctx->oformat->flags & AVFMT_NOFILE)) {
        ret = avio_open(&fmt_ctx->pb, g_encode_output, AVIO_FLAG_WRITE);
        if (ret < 0) {
            snprintf(g_encode_error, sizeof(g_encode_error), "无法打开输出文件");
            avcodec_free_context(&codec_ctx);
            avformat_free_context(fmt_ctx);
            g_encode_status = 3;
            return NULL;
        }
    }

    ret = avformat_write_header(fmt_ctx, NULL);
    if (ret < 0) {
        snprintf(g_encode_error, sizeof(g_encode_error), "无法写文件头");
        avcodec_free_context(&codec_ctx);
        avformat_free_context(fmt_ctx);
        g_encode_status = 3;
        return NULL;
    }

    // 创建帧缓冲
    frame = av_frame_alloc();
    frame->format = codec_ctx->pix_fmt;
    frame->width = codec_ctx->width;
    frame->height = codec_ctx->height;
    av_frame_get_buffer(frame, 0);

    pkt = av_packet_alloc();

    // 创建图像转换上下文
    sws_ctx = sws_getContext(
        input_width, input_height, input_pix_fmt,
        codec_ctx->width, codec_ctx->height, codec_ctx->pix_fmt,
        SWS_BILINEAR, NULL, NULL, NULL
    );

    // === 第三步:读取 PNG 序列并编码 ===
    int input_frame = 0;

    while (1) {
        snprintf(png_path, sizeof(png_path), g_encode_input, input_frame);

        if (access(png_path, R_OK) != 0) {
            break;  // 没有更多帧了
        }

        // 用 FFmpeg 读取 PNG
        png_fmt = NULL;
        ret = avformat_open_input(&png_fmt, png_path, NULL, NULL);
        if (ret < 0) {
            log_msg("[encode] 无法读取: %s\n", png_path);
            input_frame++;
            continue;
        }

        avformat_find_stream_info(png_fmt, NULL);

        int video_stream = -1;
        for (unsigned int i = 0; i < png_fmt->nb_streams; i++) {
            if (png_fmt->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
                video_stream = i;
                break;
            }
        }

        if (video_stream >= 0) {
            AVCodecParameters *png_par = png_fmt->streams[video_stream]->codecpar;
            png_decoder = avcodec_find_decoder(png_par->codec_id);
            png_codec = avcodec_alloc_context3(png_decoder);
            avcodec_parameters_to_context(png_codec, png_par);
            avcodec_open2(png_codec, png_decoder, NULL);

            png_frame = av_frame_alloc();
            png_pkt = av_packet_alloc();

            while (av_read_frame(png_fmt, png_pkt) >= 0) {
                if (png_pkt->stream_index == video_stream) {
                    ret = avcodec_send_packet(png_codec, png_pkt);
                    if (ret >= 0) {
                        ret = avcodec_receive_frame(png_codec, png_frame);
                        if (ret >= 0) {
                            // 转换 RGBA -> YUV420P
                            sws_scale(sws_ctx, png_frame->data, png_frame->linesize,
                                     0, png_frame->height, frame->data, frame->linesize);

                            frame->pts = frame_count;

                            // 编码
                            ret = avcodec_send_frame(codec_ctx, frame);
                            if (ret >= 0) {
                                while (avcodec_receive_packet(codec_ctx, pkt) >= 0) {
                                    av_packet_rescale_ts(pkt, codec_ctx->time_base, stream->time_base);
                                    pkt->stream_index = stream->index;
                                    av_interleaved_write_frame(fmt_ctx, pkt);
                                }
                            }
                            frame_count++;
                        }
                    }
                }
                av_packet_unref(png_pkt);
            }

            av_packet_free(&png_pkt);
            av_frame_free(&png_frame);
            avcodec_free_context(&png_codec);
        }

        avformat_close_input(&png_fmt);
        input_frame++;
        // 预估总帧数(启发式:检测到第100帧还没结束,假设还有更多)
        g_encode_progress = input_frame < 100 ? (float)input_frame / 100.0f : (float)input_frame / (input_frame + 50.0f);
    }

    // 13. 刷新编码器
    avcodec_send_frame(codec_ctx, NULL);
    while (avcodec_receive_packet(codec_ctx, pkt) >= 0) {
        av_packet_rescale_ts(pkt, codec_ctx->time_base, stream->time_base);
        pkt->stream_index = stream->index;
        av_interleaved_write_frame(fmt_ctx, pkt);
    }

    // 14. 写文件尾
    av_write_trailer(fmt_ctx);

    // 15. 清理
    sws_freeContext(sws_ctx);
    av_packet_free(&pkt);
    av_frame_free(&frame);
    avcodec_free_context(&codec_ctx);
    if (!(fmt_ctx->oformat->flags & AVFMT_NOFILE)) {
        avio_closep(&fmt_ctx->pb);
    }
    avformat_free_context(fmt_ctx);

    log_msg("[encode] 编码完成: %d 帧 -> %s\n", frame_count, g_encode_output);
    g_encode_status = 2;
    g_encode_progress = 1.0f;

    return NULL;
}

// Lua API: encode_video(input, output, fps, crf, codec)
static int l_encode_video(lua_State *L) {
    const char *input = luaL_checkstring(L, 1);
    const char *output = luaL_checkstring(L, 2);
    int fps = luaL_optinteger(L, 3, 24);
    int crf = luaL_optinteger(L, 4, 23);
    const char *codec = luaL_optstring(L, 5, "mpeg4");

    // 如果上次编码已完成,重置状态
    if (g_encode_status == 2 || g_encode_status == 3) {
        g_encode_status = 0;  // idle
        log_msg("[encode] 重置上次编码状态\n");
    }

    if (g_encode_status == 1) {
        lua_pushboolean(L, 0);
        lua_pushstring(L, "编码正在进行中");
        return 2;
    }

    strncpy(g_encode_input, input, sizeof(g_encode_input) - 1);
    strncpy(g_encode_output, output, sizeof(g_encode_output) - 1);
    g_encode_fps = fps;
    g_encode_crf = crf;
    strncpy(g_encode_codec, codec, sizeof(g_encode_codec) - 1);

    g_encode_status = 1;  // running
    g_encode_progress = 0.0f;
    g_encode_error[0] = '\0';

    // 检测输出格式
    size_t output_len = strlen(output);
    int is_gif = (output_len > 4 && strcmp(output + output_len - 4, ".gif") == 0);
    // APNG: 检测 .apng(明确动画意图)或 .png(标准,需判断输入是否多帧)
    int explicit_apng = (output_len > 5 && strcmp(output + output_len - 5, ".apng") == 0);
    int is_png = (output_len > 4 && strcmp(output + output_len - 4, ".png") == 0);

    // 如果是 .png,检查输入是否是多帧序列(包含 % 格式化符)
    int is_multi_frame = (strstr(input, "%") != NULL);

    // APNG 逻辑:
    // 1. 用户指定 .apng → 明确要动画
    // 2. 用户指定 .png + 多帧输入 → 自动切换到 APNG
    int is_apng = explicit_apng || (is_png && is_multi_frame && !is_gif);

    // 如果输出是 .png 单帧,不需要编码(静态 PNG)
    // 这种情况应该在 render 命令中处理,不调用 encode

    // 在后台线程启动编码
    if (is_apng) {
        pthread_create(&g_encode_thread, NULL, encode_apng_thread, NULL);
        log_msg("[encode] 启动 APNG 编码: %s -> %s\n", input, output);
    } else if (is_gif) {
        pthread_create(&g_encode_thread, NULL, encode_gif_thread, NULL);
        log_msg("[encode] 启动 GIF 编码: %s -> %s\n", input, output);
    } else {
        pthread_create(&g_encode_thread, NULL, encode_mp4_thread, NULL);
        log_msg("[encode] 启动 MP4 编码: %s -> %s\n", input, output);
    }
    pthread_detach(g_encode_thread);

    lua_pushboolean(L, 1);
    lua_pushstring(L, "编码已启动");
    return 2;
}

// Lua API: encode_status() -> table
static int l_encode_status(lua_State *L) {
    lua_newtable(L);

    lua_pushinteger(L, g_encode_status);
    lua_setfield(L, -2, "status");

    const char *status_text[] = {"idle", "running", "success", "error"};
    lua_pushstring(L, status_text[g_encode_status]);
    lua_setfield(L, -2, "status_text");

    lua_pushnumber(L, g_encode_progress);
    lua_setfield(L, -2, "progress");

    lua_pushstring(L, g_encode_output);
    lua_setfield(L, -2, "output_path");

    if (g_encode_status == 3) {
        lua_pushstring(L, g_encode_error);
        lua_setfield(L, -2, "error_msg");
    }

    return 1;
}

// Lua API: encode_cancel()
static int l_encode_cancel(lua_State *L) {
    if (g_encode_status == 1) {
        g_encode_status = 0;
        log_msg("[encode] 已取消\n");
        lua_pushboolean(L, 1);
    } else {
        lua_pushboolean(L, 0);
    }
    return 1;
}

// ========== Playback API ==========

// 前向声明
static void start_play_timer(void);
static void stop_play_timer(void);

// Lua API: play(start, end, fps)
static int l_play(lua_State *L) {
    g_play_start_frame = luaL_optinteger(L, 1, 0);
    g_play_end_frame = luaL_optinteger(L, 2, 72);
    g_play_fps = luaL_optinteger(L, 3, 24);
    g_play_current_frame = g_play_start_frame;
    g_play_status = 1;  // playing

    // 立即切换到起始帧
    char cmd[128];
    snprintf(cmd, sizeof(cmd), "moho:SetCurFrame(%d, true)", g_play_current_frame);
    if (g_L) {
        lua_getglobal(g_L, "ipc_execute");
        if (lua_isfunction(g_L, -1)) {
            lua_pushstring(g_L, cmd);
            lua_pcall(g_L, 1, 1, 0);
            lua_pop(g_L, 1);
        } else {
            lua_pop(g_L, 1);
        }
    }

    // 启动定时器
    start_play_timer();

    log_msg("[playback] 播放: %d-%d @ %dfps\n", g_play_start_frame, g_play_end_frame, g_play_fps);

    lua_pushboolean(L, 1);
    return 1;
}

// Lua API: pause()
static int l_pause(lua_State *L) {
    if (g_play_status == 1) {
        g_play_status = 2;  // paused
        stop_play_timer();  // 停止定时器
        log_msg("[playback] 已暂停 (frame=%d)\n", g_play_current_frame);
    } else if (g_play_status == 2) {
        g_play_status = 1;  // resume
        start_play_timer();  // 重启定时器
        log_msg("[playback] 已恢复 (frame=%d)\n", g_play_current_frame);
    }
    lua_pushboolean(L, 1);
    return 1;
}

// Lua API: stop_play()
static int l_stop_play(lua_State *L) {
    g_play_status = 0;  // stopped
    g_play_current_frame = 0;
    stop_play_timer();  // 停止定时器

    // 切换到帧 0
    if (g_L) {
        lua_getglobal(g_L, "ipc_execute");
        if (lua_isfunction(g_L, -1)) {
            lua_pushstring(g_L, "moho:SetCurFrame(0, true)");
            lua_pcall(g_L, 1, 1, 0);
            lua_pop(g_L, 1);
        } else {
            lua_pop(g_L, 1);
        }
    }

    log_msg("[playback] 已停止\n");
    lua_pushboolean(L, 1);
    return 1;
}

// Lua API: seek(frame)
static int l_seek(lua_State *L) {
    int frame = luaL_checkinteger(L, 1);
    g_play_current_frame = frame;

    // 立即切换帧
    char cmd[128];
    snprintf(cmd, sizeof(cmd), "moho:SetCurFrame(%d, true)", frame);
    if (g_L) {
        lua_getglobal(g_L, "ipc_execute");
        if (lua_isfunction(g_L, -1)) {
            lua_pushstring(g_L, cmd);
            lua_pcall(g_L, 1, 1, 0);
            lua_pop(g_L, 1);
        } else {
            lua_pop(g_L, 1);
        }
    }

    log_msg("[playback] 跳转: frame=%d\n", frame);
    lua_pushboolean(L, 1);
    return 1;
}

// Lua API: play_status() -> table
static int l_play_status(lua_State *L) {
    lua_newtable(L);

    lua_pushinteger(L, g_play_status);
    lua_setfield(L, -2, "status");

    const char *status_text[] = {"stopped", "playing", "paused"};
    lua_pushstring(L, status_text[g_play_status]);
    lua_setfield(L, -2, "status_text");

    lua_pushinteger(L, g_play_current_frame);
    lua_setfield(L, -2, "current_frame");

    lua_pushinteger(L, g_play_start_frame);
    lua_setfield(L, -2, "start_frame");

    lua_pushinteger(L, g_play_end_frame);
    lua_setfield(L, -2, "end_frame");

    lua_pushinteger(L, g_play_fps);
    lua_setfield(L, -2, "fps");

    return 1;
}

// Lua API: is_playing() -> boolean
static int l_is_playing(lua_State *L) {
    lua_pushboolean(L, g_play_status == 1);
    return 1;
}

// 播放定时器回调(在主线程执行帧切换)
static void play_timer_callback(CFRunLoopTimerRef timer, void *info) {
    if (g_play_status != 1) return;  // 不是播放状态

    // 切换到下一帧
    g_play_current_frame++;

    // 检查是否结束
    if (g_play_current_frame > g_play_end_frame) {
        g_play_current_frame = g_play_start_frame;  // 循环播放
        // 或者停止: g_play_status = 0;
    }

    // 执行帧切换命令
    char cmd[128];
    snprintf(cmd, sizeof(cmd), "moho:SetCurFrame(%d, false)", g_play_current_frame);

    if (g_L) {
        lua_getglobal(g_L, "ipc_execute");
        if (lua_isfunction(g_L, -1)) {
            lua_pushstring(g_L, cmd);
            lua_pcall(g_L, 1, 1, 0);
            lua_pop(g_L, 1);
        } else {
            lua_pop(g_L, 1);
        }
    }
}

// 启动播放定时器
static void start_play_timer(void) {
    if (g_play_timer) return;  // 已存在

    // 计算定时器间隔(秒)
    double interval = 1.0 / g_play_fps;

    // 创建定时器
    CFRunLoopTimerContext ctx = {0, NULL, NULL, NULL, NULL};
    g_play_timer = CFRunLoopTimerCreate(
        kCFAllocatorDefault,
        CFAbsoluteTimeGetCurrent() + interval,  // 第一点火时间
        interval,  // 间隔
        0,  // flags
        0,  // order
        play_timer_callback,
        &ctx
    );

    if (g_play_timer) {
        CFRunLoopAddTimer(CFRunLoopGetCurrent(), g_play_timer, kCFRunLoopDefaultMode);
        log_msg("[playback] 定时器已启动 (interval=%.3fs)\n", interval);
    }
}

// 停止播放定时器
static void stop_play_timer(void) {
    if (g_play_timer) {
        CFRunLoopRemoveTimer(CFRunLoopGetCurrent(), g_play_timer, kCFRunLoopDefaultMode);
        CFRelease(g_play_timer);
        g_play_timer = NULL;
        log_msg("[playback] 定时器已停止\n");
    }
}



// Lua API: quit()
// 停止 IPC 并退出 Moho
static int l_quit(lua_State *L) {
    log_msg("=== IPC quit ===\n");

    // 1. 停止 IPC
    l_stop(L);

    // 2. 获取 moho 对象并调用 Quit
    lua_getglobal(L, "moho");
    if (lua_istable(L, -1)) {
        lua_getfield(L, -1, "Quit");
        if (lua_isfunction(L, -1)) {
            lua_pushvalue(L, -2);  // self = moho
            lua_pcall(L, 1, 0, 0);
            log_msg("✓ moho:Quit() 已调用\n");
        } else {
            lua_pop(L, 1);
        }
    }
    lua_pop(L, 1);

    lua_pushboolean(L, 1);
    return 1;
}

// 模块注册
static const luaL_Reg funcs[] = {
    {"start", l_start},
    {"stop", l_stop},
    {"quit", l_quit},
    {"status", l_status},
    {"check", l_check},
    {"poll", l_poll},
    {"encode_video", l_encode_video},
    {"encode_status", l_encode_status},
    {"encode_cancel", l_encode_cancel},
    {"play", l_play},
    {"pause", l_pause},
    {"stop_play", l_stop_play},
    {"seek", l_seek},
    {"play_status", l_play_status},
    {"is_playing", l_is_playing},
    {NULL, NULL}
};

int luaopen_moho_ipc(lua_State *L) {
    // ⚠️ 检查是否已经加载(防止重复加载)
    lua_getfield(L, LUA_REGISTRYINDEX, "_ipc_module");
    if (lua_istable(L, -1)) {
        log_msg("模块已存在,返回缓存\n");
        return 1;  // 返回已存在的模块
    }
    lua_pop(L, 1);  // 移除 nil

    // 创建新模块 table
    lua_newtable(L);

    // 注册函数
    for (int i = 0; funcs[i].name; i++) {
        lua_pushcfunction(L, funcs[i].func);
        lua_setfield(L, -2, funcs[i].name);
    }

    // 同时存到 registry(给 C 的 execute_via_helper 用)
    lua_pushvalue(L, -1);  // 复制 table
    lua_setfield(L, LUA_REGISTRYINDEX, "_ipc_module");

    log_msg("模块加载(隔离模式,不注册 package.loaded)\n");
    return 1;  // 返回 table,但 Moho 不会自动注册
}