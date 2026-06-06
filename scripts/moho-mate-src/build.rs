use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let lua_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap() + "/lua-src";
    let lua_lib = lua_dir.clone() + "/lib";
    
    // 告诉 cargo 链接 Lua 静态库
    println!("cargo:rustc-link-search=native={}", lua_lib);
    println!("cargo:rustc-link-lib=static=lua");
    
    // 导出 luaopen_moho_ipc 符号
    println!("cargo:rustc-link-arg=-Wl,-export_dynamic");
    
    // 链接系统库
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=dylib=System");
    }
    
    // FFmpeg 内置库配置
    #[cfg(feature = "ffmpeg-builtin")]
    {
        let moho_fw = "/Applications/Moho.app/Contents/Frameworks";
        let scripts_dir = "/Users/def/.openclaw/workspace/skills/moho-mate/scripts";
        
        println!("cargo:rustc-link-search=native={}", moho_fw);
        println!("cargo:rustc-link-search=native={}", scripts_dir);
        
        println!("cargo:rustc-link-lib=dylib=avfilter.10");
        println!("cargo:rustc-link-lib=dylib=avcodec.61");
        println!("cargo:rustc-link-lib=dylib=avformat.61");
        println!("cargo:rustc-link-lib=dylib=avutil.59");
        println!("cargo:rustc-link-lib=dylib=swscale.8");
        println!("cargo:rustc-link-lib=dylib=swresample.5");
        
        // 设置 LC_RPATH，让二进制能找到 Moho 的 FFmpeg 库
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", moho_fw);
        
        // 使用 @rpath 替代 @executable_path，这样运行时会在 rpath 中搜索
        // macOS 默认使用 @executable_path/../Frameworks，需要改成 @rpath
        println!("cargo:rustc-link-arg=-Wl,-headerpad_max_install_names");
    }
    
    // 生成 FFmpeg FFI 绑定（可选，仅在 ffmpeg-builtin feature 且 bindgen 可用时）
    #[cfg(feature = "ffmpeg-builtin")]
    {
        if env::var("CARGO_FEATURE_FFMPEG_BUILTIN").is_ok() {
            generate_ffmpeg_bindings();
        }
    }
    
    println!("cargo:rerun-if-changed=build.rs");
}

/// 生成 FFmpeg FFI 绑定
/// 
/// 原理：
/// 1. 下载 FFmpeg 源码头文件（指定版本）
/// 2. 用 bindgen 库生成 Rust 绑定
/// 3. 输出到 OUT_DIR/ffmpeg_bindings.rs
/// 
/// 使用：
/// - 首次运行或 FFmpeg 版本更新时自动下载头文件
/// - 生成的绑定在编译时可用：include!(concat!(env!("OUT_DIR"), "/ffmpeg_bindings.rs"));
#[cfg(feature = "ffmpeg-builtin")]
fn generate_ffmpeg_bindings() {
    use bindgen::Builder;
    
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let bindings_path = Path::new(&out_dir).join("ffmpeg_bindings.rs");
    let ffmpeg_headers_dir = Path::new(&manifest_dir).join("ffmpeg-headers");
    let ffmpeg_version = "n7.1";  // FFmpeg 版本，与 Moho 内置版本对应
    
    // 检查是否需要重新生成
    if bindings_path.exists() {
        println!("cargo:warning=FFmpeg bindings already exist, skipping generation");
        return;
    }
    
    // 下载 FFmpeg 头文件（如果不存在）
    let ffmpeg_src_dir = ffmpeg_headers_dir.join(format!("FFmpeg-{}", ffmpeg_version));
    if !ffmpeg_src_dir.exists() {
        fs::create_dir_all(&ffmpeg_headers_dir).expect("Failed to create ffmpeg-headers dir");
        
        let tar_path = ffmpeg_headers_dir.join("ffmpeg.tar.gz");
        let tar_url = format!(
            "https://github.com/FFmpeg/FFmpeg/archive/refs/tags/{}.tar.gz",
            ffmpeg_version
        );
        
        println!("cargo:warning=Downloading FFmpeg {} headers...", ffmpeg_version);
        
        // 使用 curl 下载
        let status = std::process::Command::new("curl")
            .args(["-L", "-o", tar_path.to_str().unwrap(), &tar_url])
            .status()
            .expect("Failed to run curl");
        
        if !status.success() {
            println!("cargo:warning=Failed to download FFmpeg headers from {}", tar_url);
            println!("cargo:warning=FFI bindings will not be updated. Using existing bindings if available.");
            return;
        }
        
        // 解压
        println!("cargo:warning=Extracting FFmpeg headers...");
        let status = std::process::Command::new("tar")
            .args(["xzf", tar_path.to_str().unwrap(), "-C", ffmpeg_headers_dir.to_str().unwrap()])
            .status()
            .expect("Failed to run tar");
        
        if !status.success() {
            println!("cargo:warning=Failed to extract FFmpeg headers");
            println!("cargo:warning=FFI bindings will not be updated.");
            return;
        }
        
        // 删除 tar 包
        let _ = fs::remove_file(&tar_path);
    }
    
    // 创建最小配置文件（bindgen 需要）
    let avconfig_path = ffmpeg_src_dir.join("libavutil/avconfig.h");
    if !avconfig_path.exists() {
        fs::write(&avconfig_path, "// Minimal config for bindgen\n")
            .expect("Failed to create avconfig.h");
    }
    
    // 生成绑定（使用 bindgen 库）
    println!("cargo:warning=Generating FFmpeg FFI bindings...");
    
    // 需要生成的头文件
    let headers = [
        ("libavcodec/avcodec.h", "libavcodec_bindings.rs"),
        ("libavformat/avformat.h", "libavformat_bindings.rs"), 
        ("libavutil/avutil.h", "libavutil_bindings.rs"),
        ("libavutil/frame.h", "libavutil_frame_bindings.rs"),
        ("libswscale/swscale.h", "libswscale_bindings.rs"),
    ];
    
    let clang_args = vec![format!("-I{}", ffmpeg_src_dir.display())];
    
    for (header, output_name) in &headers {
        let header_path = ffmpeg_src_dir.join(header);
        if !header_path.exists() {
            continue;
        }
        
        let output_path = Path::new(&out_dir).join(output_name);
        
        let bindings = Builder::default()
            .header(header_path.to_str().unwrap())
            .clang_args(&clang_args)
            .generate();
        
        match bindings {
            Ok(b) => {
                b.write_to_file(&output_path)
                    .expect("Failed to write bindings");
                println!("cargo:warning=Generated: {}", output_name);
            }
            Err(e) => {
                println!("cargo:warning=Failed to generate bindings for {}: {:?}", header, e);
            }
        }
    }
    
    // 生成一个汇总文件
    let summary_path = Path::new(&out_dir).join("ffmpeg_bindings.rs");
    let mut summary_content = String::new();
    summary_content.push_str("// Auto-generated FFmpeg FFI bindings\n");
    summary_content.push_str("// Generated by build.rs using bindgen library\n\n");
    
    for (_, output_name) in &headers {
        let binding_path = Path::new(&out_dir).join(output_name);
        if binding_path.exists() {
            summary_content.push_str(&format!("mod {} {{\n", 
                output_name.replace(".rs", "")));
            summary_content.push_str(&format!("    include!(\"{}\");\n", output_name));
            summary_content.push_str("}\n\n");
        }
    }
    
    fs::write(&summary_path, summary_content).expect("Failed to write summary");
    println!("cargo:warning=FFmpeg bindings generated successfully");
}
