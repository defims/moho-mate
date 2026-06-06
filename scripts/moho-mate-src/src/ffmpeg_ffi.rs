//! FFmpeg FFI 绑定（手写版）
//!
//! 基于 FFmpeg 7.x API，从 rusty_ffmpeg 生成的绑定中提取
//! 只保留 encode_native.rs 实际使用的部分

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::ptr;

// === 基础类型别名 ===
pub type AVPixelFormat = c_int;
pub type AVMediaType = c_int;
pub type AVCodecID = c_int;

// === 常量 ===
pub const AVMEDIA_TYPE_VIDEO: c_int = 0;
pub const AV_CODEC_ID_GIF: c_int = 97;
pub const AV_CODEC_ID_MPEG4: c_int = 12;
pub const AV_CODEC_ID_APNG: c_int = 210;
pub const AV_CODEC_ID_PNG: c_int = 61;

pub const AV_PIX_FMT_RGB24: AVPixelFormat = 2;
pub const AV_PIX_FMT_RGBA: AVPixelFormat = 26;
pub const AV_PIX_FMT_YUV420P: AVPixelFormat = 0;
pub const AV_PIX_FMT_PAL8: AVPixelFormat = 8;

pub const AVIO_FLAG_WRITE: c_int = 2;

pub const AV_BUFFERSRC_FLAG_PUSH: c_int = 1;
pub const AV_BUFFERSRC_FLAG_NO_CHECK_FORMAT: c_int = 2;

pub const SWS_BILINEAR: c_int = 2;

// === AVRational（有理数） ===
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AVRational {
    pub num: c_int,
    pub den: c_int,
}

impl AVRational {
    pub fn new(num: c_int, den: c_int) -> Self {
        Self { num, den }
    }
}

// === AVBufferRef ===
#[repr(C)]
pub struct AVBufferRef {
    pub buffer: *mut c_void,
    pub data: *mut u8,
    pub size: c_int,
}

// === AVClass（仅用于指针） ===
#[repr(C)]
pub struct AVClass {
    _opaque: [u8; 0],
}

// === AVCodecParameters ===
#[repr(C)]
pub struct AVCodecParameters {
    pub codec_type: AVMediaType,
    pub codec_id: AVCodecID,
    pub codec_tag: c_uint,
    pub extradata: *mut u8,
    pub extradata_size: c_int,
    pub coded_side_data: *mut c_void,  // AVPacketSideData*
    pub nb_coded_side_data: c_int,
    pub format: c_int,
    pub bit_rate: i64,
    pub bits_per_coded_sample: c_int,
    pub bits_per_raw_sample: c_int,
    pub profile: c_int,
    pub level: c_int,
    pub width: c_int,
    pub height: c_int,
    pub sample_aspect_ratio: AVRational,
    pub framerate: AVRational,
    // ... 其他字段省略
}

// === AVCodecContext（编码器上下文） ===
#[repr(C)]
pub struct AVCodecContext {
    pub av_class: *const AVClass,
    pub log_level_offset: c_int,
    pub codec_type: AVMediaType,
    pub codec: *const AVCodec,
    pub codec_id: AVCodecID,
    pub codec_tag: c_uint,
    pub priv_data: *mut c_void,
    pub internal: *mut c_void,
    pub opaque: *mut c_void,
    pub bit_rate: i64,
    pub flags: c_int,
    pub flags2: c_int,
    pub extradata: *mut u8,
    pub extradata_size: c_int,
    pub time_base: AVRational,
    pub pkt_timebase: AVRational,
    pub framerate: AVRational,
    pub ticks_per_frame: c_int,
    pub delay: c_int,
    pub width: c_int,
    pub height: c_int,
    pub coded_width: c_int,
    pub coded_height: c_int,
    pub sample_aspect_ratio: AVRational,
    pub pix_fmt: AVPixelFormat,
    // ... 其他字段省略（后续按需添加）
    // 需要找到 global_quality 字段的偏移
    // 在 FFmpeg 7.x 中，global_quality 在 pix_fmt 之后约 200 字节处
    // 实际位置需要验证
    _padding: [u8; 200], // 占位，让结构体足够大
}

// === AVFrame（帧） ===
#[repr(C)]
pub struct AVFrame {
    pub data: [*mut u8; 8],
    pub linesize: [c_int; 8],
    pub extended_data: *mut *mut u8,
    pub width: c_int,
    pub height: c_int,
    pub nb_samples: c_int,
    pub format: c_int,
    pub key_frame: c_int,
    pub pict_type: c_int,
    pub sample_aspect_ratio: AVRational,
    pub pts: i64,
    pub pkt_dts: i64,
    pub time_base: AVRational,
    pub quality: c_int,
    pub opaque: *mut c_void,
    pub repeat_pict: c_int,
    pub interlaced_frame: c_int,
    pub top_field_first: c_int,
    pub palette_has_changed: c_int,
    pub sample_rate: c_int,
    pub buf: [*mut AVBufferRef; 8],
    pub extended_buf: *mut *mut AVBufferRef,
    pub nb_extended_buf: c_int,
    pub side_data: *mut *mut c_void,
    pub nb_side_data: c_int,
    pub flags: c_int,
    // ... 其他字段省略
}

// === AVPacket（数据包） ===
#[repr(C)]
pub struct AVPacket {
    pub buf: *mut AVBufferRef,
    pub pts: i64,
    pub dts: i64,
    pub data: *mut u8,
    pub size: c_int,
    pub stream_index: c_int,
    pub flags: c_int,
    pub side_data: *mut c_void,
    pub side_data_elems: c_int,
    pub duration: i64,
    pub pos: i64,
    pub opaque: *mut c_void,
    pub opaque_ref: *mut AVBufferRef,
    pub time_base: AVRational,
}

// === AVStream（流） ===
#[repr(C)]
pub struct AVStream {
    pub av_class: *const AVClass,
    pub index: c_int,
    pub id: c_int,
    pub codecpar: *mut AVCodecParameters,
    pub priv_data: *mut c_void,
    pub time_base: AVRational,
    pub start_time: i64,
    pub duration: i64,
    // ... 其他字段省略
}

// === AVFormatContext（格式上下文） ===
#[repr(C)]
pub struct AVFormatContext {
    pub av_class: *const AVClass,
    pub iformat: *const c_void,  // AVInputFormat
    pub oformat: *const c_void,  // AVOutputFormat
    pub priv_data: *mut c_void,
    pub pb: *mut AVIOContext,
    pub ctx_flags: c_int,
    pub nb_streams: c_uint,
    pub streams: *mut *mut AVStream,
    // ... 其他字段省略
}

// === AVIOContext（I/O 上下文） ===
#[repr(C)]
pub struct AVIOContext {
    pub av_class: *const AVClass,
    // ... 其他字段省略
}

// === AVCodec（编解码器） ===
#[repr(C)]
pub struct AVCodec {
    pub av_class: *const AVClass,
    pub name: *const c_char,
    pub long_name: *const c_char,
    pub type_: AVMediaType,
    pub id: AVCodecID,
    // ... 其他字段省略
}

// === AVFilterContext（滤镜上下文） ===
#[repr(C)]
pub struct AVFilterContext {
    pub av_class: *const AVClass,
    pub filter: *const c_void,  // AVFilter
    pub name: *mut c_char,
    // ... 其他字段省略
}

// === AVFilterGraph（滤镜图） ===
#[repr(C)]
pub struct AVFilterGraph {
    pub av_class: *const AVClass,
    pub nb_filters: c_uint,
    pub filters: *mut *mut AVFilterContext,
    // ... 其他字段省略
}

// === AVFilterInOut（滤镜输入输出） ===
#[repr(C)]
pub struct AVFilterInOut {
    pub name: *mut c_char,
    pub filter_ctx: *mut AVFilterContext,
    pub pad_idx: c_int,
    pub next: *mut AVFilterInOut,
}

// === 外部函数声明 ===

// libavcodec
#[link(name = "avcodec.61", kind = "dylib")]
extern "C" {
    pub fn avcodec_find_encoder(codec_id: AVCodecID) -> *const AVCodec;
    pub fn avcodec_find_decoder(codec_id: AVCodecID) -> *const AVCodec;
    pub fn avcodec_alloc_context3(codec: *const AVCodec) -> *mut AVCodecContext;
    pub fn avcodec_free_context(ctx: *mut *mut AVCodecContext);
    pub fn avcodec_open2(ctx: *mut AVCodecContext, codec: *const AVCodec, options: *mut *mut c_void) -> c_int;
    pub fn avcodec_send_frame(ctx: *mut AVCodecContext, frame: *const AVFrame) -> c_int;
    pub fn avcodec_receive_packet(ctx: *mut AVCodecContext, pkt: *mut AVPacket) -> c_int;
    pub fn avcodec_send_packet(ctx: *mut AVCodecContext, pkt: *const AVPacket) -> c_int;
    pub fn avcodec_receive_frame(ctx: *mut AVCodecContext, frame: *mut AVFrame) -> c_int;
    pub fn avcodec_parameters_to_context(ctx: *mut AVCodecContext, par: *const AVCodecParameters) -> c_int;
    pub fn avcodec_parameters_from_context(par: *mut AVCodecParameters, ctx: *const AVCodecContext) -> c_int;
}

// libavformat
#[link(name = "avformat.61", kind = "dylib")]
extern "C" {
    pub fn avformat_open_input(
        ctx: *mut *mut AVFormatContext,
        url: *const c_char,
        fmt: *const c_void,
        options: *mut *mut c_void,
    ) -> c_int;
    pub fn avformat_close_input(ctx: *mut *mut AVFormatContext);
    pub fn avformat_find_stream_info(ctx: *mut AVFormatContext, options: *mut *mut c_void) -> c_int;
    pub fn avformat_alloc_output_context2(
        ctx: *mut *mut AVFormatContext,
        ofmt: *const c_void,
        format_name: *const c_char,
        filename: *const c_char,
    ) -> c_int;
    pub fn avformat_free_context(ctx: *mut AVFormatContext);
    pub fn avformat_new_stream(ctx: *mut AVFormatContext, codec: *const c_void) -> *mut AVStream;
    pub fn avformat_write_header(ctx: *mut AVFormatContext, options: *mut *mut c_void) -> c_int;
    pub fn av_write_frame(ctx: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;
    pub fn av_interleaved_write_frame(ctx: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;
    pub fn av_write_trailer(ctx: *mut AVFormatContext) -> c_int;
    pub fn av_read_frame(ctx: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;
}

// libavutil
#[link(name = "avutil.59", kind = "dylib")]
extern "C" {
    pub fn av_frame_alloc() -> *mut AVFrame;
    pub fn av_frame_free(frame: *mut *mut AVFrame);
    pub fn av_frame_get_buffer(frame: *mut AVFrame, align: c_int) -> c_int;
    pub fn av_frame_make_writable(frame: *mut AVFrame) -> c_int;
    pub fn av_frame_copy(dst: *mut AVFrame, src: *const AVFrame) -> c_int;
    pub fn av_packet_alloc() -> *mut AVPacket;
    pub fn av_packet_free(pkt: *mut *mut AVPacket);
    pub fn av_packet_unref(pkt: *mut AVPacket);
    pub fn av_strdup(s: *const c_char) -> *mut c_char;
    pub fn av_free(ptr: *mut c_void);
    pub fn av_strerror(errnum: c_int, errbuf: *mut c_char, errbuf_size: usize) -> c_int;
}

// libswscale
#[link(name = "swscale.8", kind = "dylib")]
extern "C" {
    pub fn sws_getContext(
        srcW: c_int,
        srcH: c_int,
        srcFormat: AVPixelFormat,
        dstW: c_int,
        dstH: c_int,
        dstFormat: AVPixelFormat,
        flags: c_int,
        srcFilter: *const c_void,
        dstFilter: *const c_void,
        param: *const f64,
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

// libavfilter
#[link(name = "avfilter.10", kind = "dylib")]
extern "C" {
    pub fn avfilter_graph_alloc() -> *mut AVFilterGraph;
    pub fn avfilter_graph_free(graph: *mut *mut AVFilterGraph);
    pub fn avfilter_graph_parse_ptr(
        graph: *mut AVFilterGraph,
        filters: *const c_char,
        inputs: *mut *mut AVFilterInOut,
        outputs: *mut *mut AVFilterInOut,
        log_ctx: *mut c_void,
    ) -> c_int;
    pub fn avfilter_graph_config(graph: *mut AVFilterGraph, log_ctx: *mut c_void) -> c_int;
    pub fn avfilter_inout_alloc() -> *mut AVFilterInOut;
    pub fn avfilter_inout_free(inout: *mut *mut AVFilterInOut);
    pub fn avfilter_get_by_name(name: *const c_char) -> *const c_void;
    pub fn avfilter_graph_create_filter(
        filter_ctx: *mut *mut AVFilterContext,
        filter: *const c_void,
        name: *const c_char,
        args: *const c_char,
        kwargs: *mut c_void,
        graph_ctx: *mut AVFilterGraph,
    ) -> c_int;
    pub fn av_buffersrc_write_frame(ctx: *mut AVFilterContext, frame: *const AVFrame) -> c_int;
    pub fn av_buffersrc_add_frame_flags(ctx: *mut AVFilterContext, frame: *const AVFrame, flags: c_int) -> c_int;
    pub fn av_buffersink_get_frame(ctx: *mut AVFilterContext, frame: *mut AVFrame) -> c_int;
}

// libavio (通过 libavformat 链接)
extern "C" {
    pub fn avio_open(ctx: *mut *mut AVIOContext, url: *const c_char, flags: c_int) -> c_int;
    pub fn avio_closep(ctx: *mut *mut AVIOContext) -> c_int;
}

// === 辅助函数 ===
pub fn av_err2str(errnum: c_int) -> String {
    let mut errbuf = [0u8; 128];
    unsafe {
        av_strerror(errnum, errbuf.as_mut_ptr() as *mut c_char, errbuf.len());
        String::from_utf8_lossy(&errbuf).trim_end_matches('\0').to_string()
    }
}
