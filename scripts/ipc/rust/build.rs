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
        // CoreFoundation (CFRunLoop)
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        // libdispatch (GCD) - 在 macOS 上是 libSystem 的一部分
        println!("cargo:rustc-link-lib=dylib=System");
    }
    
    // FFmpeg 内置库配置
    #[cfg(feature = "ffmpeg-builtin")]
    {
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let ipc_dir = manifest_dir.parent().unwrap();
        let scripts_dir = ipc_dir.parent().unwrap();
        let moho_fw = "/Applications/Moho.app/Contents/Frameworks";
        
        // 告诉 cargo 链接路径
        println!("cargo:rustc-link-search=native={}", scripts_dir.display());
        println!("cargo:rustc-link-search=native={}", moho_fw);
        
        // 链接 FFmpeg 库
        println!("cargo:rustc-link-lib=dylib=avfilter.10");
        println!("cargo:rustc-link-lib=dylib=avcodec.61");
        println!("cargo:rustc-link-lib=dylib=avformat.61");
        println!("cargo:rustc-link-lib=dylib=avutil.59");
        println!("cargo:rustc-link-lib=dylib=swscale.8");
        println!("cargo:rustc-link-lib=dylib=swresample.5");
        
        // 设置 rpath
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", scripts_dir.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", moho_fw);
    }
    
    // 重新构建条件
    println!("cargo:rerun-if-changed=build.rs");
}
