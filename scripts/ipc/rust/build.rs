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
        
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", moho_fw);
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", scripts_dir);
    }
    
    println!("cargo:rerun-if-changed=build.rs");
}