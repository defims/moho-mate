/*
 * moho-mate.c - Moho 命令行工具（C 版本）
 *
 * 用法:
 *   moho-mate start [project.moho] [script.lua]
 *   moho-mate call '<lua>'
 *   moho-mate call -f script.lua
 *   moho-mate quit
 *   moho-mate status
 *   moho-mate render project.moho -f PNG -o output
 *   moho-mate encode input output --fps 24
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <dirent.h>
#include <errno.h>
#include <libgen.h>
#include <time.h>
#include <spawn.h>
#include <signal.h>
#include <curl/curl.h>

// ========== 配置 ==========

#define MOHO_APP "/Applications/Moho.app"
#define IPC_SOCKET "/tmp/moho_ipc.sock"
#define IPC_CMD_DIR "/tmp/moho_ipc_cmds"
#define IPC_TOOL "/Users/def/.openclaw/workspace/skills/moho-mate/scripts/ipc/ipc_tool.lua"
#define MOHO_CONFIG_DIR "/Users/def/Library/Preferences/Lost Marble/Moho Pro/14"
#define SCRIPTS_DIR "/Users/def/.openclaw/workspace/skills/moho-mate/scripts"
#define IPC_CONFIG_BACKUP "/tmp/moho_ipc_config_backup"
#define IPC_BACKUP_PID_FILE "/tmp/moho_ipc_backup.pid"
#define EMPTY_CONFIG_TEMPLATE "/Users/def/.openclaw/workspace/skills/moho-mate/scripts/ipc/empty_config"

// ========== 配置管理（IPC 自动备份/恢复） ==========

// IPC 配置备份（启动前）
static int ipc_config_backup(void) {
    // 备份到固定目录（不区分 PID）
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
        // 写入 PID 文件（标记 IPC 会话）
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

// IPC 使用空配置（清空 autosave）
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

// IPC 配置恢复（退出后）
static int ipc_config_restore(void) {
    char backup_dir[512];
    snprintf(backup_dir, sizeof(backup_dir), "%s", IPC_CONFIG_BACKUP);
    
    // 检查 PID 文件（确认 IPC 会话）
    FILE *f = fopen(IPC_BACKUP_PID_FILE, "r");
    if (!f) {
        printf("⚠ 无 IPC 会话标记，跳过恢复\n");
        return 0;
    }
    fclose(f);
    
    struct stat st;
    if (stat(backup_dir, &st) != 0) {
        printf("⚠ 无配置备份，跳过恢复\n");
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

// curl 写回调
static size_t write_callback(void *ptr, size_t size, size_t nmemb, void *userdata) {
    char *buf = (char*)userdata;
    size_t total = size * nmemb;
    strncpy(buf, (char*)ptr, total < 511 ? total : 511);
    buf[total < 511 ? total : 511] = 0;
    return total;
}

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
        fprintf(stderr, "✗ IPC 连接失败（服务未启动？）\n");
        return 1;
    }
    
    // 获取 token
    char token[128] = "";
    CURL *curl = curl_easy_init();
    if (curl) {
        char url[256];
        snprintf(url, sizeof(url), "http://127.0.0.1:9527/token/create");
        struct curl_slist *headers = NULL;
        headers = curl_slist_append(headers, "Content-Type: application/json");
        
        char response[512] = "";
        curl_easy_setopt(curl, CURLOPT_URL, url);
        curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, "{\"client_id\":\"moho-mate\"}");
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
        curl_easy_setopt(curl, CURLOPT_WRITEDATA, response);
        curl_easy_perform(curl);
        curl_easy_cleanup(curl);
        curl_slist_free_all(headers);
        
        // 解析 token: {"token":"xxx",...}
        char *t = strstr(response, "\"token\":\"");
        if (t) {
            t += 9;  // skip "token":""
            char *end = strchr(t, '"');
            if (end) {
                int len = end - t;
                if (len > 0 && len < (int)sizeof(token)) {
                    strncpy(token, t, len);
                    token[len] = 0;
                }
            }
        }
    }
    
    // 发送 auth
    char auth_cmd[256];
    snprintf(auth_cmd, sizeof(auth_cmd), "auth %s", token);
    send(sock, auth_cmd, strlen(auth_cmd), 0);
    send(sock, "\n", 1, 0);
    
    char resp[1024];
    int n = recv(sock, resp, sizeof(resp) - 1, 0);
    if (n > 0) {
        resp[n] = 0;
        if (strncmp(resp, "ok|", 3) != 0) {
            close(sock);
            fprintf(stderr, "✗ Token 验证失败: %s", resp);
            return 1;
        }
    }
    
    // 发送命令
    send(sock, cmd, strlen(cmd), 0);
    send(sock, "\n", 1, 0);
    
    // 接收响应
    n = recv(sock, resp, sizeof(resp) - 1, 0);
    close(sock);
    
    if (n > 0) {
        resp[n] = 0;
        // 去除尾部换行
        while (n > 0 && (resp[n-1] == '\n' || resp[n-1] == '\r')) resp[--n] = 0;
        
        if (strncmp(resp, "ok|", 3) == 0) {
            printf("%s\n", resp + 3);
            return 0;
        } else {
            fprintf(stderr, "✗ %s\n", resp);
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
    // 写入临时文件
    mkdir(IPC_CMD_DIR, 0755);
    
    char tmpfile[256];
    snprintf(tmpfile, sizeof(tmpfile), "%s/cmd_%d_%ld.lua", IPC_CMD_DIR, getpid(), time(NULL));
    
    FILE *f = fopen(tmpfile, "w");
    if (!f) {
        fprintf(stderr, "✗ 无法写入临时文件\n");
        return 1;
    }
    fprintf(f, "%s", code);
    fclose(f);
    
    int ret = ipc_send_file(tmpfile);
    unlink(tmpfile);
    return ret;
}

static int ipc_check_running(void) {
    struct stat st;
    return stat(IPC_SOCKET, &st) == 0;
}

static int auto_start_ipc(void) {
    if (ipc_check_running()) return 0;
    
    printf("▶ IPC 未启动，自动启动...\n");
    
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
    
    // 启动 Moho（用 open 命令）
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
    
    // 写入 IPC 启动代码（使用 ipc_tool.lua 模板）
    // 设置变量供 ipc_tool.lua 使用
    fprintf(f, "IPC_DIR = \"%s\"\n", SCRIPTS_DIR);
    fprintf(f, "USER_PROJECT = \"%s\"\n", project ? project : "");
    fprintf(f, "USER_SCRIPT = \"%s\"\n", script ? script : "");
    fprintf(f, "IPC_TIMEOUT = %d\n", timeout);
    fprintf(f, "dofile(\"%s\")\n", IPC_TOOL);
    fclose(f);
    
    // 启动 Moho（用 open 命令）
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
            printf("关闭 Moho: moho-mate call 'ipc_quit()'\n");
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
        // 即使 Moho 未运行，也尝试恢复配置
        ipc_config_restore();
        return 0;
    }
    printf("▶ 退出 Moho\n");
    int ret = ipc_send("ipc_quit()");
    
    // 等待 socket 断开（最多 10 秒）
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
    
    if (is_apng) {
        printf("▶ 编码 APNG（动画 PNG，无损 + 透明）\n");
    } else if (is_gif) {
        printf("▶ 编码 GIF（libavfilter 调色板优化）\n");
    } else {
        printf("▶ 编码 MP4（内置 FFmpeg）\n");
    }
    printf("  输入: %s\n", input);
    printf("  输出: %s\n", output);
    printf("  帧率: %d fps\n", fps);
    
    auto_start_ipc();
    
    // 发送编码命令
    char lua_cmd[1024];
    snprintf(lua_cmd, sizeof(lua_cmd),
        "local ipc = require('moho_ipc')\n"
        "local ok, err = ipc.encode_video(\"%s\", \"%s\", %d, %d, \"mpeg4\")\n"
        "if ok then print('✓ 编码完成') else print('✗ 编码失败: ' .. tostring(err)) end",
        input, output, fps, crf);
    
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
        const char *format_name = strcmp(format, "APNG") == 0 ? "APNG（动画 PNG）" : format;
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
    char lua_cmd[1024];
    char output_path[512];
    
    if (output) {
        snprintf(output_path, sizeof(output_path), "%s", output);
    } else {
        // 从项目名生成输出名
        char *base = strrchr(project, '/');
        base = base ? base + 1 : project;
        char name[256];
        strncpy(name, base, sizeof(name));
        char *dot = strrchr(name, '.');
        if (dot) *dot = '\0';
        
        if (is_video) {
            snprintf(output_path, sizeof(output_path), "/tmp/%s.%s", name, format);
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
        "-- 使用全局 moho 对象（IPC 环境已设置）\n"
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
        "print('✓ PNG 渲染完成: ' .. (%d - %d + 1) .. ' 帧')",
        png_dir, start_frame, end_frame, ext, end_frame, start_frame);
    
    int ret = ipc_send_multiline(lua_cmd);
    
    if (ret != 0) {
        fprintf(stderr, "✗ 渲染失败\n");
        return ret;
    }
    
    // 视频格式：调用 encode 编码
    if (is_video) {
        printf("✓ PNG 序列已保存到: %s\n", png_dir);
        
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
        
        char encode_cmd[1024];
        snprintf(encode_cmd, sizeof(encode_cmd),
            "local ipc = require('moho_ipc')\n"
            "local input = \"%s/frame_%%05d.png\"\n"
            "local output = \"%s\"\n"
            "local fps = 24\n"
            "local ok, err = ipc.encode_video(input, output, fps, 23, \"%s\")\n"
            "if ok then\n"
            "  print('✓ 编码完成: ' .. output)\n"
            "else\n"
            "  print('✗ 编码失败: ' .. tostring(err))\n"
            "end",
            png_dir, output_path, codec);
        
        ret = ipc_send_multiline(encode_cmd);
        
        // 等待编码完成
        sleep(2);
        
        // 清理临时 PNG
        printf("▶ 清理临时帧...\n");
        char cleanup_cmd[256];
        snprintf(cleanup_cmd, sizeof(cleanup_cmd), "rm -rf \"%s\"", png_dir);
        system(cleanup_cmd);
        
        if (ret == 0) {
            printf("✓ 视频已保存到: %s\n", output_path);
        }
    } else {
        printf("✓ PNG 序列已保存到: %s\n", output_path);
    }
    
    return ret;
}

// ========== draw（IPC 模式）==========

static int cmd_draw(int argc, char **argv) {
    char *shape = argc > 1 ? argv[1] : "circle";
    
    // 检查支持的形状
    if (strcmp(shape, "circle") != 0 && strcmp(shape, "bunny") != 0 && strcmp(shape, "puppy") != 0) {
        fprintf(stderr, "✗ 未知形状: %s\n", shape);
        fprintf(stderr, "可用形状: circle, bunny, puppy\n");
        return 1;
    }
    
    printf("▶ 绘制形状: %s\n", shape);
    printf("⚠️ draw 只绘制，不保存。请手动 Cmd+S\n");
    
    auto_start_ipc();
    
    // 使用 draw_ipc.lua 脚本（不保存）
    char lua_cmd[256];
    snprintf(lua_cmd, sizeof(lua_cmd),
        "local home = os.getenv('HOME')\n"
        "dofile(home .. '/.openclaw/workspace/skills/moho-mate/scripts/draw_ipc.lua')\n"
        "draw_shape('%s')", shape);
    
    printf("▶ IPC 绘制中...\n");
    int ret = ipc_send_multiline(lua_cmd);
    
    if (ret == 0) {
        printf("✓ 已绘制 %s，请手动保存\n", shape);
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
    
    // 解析 .moho 文件（XML 格式）
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
    printf("  %s draw <shape>                    绘制形状（不保存）\n", prog);
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