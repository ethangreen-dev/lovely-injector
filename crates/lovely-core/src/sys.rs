use std::ffi::c_void;

use libc::FILE;
use libloading::{Library, Symbol};
use once_cell::sync::Lazy;

pub const LUA_GLOBALSINDEX: isize = -10002;

pub type LuaState = c_void;

#[link(name = "ucrt")]
extern "C" {
    pub fn __acrt_iob_func(fileno: u32) -> *mut FILE;
}

static LUA_LIB: Lazy<Library> = Lazy::new(|| {
    unsafe {
        Library::new("lua51.dll").unwrap()
    }
});

pub static lua_pcall: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, isize, isize) -> isize>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_pcall").unwrap()
    }
});

pub static lua_getfield: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, *const char)>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_getfield").unwrap()
    }
});

pub static lua_setfield: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, *const char)>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_setfield").unwrap()
    }
});

pub static lua_gettop: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState) -> isize>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_gettop").unwrap()
    }
});

pub static lua_settop: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> isize>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_settop").unwrap()
    }
});
