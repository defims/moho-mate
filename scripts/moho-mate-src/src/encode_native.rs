//! FFmpeg 原生编码模块
//!
//! 使用自定义 FFI 绑定 Moho 内置 FFmpeg 库
//!
//! ## 支持格式
//!
//! | 格式 | 说明 | 依赖 |
//! |------|------|------|
//! | MP4 | H.264 视频 | avcodec, avformat, avutil, swscale |
//! | APNG | 动画 PNG | avcodec, avformat, avutil |
//! | GIF | 调色板优化 | avfilter, avutil |
//!
//! ## macOS 库加载方案
//!
//! ### 最终方案：Frameworks 符号链接
//!
//! ```text
//! skills/moho-mate/
//! ├── Frameworks -> /Applications/Moho.app/Contents/Frameworks/
//! └── scripts/
//!     ├── moho-mate
//!     └── libavfilter.10.dylib
//! ```
//!
//! ### 路径解析
//!
//! Moho 内置 FFmpeg 的 install name：
//! ```text
//! @executable_path/../Frameworks/libavcodec.61.dylib
//! ```
//!
//! 当 moho-mate 运行时（在 scripts/ 目录）：
//! ```text
//! @executable_path = scripts/
//! @executable_path/../Frameworks = skills/moho-mate/Frameworks/
//!                                     ↓ 符号链接
//!                            /Applications/Moho.app/Contents/Frameworks/
//! ```
//!
//! ### libavfilter 特殊处理
//!
//! - libavfilter.10.dylib 放在 **scripts 目录**（Moho 没有内置）
//! - 使用 @rpath 路径，rpath 在 build.rs 中设置
//!
//! ### 为什么不需要 install_name_tool？
//!
//! | 方案 | 符号链接数量 | install_name_tool | 用户干预 |
//! |------|-------------|-------------------|--------|
//! | ~~旧方案~~ | 0 | 需要（每次编译） | 无 |
//! | **新方案** | 1（Frameworks） | 不需要 | 一次 |
//!
//! 新方案更优：
//! - 符号链接永久有效
//! - 无需修改二进制文件
//! - 编译后直接可用
//!
//! ### 为什么不在 scripts/ 目录创建库符号链接？
//!
//! 因为库之间也有依赖：
//! ```text
//! libavformat.61.dylib
//!   └── @loader_path/../Frameworks/libavcodec.61.dylib
//! ```
//!
//! `@loader_path` = 当前库所在目录。如果在 scripts/ 创建符号链接：
//! ```text
//! @loader_path/../Frameworks = scripts/../Frameworks = skills/moho-mate/Frameworks
//! ```
//!
//! 最终还是需要 Frameworks 符号链接，所以在 scripts/ 目录创建库符号链接无效。
//!
//! ## Windows DLL 说明
//!
//! ### 为什么需要额外 DLL？
//!
//! 1. **Moho 没有内置 libavfilter**
//!    - Moho 内置: avcodec, avformat, avutil, swscale, swresample
//!    - 缺少: avfilter
//!
//! 2. **GIF 编码需要 libavfilter**
//!    - palettegen: 生成调色板
//!    - paletteuse: 应用调色板
//!    - 高质量 GIF 输出
//!
//! 3. **avfilter 依赖 avutil**
//!    - avfilter-10.dll 需要加载 avutil-59.dll
//!    - 两个文件必须一起分发
//!
//! ### 文件列表
//!
//! | 文件 | 大小 | 说明 |
//! |------|------|------|
//! | avfilter-10.dll | 22 MB | FFmpeg 滤镜库（GIF 调色板优化） |
//! | avutil-59.dll | 3.9 MB | FFmpeg 工具库（avfilter 依赖） |
//!
//! ### 获取方式
//!
//! **方法 1: 使用已编译的文件（推荐）**
//!
//! scripts 目录已包含预编译的 DLL 文件。
//!
//! **方法 2: 从 FFmpeg 官方下载**
//!
//! 1. 访问: https://www.gyan.dev/ffmpeg/builds/
//! 2. 下载: ffmpeg-release-shared.7z
//! 3. 解压后找到 bin/avfilter-10.dll 和 bin/avutil-59.dll
//! 4. 复制到 scripts 目录
//!
//! **方法 3: 交叉编译（在 macOS/Linux 上）**
//!
//! ```bash
//! # 安装 MinGW-w64
//! # macOS: brew install mingw-w64
//! # Linux: sudo apt install mingw-w64
//!
//! # 下载 FFmpeg n7.1 源码
//! git clone --depth 1 --branch n7.1 https://github.com/FFmpeg/FFmpeg.git
//! cd FFmpeg
//!
//! # 配置（Windows x86_64）
//! ./configure \
//!   --arch=x86_64 \
//!   --target-os=mingw64 \
//!   --cross-prefix=x86_64-w64-mingw32- \
//!   --enable-shared \
//!   --disable-static \
//!   --disable-programs \
//!   --disable-doc \
//!   --disable-x86asm \
//!   --enable-avfilter
//!
//! # 编译
//! make -j8
//!
//! # 输出: libavfilter/avfilter-10.dll, libavutil/avutil-59.dll
//! ```
//!
//! ## 常见问题排查
//!
//! ### 问题 1: dyld: Library not loaded
//!
//! ```
//! dyld: Library not loaded: @executable_path/../Frameworks/libavcodec.61.dylib
//! ```
//!
//! **原因**: Frameworks 符号链接不存在
//!
//! **解决**: 运行 build.sh 或手动创建符号链接
//! ```bash
//! cd skills/moho-mate
//! ln -s /Applications/Moho.app/Contents/Frameworks Frameworks
//! ```
//!
//! ### 问题 2: 找不到 avfilter-10.dll
//!
//! **原因**: Windows 缺少 DLL 文件
//!
//! **解决**: 从 scripts 目录复制 avfilter-10.dll 和 avutil-59.dll
//!
//! ## 相关文件
//!
//! - build.rs: 设置 rpath (macOS)
//! - build.sh: 创建 Frameworks 符号链接
//! - ffmpeg_ffi.rs: FFmpeg FFI 绑定

use crate::ipc_core;
use crate::ffmpeg_ffi as av;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::Ordering;
use tracing::info;

/// 检查 Moho 内置 FFmpeg 库是否可用
/// 
/// ## 平台差异
/// 
/// - macOS: /Applications/Moho.app/Contents/Frameworks/
/// - Windows: C:\Program Files\Moho\
/// - Linux: /usr/local/lib/ 或 Moho 安装目录
/// 
/// ## Windows 说明
/// 
/// Windows 版本需要检查两个位置：
/// 1. Moho 安装目录（内置库）
/// 2. scripts 目录（avfilter-10.dll 和 avutil-59.dll）
pub fn check_ffmpeg_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        let moho_fw = Path::new("/Applications/Moho.app/Contents/Frameworks");
        let libs = [
            "libavcodec.61.dylib",
            "libavformat.61.dylib",
            "libavutil.59.dylib",
            "libswscale.8.dylib",
            "libswresample.5.dylib",
        ];
        libs.iter().all(|lib| moho_fw.join(lib).exists())
    }
    
    #[cfg(target_os = "windows")]
    {
        // Windows: 检查 Moho 安装目录
        let moho_dir = Path::new("C:\\Program Files\\Moho");
        let libs = [
            "avcodec-61.dll",
            "avformat-61.dll",
            "avutil-59.dll",
            "swscale-8.dll",
            "swresample-5.dll",
        ];
        
        // 如果 Moho 目录存在，检查内置库
        if moho_dir.exists() {
            libs.iter().all(|lib| moho_dir.join(lib).exists())
        } else {
            // 否则检查 scripts 目录
            let scripts_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."));
            libs.iter().all(|lib| scripts_dir.join(lib).exists())
        }
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Linux: 检查系统库或 Moho 安装目录
        let libs = [
            "libavcodec.so.61",
            "libavformat.so.61",
            "libavutil.so.59",
            "libswscale.so.8",
            "libswresample.so.5",
        ];
        libs.iter().all(|lib| Path::new("/usr/local/lib").join(lib).exists())
    }
}

/// 检查 libavfilter 是否可用
/// 
/// ## 关键点
/// 
/// - libavfilter 在 **scripts 目录**，不是 Moho 内置目录
/// - Moho 没有内置 libavfilter
/// - GIF 编码需要 libavfilter（调色板优化）
/// 
/// ## 路径
/// 
/// - macOS: scripts/libavfilter.10.dylib
/// - Windows: scripts/avfilter-10.dll
/// - Linux: scripts/libavfilter.so.10
/// 
/// ## 运行时加载
/// 
/// - macOS: 通过 rpath 加载（见 build.rs）
/// - Windows: 通过 PATH 或 DLL 目录加载
/// 
/// ## 命名差异
/// 
/// | 平台 | 文件名 |
/// |------|--------|
/// | macOS | libavfilter.10.dylib |
/// | Windows | avfilter-10.dll |
/// | Linux | libavfilter.so.10 |
/// 
/// ## Windows 依赖
/// 
/// avfilter-10.dll 依赖 avutil-59.dll，需要一起分发。
/// 
/// 已通过交叉编译生成（在 macOS 上使用 MinGW-w64）：
/// - avfilter-10.dll (22 MB)
/// - avutil-59.dll (3.9 MB)
pub fn check_avfilter_available() -> bool {
    // 获取 scripts 目录路径
    // 优先使用环境变量，否则使用默认路径
    let scripts_dir = if let Ok(dir) = std::env::var("MOHO_MATE_SCRIPTS_DIR") {
        PathBuf::from(dir)
    } else {
        // 默认路径：可执行文件所在目录
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        
        // scripts 目录可能在 exe 同级或上级
        if exe_dir.join("libavfilter.10.dylib").exists() || exe_dir.join("avfilter-10.dll").exists() {
            exe_dir
        } else {
            // 尝试默认安装路径
            PathBuf::from("/Users/def/.openclaw/workspace/skills/moho-mate/scripts")
        }
    };
    
    // 根据平台检查不同的文件名
    #[cfg(target_os = "macos")]
    let lib_name = "libavfilter.10.dylib";
    
    #[cfg(target_os = "windows")]
    let lib_name = "avfilter-10.dll";
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let lib_name = "libavfilter.so.10";
    
    scripts_dir.join(lib_name).exists()
}

pub fn encode_gif_with_palette(input: &str, output: &str, fps: i32) -> anyhow::Result<()> {
    unsafe {
        // 获取第一帧分辨率
        let mut width = 0i32;
        let mut height = 0i32;
        
        let png_path = input.replace("%05d", &format!("{:05}", 0));
        let png_path_c = CString::new(png_path.as_str()).unwrap();
        
        let mut fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
        let ret = av::avformat_open_input(&mut fmt_ctx, png_path_c.as_ptr(), ptr::null(), ptr::null_mut());
        if ret < 0 {
            anyhow::bail!("无法读取第一帧: {}", png_path);
        }
        
        av::avformat_find_stream_info(fmt_ctx, ptr::null_mut());
        
        for i in 0..(*fmt_ctx).nb_streams {
            let stream = *(*fmt_ctx).streams.add(i as usize);
            if (*(*stream).codecpar).codec_type == av::AVMEDIA_TYPE_VIDEO {
                width = (*(*stream).codecpar).width;
                height = (*(*stream).codecpar).height;
                break;
            }
        }
        
        av::avformat_close_input(&mut fmt_ctx);
        
        if width <= 0 || height <= 0 {
            anyhow::bail!("无法检测输入分辨率");
        }
        
        info!("GIF 编码 (libavfilter): {}x{}, fps={}", width, height, fps);
        
        // === 创建 GIF 编码器 ===
        let codec = av::avcodec_find_encoder(av::AV_CODEC_ID_GIF);
        if codec.is_null() {
            anyhow::bail!("找不到 GIF 编码器");
        }
        
        let output_c = CString::new(output).unwrap();
        let mut out_fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
        let ret = av::avformat_alloc_output_context2(&mut out_fmt_ctx, ptr::null(), ptr::null(), output_c.as_ptr());
        if ret < 0 {
            anyhow::bail!("无法创建输出上下文");
        }
        
        let stream = av::avformat_new_stream(out_fmt_ctx, ptr::null());
        if stream.is_null() {
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法创建流");
        }
        
        let mut codec_ctx = av::avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法分配编码器上下文");
        }
        
        (*codec_ctx).width = width;
        (*codec_ctx).height = height;
        (*codec_ctx).time_base = av::AVRational::new(1, fps);
        (*codec_ctx).framerate = av::AVRational::new(fps, 1);
        (*codec_ctx).pix_fmt = av::AV_PIX_FMT_PAL8;
        
        let ret = av::avcodec_open2(codec_ctx, codec, ptr::null_mut());
        if ret < 0 {
            av::avcodec_free_context(&mut codec_ctx);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法打开编码器: {}", av::av_err2str(ret));
        }
        
        av::avcodec_parameters_from_context((*stream).codecpar, codec_ctx);
        (*stream).time_base = (*codec_ctx).time_base;
        
        let ret = av::avio_open(&mut (*out_fmt_ctx).pb, output_c.as_ptr(), av::AVIO_FLAG_WRITE);
        if ret < 0 {
            av::avcodec_free_context(&mut codec_ctx);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法打开输出文件");
        }
        
        av::avformat_write_header(out_fmt_ctx, ptr::null_mut());
        
        // === 创建 libavfilter 滤镜管道 ===
        let mut filter_graph = av::avfilter_graph_alloc();
        if filter_graph.is_null() {
            av::avcodec_free_context(&mut codec_ctx);
            av::avio_closep(&mut (*out_fmt_ctx).pb);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法创建滤镜图");
        }
        
        // 创建 buffersrc
        let buffersrc = av::avfilter_get_by_name(CString::new("buffer").unwrap().as_ptr());
        let args = format!("video_size={}x{}:pix_fmt={}:time_base=1/{}", width, height, av::AV_PIX_FMT_RGBA as i32, fps);
        let args_c = CString::new(args.as_str()).unwrap();
        let mut buffersrc_ctx: *mut av::AVFilterContext = ptr::null_mut();
        let ret = av::avfilter_graph_create_filter(&mut buffersrc_ctx, buffersrc, CString::new("in").unwrap().as_ptr(), args_c.as_ptr(), ptr::null_mut(), filter_graph);
        if ret < 0 {
            av::avfilter_graph_free(&mut filter_graph);
            av::avcodec_free_context(&mut codec_ctx);
            av::avio_closep(&mut (*out_fmt_ctx).pb);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法创建 buffersrc");
        }
        
        // 创建 buffersink
        let buffersink = av::avfilter_get_by_name(CString::new("buffersink").unwrap().as_ptr());
        let mut buffersink_ctx: *mut av::AVFilterContext = ptr::null_mut();
        let ret = av::avfilter_graph_create_filter(&mut buffersink_ctx, buffersink, CString::new("out").unwrap().as_ptr(), ptr::null(), ptr::null_mut(), filter_graph);
        if ret < 0 {
            av::avfilter_graph_free(&mut filter_graph);
            av::avcodec_free_context(&mut codec_ctx);
            av::avio_closep(&mut (*out_fmt_ctx).pb);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法创建 buffersink");
        }
        
        // 设置滤镜端点
        let mut outputs = av::avfilter_inout_alloc();
        let mut inputs = av::avfilter_inout_alloc();
        
        (*outputs).name = av::av_strdup(CString::new("in").unwrap().as_ptr());
        (*outputs).filter_ctx = buffersrc_ctx;
        (*outputs).pad_idx = 0;
        (*outputs).next = ptr::null_mut();
        
        (*inputs).name = av::av_strdup(CString::new("out").unwrap().as_ptr());
        (*inputs).filter_ctx = buffersink_ctx;
        (*inputs).pad_idx = 0;
        (*inputs).next = ptr::null_mut();
        
        // 滤镜链: format=rgb24,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse
        let filter_desc = CString::new("format=rgb24,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5").unwrap();
        let ret = av::avfilter_graph_parse_ptr(filter_graph, filter_desc.as_ptr(), &mut inputs, &mut outputs, ptr::null_mut());
        if ret < 0 {
            av::avfilter_inout_free(&mut inputs);
            av::avfilter_inout_free(&mut outputs);
            av::avfilter_graph_free(&mut filter_graph);
            av::avcodec_free_context(&mut codec_ctx);
            av::avio_closep(&mut (*out_fmt_ctx).pb);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法解析滤镜链");
        }
        
        let ret = av::avfilter_graph_config(filter_graph, ptr::null_mut());
        if ret < 0 {
            av::avfilter_inout_free(&mut inputs);
            av::avfilter_inout_free(&mut outputs);
            av::avfilter_graph_free(&mut filter_graph);
            av::avcodec_free_context(&mut codec_ctx);
            av::avio_closep(&mut (*out_fmt_ctx).pb);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法配置滤镜图");
        }
        
        av::avfilter_inout_free(&mut inputs);
        av::avfilter_inout_free(&mut outputs);
        
        info!("GIF 滤镜管道已创建");
        
        // === 第一阶段: 读取 PNG 序列并推入滤镜管道 ===
        let mut png_frame = av::av_frame_alloc();
        let mut input_frame_idx = 0i32;
        
        loop {
            let png_path = input.replace("%05d", &format!("{:05}", input_frame_idx));
            if !Path::new(&png_path).exists() {
                break;
            }
            
            let png_path_c = CString::new(png_path.as_str()).unwrap();
            let mut png_fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
            
            let ret = av::avformat_open_input(&mut png_fmt_ctx, png_path_c.as_ptr(), ptr::null(), ptr::null_mut());
            if ret < 0 {
                input_frame_idx += 1;
                continue;
            }
            
            av::avformat_find_stream_info(png_fmt_ctx, ptr::null_mut());
            
            // 查找视频流
            let mut video_stream_idx = -1i32;
            for i in 0..(*png_fmt_ctx).nb_streams {
                let stream = *(*png_fmt_ctx).streams.add(i as usize);
                if (*(*stream).codecpar).codec_type == av::AVMEDIA_TYPE_VIDEO {
                    video_stream_idx = i as i32;
                    break;
                }
            }
            
            if video_stream_idx >= 0 {
                let stream = *(*png_fmt_ctx).streams.add(video_stream_idx as usize);
                let png_decoder = av::avcodec_find_decoder((*(*stream).codecpar).codec_id);
                let mut png_codec_ctx = av::avcodec_alloc_context3(png_decoder);
                av::avcodec_parameters_to_context(png_codec_ctx, (*stream).codecpar);
                av::avcodec_open2(png_codec_ctx, png_decoder, ptr::null_mut());
                
                let mut png_pkt = av::av_packet_alloc();
                while av::av_read_frame(png_fmt_ctx, png_pkt) >= 0 {
                    if (*png_pkt).stream_index == video_stream_idx {
                        let ret = av::avcodec_send_packet(png_codec_ctx, png_pkt);
                        if ret >= 0 {
                            let ret = av::avcodec_receive_frame(png_codec_ctx, png_frame);
                            if ret >= 0 {
                                // 推入滤镜管道
                                (*png_frame).pts = input_frame_idx as i64;
                                av::av_buffersrc_add_frame_flags(buffersrc_ctx, png_frame, 0);
                            }
                        }
                    }
                    av::av_packet_unref(png_pkt);
                }
                av::av_packet_free(&mut png_pkt);
                av::avcodec_free_context(&mut png_codec_ctx);
            }
            
            av::avformat_close_input(&mut png_fmt_ctx);
            input_frame_idx += 1;
            
            let progress = (input_frame_idx as f64 / (input_frame_idx as f64 + 100.0) * 0.5 * 100.0) as i32;
            ipc_core::ENCODE_PROGRESS.store(progress.min(4900), Ordering::SeqCst);
        }
        
        info!("共读取 {} 帧, 开始生成调色板...", input_frame_idx);
        
        // 刷新滤镜管道
        av::av_buffersrc_add_frame_flags(buffersrc_ctx, ptr::null_mut(), av::AV_BUFFERSRC_FLAG_PUSH);
        
        // === 第二阶段: 从滤镜管道拉取处理后的帧并编码 ===
        let mut frame = av::av_frame_alloc();
        let mut pkt = av::av_packet_alloc();
        let mut frame_count = 0i64;
        
        while av::av_buffersink_get_frame(buffersink_ctx, frame) >= 0 {
            (*frame).pts = frame_count;
            
            // 编码
            av::avcodec_send_frame(codec_ctx, frame);
            while av::avcodec_receive_packet(codec_ctx, pkt) >= 0 {
                (*pkt).stream_index = 0;
                av::av_interleaved_write_frame(out_fmt_ctx, pkt);
            }
            
            frame_count += 1;
            
            let progress = (0.5 + frame_count as f64 / input_frame_idx as f64 * 0.5 * 100.0) as i32;
            ipc_core::ENCODE_PROGRESS.store(progress.min(9900), Ordering::SeqCst);
        }
        
        // 写文件尾
        av::av_write_trailer(out_fmt_ctx);
        
        // 清理
        av::av_frame_free(&mut frame);
        av::av_frame_free(&mut png_frame);
        av::av_packet_free(&mut pkt);
        av::avfilter_graph_free(&mut filter_graph);
        av::avcodec_free_context(&mut codec_ctx);
        av::avio_closep(&mut (*out_fmt_ctx).pb);
        av::avformat_free_context(out_fmt_ctx);
        
        info!("GIF 编码完成: {} ({} 帧)", output, frame_count);
        Ok(())
    }
}

pub fn encode_mp4(input: &str, output: &str, fps: i32, crf: i32) -> anyhow::Result<()> {
    unsafe {
        let mut width = 0i32;
        let mut height = 0i32;
        
        let png_path = input.replace("%05d", &format!("{:05}", 0));
        let png_path_c = CString::new(png_path.as_str()).unwrap();
        
        let mut fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
        let ret = av::avformat_open_input(&mut fmt_ctx, png_path_c.as_ptr(), ptr::null(), ptr::null_mut());
        if ret < 0 {
            anyhow::bail!("无法读取第一帧: {}", png_path);
        }
        
        av::avformat_find_stream_info(fmt_ctx, ptr::null_mut());
        
        for i in 0..(*fmt_ctx).nb_streams {
            let stream = *(*fmt_ctx).streams.add(i as usize);
            if (*(*stream).codecpar).codec_type == av::AVMEDIA_TYPE_VIDEO {
                width = (*(*stream).codecpar).width;
                height = (*(*stream).codecpar).height;
                break;
            }
        }
        
        av::avformat_close_input(&mut fmt_ctx);
        
        if width <= 0 || height <= 0 {
            anyhow::bail!("无法检测输入分辨率");
        }
        
        info!("MP4 编码: {}x{}, fps={}, crf={}", width, height, fps, crf);
        
        let codec = av::avcodec_find_encoder(av::AV_CODEC_ID_MPEG4);
        if codec.is_null() {
            anyhow::bail!("找不到 MPEG4 编码器");
        }
        
        let output_c = CString::new(output).unwrap();
        let mut out_fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
        let ret = av::avformat_alloc_output_context2(&mut out_fmt_ctx, ptr::null(), ptr::null(), output_c.as_ptr());
        if ret < 0 {
            anyhow::bail!("无法创建输出上下文");
        }
        
        let stream = av::avformat_new_stream(out_fmt_ctx, ptr::null());
        if stream.is_null() {
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法创建流");
        }
        
        let mut codec_ctx = av::avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法分配编码器上下文");
        }
        
        (*codec_ctx).width = width;
        (*codec_ctx).height = height;
        (*codec_ctx).time_base = av::AVRational::new(1, fps);
        (*codec_ctx).framerate = av::AVRational::new(fps, 1);
        (*codec_ctx).pix_fmt = av::AV_PIX_FMT_YUV420P;
        // global_quality 需要通过 AVCodecContext 的正确字段设置
        // 暂时跳过，使用默认值
        
        let ret = av::avcodec_open2(codec_ctx, codec, ptr::null_mut());
        if ret < 0 {
            av::avcodec_free_context(&mut codec_ctx);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法打开编码器: {}", av::av_err2str(ret));
        }
        
        av::avcodec_parameters_from_context((*stream).codecpar, codec_ctx);
        (*stream).time_base = (*codec_ctx).time_base;
        
        let ret = av::avio_open(&mut (*out_fmt_ctx).pb, output_c.as_ptr(), av::AVIO_FLAG_WRITE);
        if ret < 0 {
            av::avcodec_free_context(&mut codec_ctx);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法打开输出文件");
        }
        
        av::avformat_write_header(out_fmt_ctx, ptr::null_mut());
        
        let mut frame = av::av_frame_alloc();
        let mut pkt = av::av_packet_alloc();
        
        (*frame).format = av::AV_PIX_FMT_YUV420P as i32;
        (*frame).width = width;
        (*frame).height = height;
        av::av_frame_get_buffer(frame, 0);
        
        let png_decoder = av::avcodec_find_decoder(av::AV_CODEC_ID_PNG);
        let mut png_codec_ctx = av::avcodec_alloc_context3(png_decoder);
        av::avcodec_open2(png_codec_ctx, png_decoder, ptr::null_mut());
        
        let mut sws_ctx = av::sws_getContext(
            width, height, av::AV_PIX_FMT_RGBA,
            width, height, av::AV_PIX_FMT_YUV420P,
            av::SWS_BILINEAR, ptr::null(), ptr::null(), ptr::null()
        );
        
        let mut frame_count = 0i64;
        let mut input_frame = 0i32;
        
        loop {
            let png_path = input.replace("%05d", &format!("{:05}", input_frame));
            if !Path::new(&png_path).exists() {
                break;
            }
            
            let png_path_c = CString::new(png_path.as_str()).unwrap();
            let mut png_fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
            
            let ret = av::avformat_open_input(&mut png_fmt_ctx, png_path_c.as_ptr(), ptr::null(), ptr::null_mut());
            if ret < 0 {
                input_frame += 1;
                continue;
            }
            
            av::avformat_find_stream_info(png_fmt_ctx, ptr::null_mut());
            
            let mut png_pkt = av::av_packet_alloc();
            while av::av_read_frame(png_fmt_ctx, png_pkt) >= 0 {
                if (*png_pkt).stream_index == 0 {
                    let ret = av::avcodec_send_packet(png_codec_ctx, png_pkt);
                    if ret >= 0 {
                        let mut png_frame = av::av_frame_alloc();
                        if av::avcodec_receive_frame(png_codec_ctx, png_frame) >= 0 {
                            av::av_frame_make_writable(frame);
                            
                            av::sws_scale(
                                sws_ctx,
                                (*png_frame).data.as_ptr() as *const _,
                                (*png_frame).linesize.as_ptr() as *const _,
                                0, height,
                                (*frame).data.as_ptr() as *mut _,
                                (*frame).linesize.as_ptr() as *mut _
                            );
                            
                            (*frame).pts = frame_count;
                            
                            av::avcodec_send_frame(codec_ctx, frame);
                            while av::avcodec_receive_packet(codec_ctx, pkt) >= 0 {
                                (*pkt).stream_index = 0;
                                av::av_interleaved_write_frame(out_fmt_ctx, pkt);
                            }
                            
                            frame_count += 1;
                        }
                        av::av_frame_free(&mut png_frame);
                    }
                }
                av::av_packet_unref(png_pkt);
            }
            
            av::av_packet_free(&mut png_pkt);
            av::avformat_close_input(&mut png_fmt_ctx);
            input_frame += 1;
            
            let progress = (input_frame as f64 / 100.0 * 100.0) as i32;
            ipc_core::ENCODE_PROGRESS.store(progress.min(9900), Ordering::SeqCst);
        }
        
        av::avcodec_send_frame(codec_ctx, ptr::null_mut());
        while av::avcodec_receive_packet(codec_ctx, pkt) >= 0 {
            (*pkt).stream_index = 0;
            av::av_interleaved_write_frame(out_fmt_ctx, pkt);
        }
        
        av::av_write_trailer(out_fmt_ctx);
        
        av::sws_freeContext(sws_ctx);
        av::avcodec_free_context(&mut png_codec_ctx);
        av::av_frame_free(&mut frame);
        av::av_packet_free(&mut pkt);
        av::avcodec_free_context(&mut codec_ctx);
        av::avio_closep(&mut (*out_fmt_ctx).pb);
        av::avformat_free_context(out_fmt_ctx);
        
        info!("MP4 编码完成: {} ({} 帧)", output, frame_count);
        Ok(())
    }
}

pub fn encode_apng(input: &str, output: &str, fps: i32) -> anyhow::Result<()> {
    unsafe {
        let mut width = 0i32;
        let mut height = 0i32;
        
        let png_path = input.replace("%05d", &format!("{:05}", 0));
        let png_path_c = CString::new(png_path.as_str()).unwrap();
        
        eprintln!("[DEBUG APNG] 读取第一帧: {}", png_path);
        
        let mut fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
        let ret = av::avformat_open_input(&mut fmt_ctx, png_path_c.as_ptr(), ptr::null(), ptr::null_mut());
        if ret < 0 {
            eprintln!("[DEBUG APNG] avformat_open_input 失败: ret={}", ret);
            anyhow::bail!("无法读取第一帧: {}", png_path);
        }
        eprintln!("[DEBUG APNG] avformat_open_input 成功");
        
        av::avformat_find_stream_info(fmt_ctx, ptr::null_mut());
        
        eprintln!("[DEBUG APNG] nb_streams={}", (*fmt_ctx).nb_streams);
        
        for i in 0..(*fmt_ctx).nb_streams {
            let stream = *(*fmt_ctx).streams.add(i as usize);
            eprintln!("[DEBUG APNG] stream {}: codec_type={}", i, (*(*stream).codecpar).codec_type);
            if (*(*stream).codecpar).codec_type == av::AVMEDIA_TYPE_VIDEO {
                width = (*(*stream).codecpar).width;
                height = (*(*stream).codecpar).height;
                eprintln!("[DEBUG APNG] 检测到视频流: {}x{}", width, height);
                break;
            }
        }
        
        av::avformat_close_input(&mut fmt_ctx);
        
        if width <= 0 || height <= 0 {
            anyhow::bail!("无法检测输入分辨率");
        }
        
        info!("APNG 编码: {}x{}, fps={}", width, height, fps);
        
        let codec = av::avcodec_find_encoder(av::AV_CODEC_ID_APNG);
        if codec.is_null() {
            anyhow::bail!("找不到 APNG 编码器");
        }
        
        // APNG 必须使用 .png 后缀，但需要显式指定 apng muxer
        let actual_output = if output.ends_with(".apng") {
            output.to_string()  // 保持 .apng 后缀
        } else {
            output.to_string()
        };
        
        let output_c = CString::new(actual_output.as_str()).unwrap();
        let format_name = CString::new("apng").unwrap();
        let mut out_fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
        let ret = av::avformat_alloc_output_context2(&mut out_fmt_ctx, ptr::null(), format_name.as_ptr(), output_c.as_ptr());
        if ret < 0 {
            anyhow::bail!("无法创建输出上下文");
        }
        
        // 调试：检查 oformat 是否正确
        let oformat = (*out_fmt_ctx).oformat;
        if oformat.is_null() {
            anyhow::bail!("无法获取输出格式");
        }
        
        eprintln!("[DEBUG] APNG muxer 已创建, oformat={:?}", oformat);
        
        let stream = av::avformat_new_stream(out_fmt_ctx, ptr::null());
        if stream.is_null() {
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法创建流");
        }
        
        let mut codec_ctx = av::avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法分配编码器上下文");
        }
        
        (*codec_ctx).width = width;
        (*codec_ctx).height = height;
        (*codec_ctx).time_base = av::AVRational::new(1, fps);
        (*codec_ctx).framerate = av::AVRational::new(fps, 1);
        (*codec_ctx).pix_fmt = av::AV_PIX_FMT_RGBA;
        
        let ret = av::avcodec_open2(codec_ctx, codec, ptr::null_mut());
        if ret < 0 {
            av::avcodec_free_context(&mut codec_ctx);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法打开编码器: {}", av::av_err2str(ret));
        }
        eprintln!("[DEBUG APNG] 编码器已打开");
        
        av::avcodec_parameters_from_context((*stream).codecpar, codec_ctx);
        (*stream).time_base = (*codec_ctx).time_base;
        eprintln!("[DEBUG APNG] 流参数已设置");
        
        let ret = av::avio_open(&mut (*out_fmt_ctx).pb, output_c.as_ptr(), av::AVIO_FLAG_WRITE);
        if ret < 0 {
            eprintln!("[DEBUG APNG] avio_open 失败: ret={}", ret);
            av::avcodec_free_context(&mut codec_ctx);
            av::avformat_free_context(out_fmt_ctx);
            anyhow::bail!("无法打开输出文件");
        }
        eprintln!("[DEBUG APNG] 输出文件已打开: {}", actual_output);
        
        av::avformat_write_header(out_fmt_ctx, ptr::null_mut());
        eprintln!("[DEBUG APNG] 文件头已写入");
        
        let mut frame = av::av_frame_alloc();
        let mut pkt = av::av_packet_alloc();
        
        (*frame).format = av::AV_PIX_FMT_RGBA as i32;
        (*frame).width = width;
        (*frame).height = height;
        av::av_frame_get_buffer(frame, 0);
        
        let png_decoder = av::avcodec_find_decoder(av::AV_CODEC_ID_PNG);
        let mut png_codec_ctx = av::avcodec_alloc_context3(png_decoder);
        av::avcodec_open2(png_codec_ctx, png_decoder, ptr::null_mut());
        
        let mut frame_count = 0i64;
        let mut input_frame = 0i32;
        
        loop {
            let png_path = input.replace("%05d", &format!("{:05}", input_frame));
            if !Path::new(&png_path).exists() {
                break;
            }
            
            let png_path_c = CString::new(png_path.as_str()).unwrap();
            let mut png_fmt_ctx: *mut av::AVFormatContext = ptr::null_mut();
            
            let ret = av::avformat_open_input(&mut png_fmt_ctx, png_path_c.as_ptr(), ptr::null(), ptr::null_mut());
            if ret < 0 {
                input_frame += 1;
                continue;
            }
            
            av::avformat_find_stream_info(png_fmt_ctx, ptr::null_mut());
            
            let mut png_pkt = av::av_packet_alloc();
            while av::av_read_frame(png_fmt_ctx, png_pkt) >= 0 {
                if (*png_pkt).stream_index == 0 {
                    let ret = av::avcodec_send_packet(png_codec_ctx, png_pkt);
                    if ret >= 0 {
                        let mut png_frame = av::av_frame_alloc();
                        if av::avcodec_receive_frame(png_codec_ctx, png_frame) >= 0 {
                            av::av_frame_make_writable(frame);
                            av::av_frame_copy(frame, png_frame);
                            
                            (*frame).pts = frame_count;
                            
                            av::avcodec_send_frame(codec_ctx, frame);
                            while av::avcodec_receive_packet(codec_ctx, pkt) >= 0 {
                                (*pkt).stream_index = 0;
                                av::av_interleaved_write_frame(out_fmt_ctx, pkt);
                            }
                            
                            frame_count += 1;
                        }
                        av::av_frame_free(&mut png_frame);
                    }
                }
                av::av_packet_unref(png_pkt);
            }
            
            av::av_packet_free(&mut png_pkt);
            av::avformat_close_input(&mut png_fmt_ctx);
            input_frame += 1;
            
            let progress = (input_frame as f64 / 100.0 * 100.0) as i32;
            ipc_core::ENCODE_PROGRESS.store(progress.min(9900), Ordering::SeqCst);
        }
        
        av::avcodec_send_frame(codec_ctx, ptr::null_mut());
        while av::avcodec_receive_packet(codec_ctx, pkt) >= 0 {
            (*pkt).stream_index = 0;
            av::av_interleaved_write_frame(out_fmt_ctx, pkt);
        }
        
        av::av_write_trailer(out_fmt_ctx);
        
        av::avcodec_free_context(&mut png_codec_ctx);
        av::av_frame_free(&mut frame);
        av::av_packet_free(&mut pkt);
        av::avcodec_free_context(&mut codec_ctx);
        av::avio_closep(&mut (*out_fmt_ctx).pb);
        av::avformat_free_context(out_fmt_ctx);
        
        info!("APNG 编码完成: {} ({} 帧)", actual_output, frame_count);
        Ok(())
    }
}

/// 使用内置 FFmpeg 编码（自动检测格式）
/// 
/// ## 格式支持
/// 
/// | 格式 | 状态 | 说明 |
/// |------|:----:|------|
/// | MP4 | ✅ | 使用 Moho 内置 FFmpeg |
/// | APNG | ✅ | 使用 Moho 内置 FFmpeg |
/// | GIF | ✅ | 需要 libavfilter（scripts 目录）|
/// 
/// ## GIF 编码
/// 
/// GIF 需要 libavfilter 做调色板优化：
/// 1. check_avfilter_available() 检查 scripts/libavfilter.10.dylib
/// 2. encode_gif_with_palette() 使用 libavfilter 滤镜链
/// 
/// ## 相关文件
/// 
/// - build.rs: 设置 rpath 指向 scripts 目录
/// - build.sh: 执行 install_name_tool
/// - ffmpeg_ffi.rs: libavfilter FFI 绑定
pub fn encode_with_builtin_ffmpeg(input: &str, output: &str, fps: i32, crf: i32) -> anyhow::Result<()> {
    if !check_ffmpeg_available() {
        anyhow::bail!("Moho 内置 FFmpeg 库不可用");
    }
    
    let output_ext = if output.ends_with(".gif") {
        "gif"
    } else if output.ends_with(".apng") || output.ends_with(".png") {
        "apng"
    } else {
        "mp4"
    };
    
    info!("使用内置 FFmpeg 编码: {} -> {} ({})", input, output, output_ext);
    
    let result = if output_ext == "gif" {
        // GIF 使用 libavfilter（调色板优化）
        if !check_avfilter_available() {
            anyhow::bail!(
                "GIF 编码需要 libavfilter，请确保 scripts/libavfilter.10.dylib 存在\n\
                或安装系统 ffmpeg：brew install ffmpeg"
            )
        }
        encode_gif_with_palette(input, output, fps)
    } else if output_ext == "apng" {
        encode_apng(input, output, fps)
    } else {
        encode_mp4(input, output, fps, crf)
    };
    
    // 更新进度
    ipc_core::ENCODE_PROGRESS.store(10000, Ordering::SeqCst);
    
    result
}
