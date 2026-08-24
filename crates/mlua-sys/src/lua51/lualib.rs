//! Contains definitions from `lualib.h`.

use std::os::raw::{c_char};

pub const LUA_COLIBNAME: *const c_char = cstr!("coroutine");
pub const LUA_TABLIBNAME: *const c_char = cstr!("table");
pub const LUA_IOLIBNAME: *const c_char = cstr!("io");
pub const LUA_OSLIBNAME: *const c_char = cstr!("os");
pub const LUA_STRLIBNAME: *const c_char = cstr!("string");
pub const LUA_MATHLIBNAME: *const c_char = cstr!("math");
pub const LUA_DBLIBNAME: *const c_char = cstr!("debug");
pub const LUA_LOADLIBNAME: *const c_char = cstr!("package");

#[cfg(feature = "luajit")]
pub const LUA_BITLIBNAME: *const c_char = cstr!("bit");
#[cfg(feature = "luajit")]
pub const LUA_JITLIBNAME: *const c_char = cstr!("jit");
#[cfg(feature = "luajit")]
pub const LUA_FFILIBNAME: *const c_char = cstr!("ffi");
