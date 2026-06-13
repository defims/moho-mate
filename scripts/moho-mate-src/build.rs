use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap(); // msvc or gnu
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    
    // 检查是否启用 ffmpeg-builtin feature
    let ffmpeg_builtin = std::env::var("CARGO_FEATURE_FFMPEG_BUILTIN").is_ok();
    
    // ============================================================
    // Lua 库配置
    // ============================================================
    // 
    // 根据目标平台和架构选择正确的 Lua 库目录
    // 
    // 目录结构：
    //   lua-src/
    //   ├── src/           # Lua 源码
    //   ├── lib-x64/       # macOS x86_64 / Linux x86_64
    //   ├── lib-arm64/     # macOS Apple Silicon
    //   ├── lib-msvc/      # Windows MSVC
    //   └── lib-mingw/     # Windows MinGW
    //
    // 注意：MSVC 的库文件名是 lua.lib，其他平台是 liblua.a
    //
    // 相关文件：
    //   - src/main.rs: 使用 `use moho_mate::*` 引用库
    //   - src/lib.rs: 定义 Lua FFI 函数
    //
    let lua_dir = manifest_dir.clone() + "/lua-src";
    let is_msvc = target_os == "windows" && target_env == "msvc";
    
    let lua_lib_dir = if target_os == "windows" {
        // Windows: 区分 MSVC 和 MinGW
        if target_env == "msvc" {
            lua_dir.clone() + "/lib-msvc"
        } else {
            lua_dir.clone() + "/lib-mingw"
        }
    } else if target_os == "macos" {
        // macOS 按架构分目录
        let arch_dir = if target_arch == "aarch64" { "lib-arm64" } else { "lib-x64" };
        lua_dir.clone() + "/" + arch_dir
    } else {
        lua_dir.clone() + "/lib"
    };
    
    // 检查 Lua 库是否存在，如果不存在则编译
    // 注意：MSVC 的库文件名是 lua.lib，其他平台是 liblua.a
    let lib_name = if is_msvc { "lua.lib" } else { "liblua.a" };
    let lua_lib_path = Path::new(&lua_lib_dir).join(lib_name);
    
    if !lua_lib_path.exists() {
        compile_lua_with_cc(&target_os, &target_arch, &lua_dir, &lua_lib_dir, is_msvc);
    }
    
    // 告诉 cargo 链接 Lua 静态库
    println!("cargo:rustc-link-search=native={}", lua_lib_dir);
    println!("cargo:rustc-link-lib=static=lua");
    
    // 链接系统库
    if target_os == "macos" {
        // macOS: 导出符号 + 系统库
        println!("cargo:rustc-link-arg=-Wl,-export_dynamic");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=dylib=System");
        
        // ffmpeg-builtin: 链接 Moho 内置的 FFmpeg 库
        if ffmpeg_builtin {
            link_moho_ffmpeg();
        }
    } else if target_os == "windows" {
        // Windows: 需要链接一些系统库
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=kernel32");
    }
    
    println!("cargo:rerun-if-changed=build.rs");
}

/// 使用 cc crate 编译 Lua（自动处理 MSVC 环境）
fn compile_lua_with_cc(target_os: &str, target_arch: &str, lua_src_dir: &str, lua_lib_dir: &str, _is_msvc: bool) {
    println!("cargo:warning=Lua 库不存在，开始编译（使用 cc crate）...");
    println!("cargo:warning=目标平台: {} ({})", target_os, target_arch);
    println!("cargo:warning=源码目录: {}", lua_src_dir);
    println!("cargo:warning=输出目录: {}", lua_lib_dir);
    
    // 创建输出目录
    fs::create_dir_all(lua_lib_dir).expect("Failed to create lua lib dir");
    
    // 进入 Lua 源码目录
    let src_dir = Path::new(lua_src_dir).join("src");
    if !src_dir.exists() {
        panic!("Lua 源码目录不存在: {:?}", src_dir);
    }
    
    // Lua 源文件列表
    let lua_sources = [
        "lapi.c", "lauxlib.c", "lbaselib.c", "lcode.c", "lcorolib.c",
        "lctype.c", "ldblib.c", "ldebug.c", "ldo.c", "ldump.c",
        "lfunc.c", "lgc.c", "linit.c", "liolib.c", "llex.c",
        "lmathlib.c", "lmem.c", "loadlib.c", "lobject.c", "lopcodes.c",
        "loslib.c", "lparser.c", "lstate.c", "lstring.c", "lstrlib.c",
        "ltable.c", "ltablib.c", "ltm.c", "lundump.c", "lutf8lib.c",
        "lvm.c", "lzio.c",
    ];
    
    // 使用 cc::Build 编译
    let mut build = cc::Build::new();
    
    // 设置编译器标志
    if target_os == "windows" {
        build.define("LUA_USE_WINDOWS", None);
    } else if target_os == "macos" {
        build.define("LUA_USE_MACOSX", None);
        
        // 交叉编译：指定目标架构
        if target_arch == "aarch64" {
            build.flag("-target").flag("arm64-apple-macos11");
        } else if target_arch == "x86_64" {
            build.flag("-target").flag("x86_64-apple-macos10.12");
        }
    } else {
        build.define("LUA_USE_LINUX", None);
    }
    
    // 添加源文件
    for src in &lua_sources {
        let src_path = src_dir.join(src);
        if src_path.exists() {
            build.file(&src_path);
        }
    }
    
    // 设置包含路径
    build.include(&src_dir);
    
    // 编译
    let lib_path = Path::new(lua_lib_dir).join("liblua.a");
    build.compile("lua");
    
    // cc::Build 会自动生成库文件到 OUT_DIR，我们需要复制到目标目录
    // 获取 OUT_DIR
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_lib = Path::new(&out_dir).join("liblua.a");
    
    // 复制库文件到目标目录
    if out_lib.exists() {
        fs::copy(&out_lib, &lib_path).expect("Failed to copy liblua.a");
        println!("cargo:warning=Lua 编译完成: {:?}", lib_path);
    } else {
        // MSVC 可能生成 lua.lib
        let out_lib_msvc = Path::new(&out_dir).join("lua.lib");
        let lib_path_msvc = Path::new(lua_lib_dir).join("lua.lib");
        if out_lib_msvc.exists() {
            fs::copy(&out_lib_msvc, &lib_path_msvc).expect("Failed to copy lua.lib");
            println!("cargo:warning=Lua 编译完成: {:?}", lib_path_msvc);
        } else {
            println!("cargo:warning=Lua 编译完成（库在 OUT_DIR）");
        }
    }
}

/// 链接 Moho 内置的 FFmpeg 库
/// 
/// ## 最终方案：scripts 目录库符号链接
/// 
/// 符号链接在 scripts 目录，指向 Moho Frameworks：
/// ```text
/// scripts/libavcodec.61.dylib -> /Applications/Moho.app/.../libavcodec.61.dylib
/// ```
/// 
/// ## 操作步骤（在 build.sh 中执行）
/// 
/// 1. 创建库符号链接（5 个）
/// 2. 使用 install_name_tool 修改库引用路径
///    `@executable_path/../Frameworks/` → `@executable_path/`
/// 
/// ## 关键点
/// 
/// `@loader_path` 解析为真实文件所在目录（Moho Frameworks），
/// 所以库之间的依赖自动解决。
/// 
/// ## 相关文件
/// 
/// - build.sh: 创建符号链接 + 修改库引用路径
/// - ffmpeg_ffi.rs: FFmpeg FFI 绑定
/// - encode_native.rs: FFmpeg 编码实现
fn link_moho_ffmpeg() {
    let moho_frameworks = "/Applications/Moho.app/Contents/Frameworks";
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let scripts_dir = manifest_dir.clone() + "/.."; // scripts/ 目录
    
    // 检查 Moho Frameworks 目录
    if !Path::new(moho_frameworks).exists() {
        println!("cargo:warning=Moho Frameworks 目录不存在: {}", moho_frameworks);
        return;
    }
    
    // 检查 scripts 目录中的 libavfilter
    let libavfilter_path = Path::new(&scripts_dir).join("libavfilter.10.dylib");
    if !libavfilter_path.exists() {
        println!("cargo:warning=libavfilter 不存在: {:?}", libavfilter_path);
        println!("cargo:warning=请确保 scripts/ 目录中有 libavfilter.10.dylib");
        return;
    }
    
    // 添加库搜索路径
    // Moho 内置库
    println!("cargo:rustc-link-search=native={}", moho_frameworks);
    // scripts 目录的 libavfilter
    println!("cargo:rustc-link-search=native={}", scripts_dir);
    
    // 链接 FFmpeg 库（顺序很重要：依赖关系）
    // avfilter -> avformat -> avcodec -> swscale -> swresample -> avutil
    println!("cargo:rustc-link-lib=dylib=avfilter.10");
    println!("cargo:rustc-link-lib=dylib=avformat.61");
    println!("cargo:rustc-link-lib=dylib=avcodec.61");
    println!("cargo:rustc-link-lib=dylib=swscale.8");
    println!("cargo:rustc-link-lib=dylib=swresample.5");
    println!("cargo:rustc-link-lib=dylib=avutil.59");
    
    // 设置 rpath，让运行时能找到 scripts 目录的 libavfilter
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", scripts_dir);
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", moho_frameworks);
    
    // 生成 post-build 脚本，用 install_name_tool 修改库路径
    // 这在 cargo 构建完成后执行
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let post_build_script = Path::new(&out_dir).join("post-build.sh");
    
    let script_content = format!(r#"#!/bin/bash
# post-build.sh - 修改 FFmpeg 库路径
# 由 build.rs 自动生成

MOHO_MATE="$1"

if [ ! -f "$MOHO_MATE" ]; then
    echo "moho-mate not found: $MOHO_MATE"
    exit 1
fi

# 修改 Moho 内置库的路径为绝对路径
install_name_tool -change "@executable_path/../Frameworks/libavcodec.61.dylib" "/Applications/Moho.app/Contents/Frameworks/libavcodec.61.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libavformat.61.dylib" "/Applications/Moho.app/Contents/Frameworks/libavformat.61.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libavutil.59.dylib" "/Applications/Moho.app/Contents/Frameworks/libavutil.59.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libswscale.8.dylib" "/Applications/Moho.app/Contents/Frameworks/libswscale.8.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libswresample.5.dylib" "/Applications/Moho.app/Contents/Frameworks/libswresample.5.dylib" "$MOHO_MATE" 2>/dev/null || true

echo "✓ FFmpeg 库路径已修改"
"#);
    
    std::fs::write(&post_build_script, script_content).expect("Failed to write post-build script");
    
    println!("cargo:warning=已配置链接 FFmpeg:");
    println!("cargo:warning=  - Moho 内置: {}", moho_frameworks);
    println!("cargo:warning=  - libavfilter: {}", scripts_dir);
    println!("cargo:warning=运行构建后，执行以下命令修改库路径:");
    println!("cargo:warning=  bash {} $PWD/target/release/moho-mate", post_build_script.display());
}
