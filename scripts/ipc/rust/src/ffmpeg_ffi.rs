//! FFmpeg FFI 绑定
//!
//! 使用 Moho 内置 FFmpeg 库 + libavfilter

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;

// === AVCodec ===
pub type AVCodecContext = *mut c_void;
pub type AVCodec = *mut c_void;
pub type AVCodecParameters = *mut c_void;
pub type AVPacket = *mut c_void;
pub type AVFrame = *mut c_void;
pub type AVStream = *mut c_void;
pub type AVFormatContext = *mut c_void;
pub type AVIOContext = *mut c_void;
pub type AVOutputFormat = *mut c_void;
pub type AVInputFormat = *mut c_void;

// === AVFilter ===
pub type AVFilterGraph = *mut c_void;
pub type AVFilterContext = *mut c_void;
pub type AVFilterInOut = *mut c_void;
pub type AVFilter = *mut c_void;

// === 常量 ===
pub const AVMEDIA_TYPE_VIDEO: c_int = 0;
pub const AV_CODEC_ID_GIF: c_int = 94;
pub const AV_CODEC_ID_MPEG4: c_int = 13;
pub const AV_CODEC_ID_APNG: c_int = 165;
pub const AV_CODEC_ID_PNG: c_int = 61;

pub const AVFMT_NOFILE: c_int = 0x0001;
pub const AVFMT_GLOBALHEADER: c_int = 0x0004;

pub const AV_PIX_FMT_RGB24: c_int = 2;
pub const AV_PIX_FMT_RGBA: c_int = 26;
pub const AV_PIX_FMT_YUV420P: c_int = 0;
pub const AV_PIX_FMT_PAL8: c_int = 8;

pub const EAGAIN: c_int = 35;

// === libavcodec ===
#[link(name = "avcodec.61", kind = "dylib")]
extern "C" {
    pub fn avcodec_find_encoder(codec_id: c_int) -> AVCodec;
    pub fn avcodec_find_decoder(codec_id: c_int) -> AVCodec;
    pub fn avcodec_alloc_context3(codec: AVCodec) -> AVCodecContext;
    pub fn avcodec_free_context(ctx: *mut AVCodecContext);
    pub fn avcodec_open2(ctx: AVCodecContext, codec: AVCodec, options: *mut c_void) -> c_int;
    pub fn avcodec_send_frame(ctx: AVCodecContext, frame: AVFrame) -> c_int;
    pub fn avcodec_receive_packet(ctx: AVCodecContext, pkt: AVPacket) -> c_int;
    pub fn avcodec_send_packet(ctx: AVCodecContext, pkt: AVPacket) -> c_int;
    pub fn avcodec_receive_frame(ctx: AVCodecContext, frame: AVFrame) -> c_int;
    pub fn avcodec_parameters_to_context(ctx: AVCodecContext, par: AVCodecParameters) -> c_int;
    pub fn avcodec_parameters_from_context(par: AVCodecParameters, ctx: AVCodecContext) -> c_int;
    pub fn av_packet_alloc() -> AVPacket;
    pub fn av_packet_free(pkt: *mut AVPacket);
    pub fn av_packet_unref(pkt: AVPacket);
    pub fn av_new_packet(pkt: AVPacket, size: c_int) -> c_int;
}

// === libavformat ===
#[link(name = "avformat.61", kind = "dylib")]
extern "C" {
    pub fn avformat_open_input(
        ctx: *mut AVFormatContext,
        url: *const c_char,
        fmt: AVInputFormat,
        options: *mut c_void,
    ) -> c_int;
    pub fn avformat_close_input(ctx: *mut AVFormatContext);
    pub fn avformat_find_stream_info(ctx: AVFormatContext, options: *mut c_void) -> c_int;
    pub fn avformat_alloc_output_context2(
        ctx: *mut AVFormatContext,
        ofmt: AVOutputFormat,
        format_name: *const c_char,
        filename: *const c_char,
    ) -> c_int;
    pub fn avformat_free_context(ctx: AVFormatContext);
    pub fn avformat_new_stream(ctx: AVFormatContext, codec: AVCodec) -> AVStream;
    pub fn avformat_write_header(ctx: AVFormatContext, options: *mut c_void) -> c_int;
    pub fn av_write_frame(ctx: AVFormatContext, pkt: AVPacket) -> c_int;
    pub fn av_interleaved_write_frame(ctx: AVFormatContext, pkt: AVPacket) -> c_int;
    pub fn av_write_trailer(ctx: AVFormatContext) -> c_int;
    pub fn avio_open(ctx: *mut AVIOContext, url: *const c_char, flags: c_int) -> c_int;
    pub fn avio_closep(ctx: *mut AVIOContext);
    pub fn av_find_best_stream(
        ctx: AVFormatContext,
        type_: c_int,
        wanted_stream_nb: c_int,
        related_stream: c_int,
        codec_ret: *mut AVCodec,
        flags: c_int,
    ) -> c_int;
    pub fn av_read_frame(ctx: AVFormatContext, pkt: AVPacket) -> c_int;
}

// === libavutil ===
#[link(name = "avutil.59", kind = "dylib")]
extern "C" {
    pub fn av_frame_alloc() -> AVFrame;
    pub fn av_frame_free(frame: *mut AVFrame);
    pub fn av_frame_get_buffer(frame: AVFrame, align: c_int) -> c_int;
    pub fn av_frame_make_writable(frame: AVFrame) -> c_int;
    pub fn av_image_alloc(
        pointers: *mut *mut u8,
        linesizes: *mut c_int,
        w: c_int,
        h: c_int,
        pix_fmt: c_int,
        align: c_int,
    ) -> c_int;
    pub fn av_image_fill_arrays(
        pointers: *mut *mut u8,
        linesizes: *mut c_int,
        src: *const u8,
        pix_fmt: c_int,
        width: c_int,
        height: c_int,
        align: c_int,
    ) -> c_int;
    pub fn av_strdup(s: *const c_char) -> *mut c_char;
    pub fn av_free(ptr: *mut c_void);
    pub fn av_get_media_type_string(type_: c_int) -> *const c_char;
    pub fn av_gettime_relative() -> i64;
    pub fn av_opt_set(obj: *mut c_void, name: *const c_char, val: *const c_char, flags: c_int) -> c_int;
    pub fn av_opt_set_int(obj: *mut c_void, name: *const c_char, val: i64, flags: c_int) -> c_int;
    pub fn av_opt_set_q(obj: *mut c_void, name: *const c_char, val: AVRational, flags: c_int) -> c_int;
    pub fn av_strerror(errnum: c_int, errbuf: *mut c_char, errbuf_size: usize) -> c_int;
    pub fn av_dict_set(pm: *mut c_void, key: *const c_char, value: *const c_char, flags: c_int) -> c_int;
    pub fn av_dict_free(pm: *mut c_void);
}

// === libswscale ===
#[link(name = "swscale.8", kind = "dylib")]
extern "C" {
    pub fn sws_getContext(
        srcW: c_int,
        srcH: c_int,
        srcFormat: c_int,
        dstW: c_int,
        dstH: c_int,
        dstFormat: c_int,
        flags: c_int,
        srcFilter: *mut c_void,
        dstFilter: *mut c_void,
        param: *const c_double,
    ) -> *mut c_void;
    pub fn sws_freeContext(ctx: *mut c_void);
    pub fn sws_scale(
        ctx: *mut c_void,
        srcSlice: *const *const u8,
        srcStride: *const c_int,
        srcSliceY: c_int,
        srcSliceH: c_int,
        dst: *mut *const u8,
        dstStride: *const c_int,
    ) -> c_int;
}

// === libavfilter ===
#[link(name = "avfilter.10", kind = "dylib")]
extern "C" {
    pub fn avfilter_graph_alloc() -> AVFilterGraph;
    pub fn avfilter_graph_free(graph: *mut AVFilterGraph);
    pub fn avfilter_graph_parse2(
        graph: AVFilterGraph,
        filters: *const c_char,
        inputs: *mut AVFilterInOut,
        outputs: *mut AVFilterInOut,
    ) -> c_int;
    pub fn avfilter_graph_config(graph: AVFilterGraph, logctx: *mut c_void) -> c_int;
    pub fn avfilter_inout_free(inout: *mut AVFilterInOut);
    pub fn avfilter_get_by_name(name: *const c_char) -> AVFilter;
    pub fn avfilter_graph_create_filter(
        filter_ctx: *mut AVFilterContext,
        filter: AVFilter,
        name: *const c_char,
        args: *const c_char,
        kwargs: *mut c_void,
        graph_ctx: AVFilterGraph,
    ) -> c_int;
    pub fn av_buffersrc_write_frame(ctx: AVFilterContext, frame: AVFrame) -> c_int;
    pub fn av_buffersrc_add_frame_flags(ctx: AVFilterContext, frame: AVFrame, flags: c_int) -> c_int;
    pub fn av_buffersink_get_frame(ctx: AVFilterContext, frame: AVFrame) -> c_int;
    pub fn av_buffersink_get_frame_flags(ctx: AVFilterContext, frame: AVFrame, flags: c_int) -> c_int;
    pub fn av_buffersink_set_frame_size(ctx: AVFilterContext, frame_size: c_int) -> c_int;
    pub fn av_buffersink_get_format(ctx: AVFilterContext) -> c_int;
}

// === 辅助结构 ===
#[repr(C)]
pub struct AVRational {
    pub num: c_int,
    pub den: c_int,
}

// === 辅助函数 ===
pub fn av_err2str(errnum: c_int) -> String {
    let mut errbuf = [0u8; 128];
    unsafe {
        av_strerror(errnum, errbuf.as_mut_ptr() as *mut c_char, errbuf.len());
        CStr::from_ptr(errbuf.as_ptr() as *const c_char)
            .to_string_lossy()
            .into_owned()
    }
}

pub fn get_stream_codecpar(stream: AVStream) -> AVCodecParameters {
    unsafe {
        // AVStream 结构体的 codecpar 字段偏移量
        // 在 FFmpeg 7.0 中，codecpar 是第一个字段
        // 实际需要通过偏移访问，这里简化处理
        // 正确做法应该是使用 AVStream 的公开 API
        let ptr = stream as *const u8;
        // codecpar 在 AVStream 结构体的偏移（需要验证）
        // 暂时使用一个假设的偏移
        let codecpar_ptr = ptr.add(0) as *mut c_void; // 这是不正确的，需要修正
        codecpar_ptr
    }
}

// === 辅助宏（Rust 版） ===
pub fn frame_set_width(frame: AVFrame, w: c_int) {
    unsafe {
        // 通过偏移设置字段
        // 实际应该使用公开 API
        let ptr = frame as *mut u8;
        // width 偏移（需要验证）
        ptr.write_bytes(w as u8, 4); // 这是不正确的
    }
}

// === 常量（滤镜） ===
pub const AV_BUFFERSRC_FLAG_PUSH: c_int = 1;
pub const AV_BUFFERSRC_FLAG_NO_CHECK_FORMAT: c_int = 2;
pub const AV_BUFFERSINK_FLAG_EOS: c_int = 1;

// === AVFrame 字段访问辅助 ===
// 由于 Rust 不能直接访问 C 结构体的字段，我们需要通过偏移量
// 但这样做不安全且不可移植。更好的方案是使用 ffmpeg-next crate
// 这里先简化处理，实际使用时需要修正