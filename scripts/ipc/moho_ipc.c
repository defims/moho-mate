/*
 * moho_ipc.c - Moho IPC with CFSocket + ScriptInterfaceHelper
 *
 * 原理:
 * 1. CFSocket 收到命令后调用 Lua 的 ipc_execute()
 * 2. ipc_execute() 内部使用 MOHO.ScriptInterfaceHelper 获取 moho
 * 3. 官方推荐的"敏感资源"管理方式,稳定不崩溃
 *
 * 用法:
 * local ipc = require("moho_ipc")
 * ipc.start()  -- 启动 IPC 服务
 *
 * 发送命令:
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
#include <pthread.h>
#include <dispatch/dispatch.h>

// FFmpeg headers
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
#include <libswscale/swscale.h>
#include <libavfilter/avfilter.h>
#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>

#define SOCKET_PATH "/tmp/moho_ipc.sock"
#define CMD_SIZE 8192
#define RESP_SIZE 16384  // 响应缓冲区大小
#define SCRIPTS_DIR "/Users/def/.openclaw/workspace/skills/moho-mate/scripts"

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

// 执行命令(通过 Lua 的 ipc_execute,内部使用 ScriptInterfaceHelper)
// 返回响应字符串(存储在 g_response)
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

        // 通过 ipc_execute 执行(内部使用 ScriptInterfaceHelper)
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

// ========== Playback 状态 ==========
static volatile int g_play_status = 0;  // 0=stopped, 1=playing, 2=paused
static volatile int g_play_current_frame = 0;
static volatile int g_play_start_frame = 0;
static volatile int g_play_end_frame = 72;
static volatile int g_play_fps = 24;
static CFRunLoopTimerRef g_play_timer = NULL;
static CFRunLoopSourceRef g_play_source = NULL;

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
    
    // === 第四步：读取 PNG 序列并推入滤镜管道 ===
    pkt = av_packet_alloc();
    frame = av_frame_alloc();   // 输出帧（PAL8）
    png_frame = av_frame_alloc();  // 输入帧（RGBA）
    
    int input_frame = 0;
    
    // 第一阶段：将所有帧推入滤镜管道
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
    log_msg("[encode] 共读取 %d 帧，开始生成调色板...\n", total_frames);
    
    // 刷新滤镜管道（让 palettegen 生成调色板）
    ret = av_buffersrc_add_frame_flags(buffersrc_ctx, NULL, AV_BUFFERSRC_FLAG_PUSH);
    if (ret < 0) {
        log_msg("[encode] 无法刷新滤镜管道\n");
    }
    
    // 第二阶段：从滤镜管道拉取处理后的帧并编码
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

    // 在后台线程启动编码
    if (is_gif) {
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

// 播放定时器回调（在主线程执行帧切换）
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
    
    // 计算定时器间隔（秒）
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



// 模块注册
static const luaL_Reg funcs[] = {
    {"start", l_start},
    {"stop", l_stop},
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
    luaL_newlib(L, funcs);
    log_msg("模块加载 (ScriptInterfaceHelper 执行版)\n");
    return 1;
}