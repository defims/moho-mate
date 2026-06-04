fn main() {
    let lua_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap() + "/lua-src";
    let lua_lib = lua_dir.clone() + "/lib";
    
    // 告诉 cargo 链接 Lua 静态库
    println!("cargo:rustc-link-search=native={}", lua_lib);
    println!("cargo:rustc-link-lib=static=lua");
    
    // 导出 luaopen_moho_ipc 符号（让 dlopen 能找到）
    println!("cargo:rustc-link-arg=-Wl,-export_dynamic");
    
    // 重新构建条件
    println!("cargo:rerun-if-changed=build.rs");
}
