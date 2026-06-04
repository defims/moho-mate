//! Lua 5.4.4 FFI 绑定
//!
//! 直接声明 Lua C API，不使用 mlua
//! 与 Moho 的 Lua state 完全兼容

use std::os::raw::{c_int, c_void, c_char};

pub type lua_State = *mut c_void;
pub type lua_CFunction = Option<unsafe extern "C" fn(lua_State) -> c_int>;

// Lua 类型常量
pub const LUA_TNONE: c_int = -1;
pub const LUA_TNIL: c_int = 0;
pub const LUA_TBOOLEAN: c_int = 1;
pub const LUA_TLIGHTUSERDATA: c_int = 2;
pub const LUA_TNUMBER: c_int = 3;
pub const LUA_TSTRING: c_int = 4;
pub const LUA_TTABLE: c_int = 5;
pub const LUA_TFUNCTION: c_int = 6;
pub const LUA_TUSERDATA: c_int = 7;
pub const LUA_TTHREAD: c_int = 8;

// Lua 注册表索引
pub const LUA_REGISTRYINDEX: c_int = -1001000;

// ========== 栈操作 ==========

extern "C" {
    // 栈大小
    pub fn lua_gettop(L: lua_State) -> c_int;
    pub fn lua_settop(L: lua_State, idx: c_int);
    pub fn lua_pushvalue(L: lua_State, idx: c_int);

    // 类型检查
    pub fn lua_type(L: lua_State, idx: c_int) -> c_int;
    pub fn lua_typename(L: lua_State, tp: c_int) -> *const c_char;
    pub fn lua_isnumber(L: lua_State, idx: c_int) -> c_int;
    pub fn lua_isstring(L: lua_State, idx: c_int) -> c_int;
    pub fn lua_isuserdata(L: lua_State, idx: c_int) -> c_int;

    // 获取值（实际函数名，不是宏）
    pub fn lua_toboolean(L: lua_State, idx: c_int) -> c_int;
    pub fn lua_tointegerx(L: lua_State, idx: c_int, isnum: *mut c_int) -> i64;
    pub fn lua_tonumberx(L: lua_State, idx: c_int, isnum: *mut c_int) -> f64;
    pub fn lua_tolstring(L: lua_State, idx: c_int, len: *mut usize) -> *const c_char;
    pub fn lua_topointer(L: lua_State, idx: c_int) -> *const c_void;
    pub fn lua_touserdata(L: lua_State, idx: c_int) -> *mut c_void;

    // 表操作
    pub fn lua_createtable(L: lua_State, narr: c_int, nrec: c_int);
    pub fn lua_newuserdatauv(L: lua_State, sz: usize, nuvalue: c_int) -> *mut c_void;
    pub fn lua_getmetatable(L: lua_State, objindex: c_int) -> c_int;
    pub fn lua_setmetatable(L: lua_State, objindex: c_int);

    // 压栈
    pub fn lua_pushnil(L: lua_State);
    pub fn lua_pushnumber(L: lua_State, n: f64);
    pub fn lua_pushinteger(L: lua_State, n: i64);
    pub fn lua_pushlstring(L: lua_State, s: *const c_char, len: usize);
    pub fn lua_pushstring(L: lua_State, s: *const c_char);
    pub fn lua_pushcclosure(L: lua_State, fn_: lua_CFunction, n: c_int);
    pub fn lua_pushboolean(L: lua_State, b: c_int);
    pub fn lua_pushlightuserdata(L: lua_State, p: *mut c_void);
    pub fn lua_pushthread(L: lua_State) -> c_int;

    // 表访问
    pub fn lua_getglobal(L: lua_State, name: *const c_char);
    pub fn lua_getfield(L: lua_State, idx: c_int, k: *const c_char);
    pub fn lua_geti(L: lua_State, idx: c_int, n: i64);
    pub fn lua_setglobal(L: lua_State, name: *const c_char);
    pub fn lua_setfield(L: lua_State, idx: c_int, k: *const c_char);
    pub fn lua_seti(L: lua_State, idx: c_int, n: i64);
    pub fn lua_settable(L: lua_State, idx: c_int);
    pub fn lua_gettable(L: lua_State, idx: c_int);
    pub fn lua_rawget(L: lua_State, idx: c_int);
    pub fn lua_rawset(L: lua_State, idx: c_int);

    // 执行 (Lua 5.4 使用 k 后缀的函数)
    pub fn luaL_loadstring(L: lua_State, s: *const c_char) -> c_int;
    pub fn lua_pcallk(L: lua_State, nargs: c_int, nresults: c_int, errfunc: c_int, ctx: usize, k: lua_CFunction) -> c_int;
    pub fn lua_callk(L: lua_State, nargs: c_int, nresults: c_int, ctx: usize, k: lua_CFunction);

    // 辅助库
    pub fn luaL_checkstack(L: lua_State, sz: c_int, msg: *const c_char);
    pub fn luaL_checktype(L: lua_State, arg: c_int, t: c_int);
    pub fn luaL_checklstring(L: lua_State, arg: c_int, l: *mut usize) -> *const c_char;
    pub fn luaL_checknumber(L: lua_State, arg: c_int) -> f64;
    pub fn luaL_checkinteger(L: lua_State, arg: c_int) -> i64;
    pub fn luaL_optlstring(L: lua_State, arg: c_int, def: *const c_char, l: *mut usize) -> *const c_char;
    pub fn luaL_optnumber(L: lua_State, arg: c_int, def: f64) -> f64;
    pub fn luaL_optinteger(L: lua_State, arg: c_int, d: i64) -> i64;
    pub fn luaL_ref(L: lua_State, t: c_int) -> c_int;
    pub fn luaL_unref(L: lua_State, t: c_int, ref_: c_int);
}

// ========== Lua 5.4 兼容宏 ==========

/// lua_pcall 是宏，实际调用 lua_pcallk
#[inline]
pub unsafe fn lua_pcall(L: lua_State, nargs: c_int, nresults: c_int, errfunc: c_int) -> c_int {
    lua_pcallk(L, nargs, nresults, errfunc, 0, None)
}

/// lua_call 是宏，实际调用 lua_callk
#[inline]
pub unsafe fn lua_call(L: lua_State, nargs: c_int, nresults: c_int) {
    lua_callk(L, nargs, nresults, 0, None)
}

/// lua_pushcfunction 是宏，实际调用 lua_pushcclosure
#[inline]
pub unsafe fn lua_pushcfunction(L: lua_State, f: lua_CFunction) {
    lua_pushcclosure(L, f, 0);
}

/// lua_tostring 是宏，实际调用 lua_tolstring
#[inline]
pub unsafe fn lua_tostring(L: lua_State, idx: c_int) -> *const c_char {
    lua_tolstring(L, idx, std::ptr::null_mut())
}

/// lua_tointeger 是宏，实际调用 lua_tointegerx
#[inline]
pub unsafe fn lua_tointeger(L: lua_State, idx: c_int) -> i64 {
    lua_tointegerx(L, idx, std::ptr::null_mut())
}

/// lua_tonumber 是宏，实际调用 lua_tonumberx
#[inline]
pub unsafe fn lua_tonumber(L: lua_State, idx: c_int) -> f64 {
    lua_tonumberx(L, idx, std::ptr::null_mut())
}

/// lua_newuserdata 是宏，实际调用 lua_newuserdatauv
#[inline]
pub unsafe fn lua_newuserdata(L: lua_State, sz: usize) -> *mut c_void {
    lua_newuserdatauv(L, sz, 1)
}

// ========== Rust 辅助函数 ==========

/// 安全压入 Rust 字符串
pub unsafe fn push_string(L: lua_State, s: &str) {
    lua_pushlstring(L, s.as_ptr() as *const c_char, s.len());
}

/// 安全获取 Lua 字符串
pub unsafe fn to_string(L: lua_State, idx: c_int) -> Option<&'static str> {
    let ptr = lua_tolstring(L, idx, std::ptr::null_mut());
    if ptr.is_null() {
        None
    } else {
        Some(std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or(""))
    }
}
