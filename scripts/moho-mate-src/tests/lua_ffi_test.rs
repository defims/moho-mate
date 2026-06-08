//! Lua FFI 测试
//!
//! 这些测试需要 lua-ffi feature 才能运行
//! 运行: cargo test --features lua-ffi

#[cfg(feature = "lua-ffi")]
use moho_mate::lua_ffi::*;

#[cfg(feature = "lua-ffi")]
#[test]
fn test_lua_state_exists() {
    unsafe {
        // 创建最小 Lua 状态用于测试
        let L = luaL_newstate();
        assert!(!L.is_null(), "luaL_newstate should return non-null pointer");
        
        // 测试基础库加载
        luaL_openlibs(L);
        
        // 测试简单表达式执行
        let result = luaL_dostring(L, c"return 1 + 1".as_ptr());
        assert_eq!(result, 0, "luaL_dostring should succeed");
        
        // 检查结果
        let value = lua_tointeger(L, -1);
        assert_eq!(value, 2, "1 + 1 should equal 2");
        
        // 清理
        lua_close(L);
    }
}

#[cfg(feature = "lua-ffi")]
#[test]
fn test_lua_table_operations() {
    unsafe {
        let L = luaL_newstate();
        luaL_openlibs(L);
        
        // 创建表
        lua_createtable(L, 0, 3);
        assert_eq!(lua_gettop(L), 1, "table should be on stack top");
        
        // 设置字段
        lua_pushstring(L, c"name".as_ptr());
        lua_pushstring(L, c"test".as_ptr());
        lua_settable(L, -3);
        
        // 获取字段
        lua_pushstring(L, c"name".as_ptr());
        lua_gettable(L, -2);
        
        // 检查值
        let s = lua_tostring(L, -1);
        let name = std::ffi::CStr::from_ptr(s).to_str().unwrap();
        assert_eq!(name, "test", "table field should be 'test'");
        
        lua_close(L);
    }
}

#[cfg(feature = "lua-ffi")]
#[test]
fn test_lua_integer_operations() {
    unsafe {
        let L = luaL_newstate();
        luaL_openlibs(L);
        
        // 测试整数推栈
        lua_pushinteger(L, 42);
        assert_eq!(lua_gettop(L), 1, "integer should be pushed");
        assert_eq!(lua_tointeger(L, -1), 42, "integer should be 42");
        
        // 测试负数
        lua_pushinteger(L, -100);
        assert_eq!(lua_tointeger(L, -1), -100, "negative integer should work");
        
        lua_close(L);
    }
}

#[cfg(feature = "lua-ffi")]
#[test]
fn test_lua_boolean_operations() {
    unsafe {
        let L = luaL_newstate();
        
        // 测试布尔推栈
        lua_pushboolean(L, 1);
        assert!(lua_toboolean(L, -1) != 0, "true should be non-zero");
        
        lua_pushboolean(L, 0);
        assert_eq!(lua_toboolean(L, -1), 0, "false should be zero");
        
        lua_close(L);
    }
}

// 非 lua-ffi feature 时的占位测试
#[cfg(not(feature = "lua-ffi"))]
#[test]
fn test_lua_ffi_skipped() {
    // lua-ffi 测试需要启用 feature
    println!("lua-ffi tests skipped (feature not enabled)");
}
