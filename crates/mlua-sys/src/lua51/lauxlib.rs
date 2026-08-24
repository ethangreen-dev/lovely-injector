//! Contains definitions from `lauxlib.h`.51/

use std::os::raw::{c_char, c_int};
use std::ptr;
use crate::*;

use super::lua::{self, lua_CFunction, lua_State};

// Extra error code for 'luaL_load'
pub const LUA_ERRFILE: c_int = lua::LUA_ERRERR + 1;

// Key, in the registry, for table of loaded modules
pub const LUA_LOADED_TABLE: *const c_char = cstr!("_LOADED");

#[repr(C)]
pub struct luaL_Reg {
    pub name: *const c_char,
    pub func: lua_CFunction,
}


// Pre-defined references
pub const LUA_NOREF: c_int = -2;
pub const LUA_REFNIL: c_int = -1;

//
// Some useful macros (implemented as Rust functions)
//

#[inline(always)]
pub unsafe fn luaL_argcheck(L: *mut lua_State, cond: c_int, narg: c_int, extramsg: *const c_char) {
    if cond == 0 {
        luaL_argerror(L, narg, extramsg);
    }
}

#[inline(always)]
pub unsafe fn luaL_checkstring(L: *mut lua_State, n: c_int) -> *const c_char {
    luaL_checklstring(L, n, ptr::null_mut())
}

#[inline(always)]
pub unsafe fn luaL_optstring(L: *mut lua_State, n: c_int, d: *const c_char) -> *const c_char {
    luaL_optlstring(L, n, d, ptr::null_mut())
}

#[inline(always)]
pub unsafe fn luaL_typename(L: *mut lua_State, i: c_int) -> *const c_char {
    lua::lua_typename(L, lua::lua_type(L, i))
}

pub unsafe fn luaL_dofile(L: *mut lua_State, filename: *const c_char) -> c_int {
    let status = luaL_loadfile(L, filename);
    if status == 0 {
        lua::lua_pcall(L, 0, lua::LUA_MULTRET, 0)
    } else {
        status
    }
}

#[inline(always)]
pub unsafe fn luaL_dostring(L: *mut lua_State, s: *const c_char) -> c_int {
    let status = luaL_loadstring(L, s);
    if status == 0 {
        lua::lua_pcall(L, 0, lua::LUA_MULTRET, 0)
    } else {
        status
    }
}

#[inline(always)]
pub unsafe fn luaL_getmetatable(L: *mut lua_State, n: *const c_char) {
    lua::lua_getfield_(L, lua::LUA_REGISTRYINDEX, n);
}

#[inline(always)]
pub unsafe fn luaL_opt<T>(
    L: *mut lua_State,
    f: unsafe extern "C-unwind" fn(*mut lua_State, c_int) -> T,
    n: c_int,
    d: T,
) -> T {
    if lua::lua_isnoneornil(L, n) != 0 {
        d
    } else {
        f(L, n)
    }
}

//
// Generic Buffer Manipulation
//

#[cfg(target_arch = "wasm32")]
const BUFSIZ: usize = 1024; // WASI libc's BUFSIZ is 1024
#[cfg(not(target_arch = "wasm32"))]
const BUFSIZ: usize = libc::BUFSIZ as usize;

// The buffer size used by the lauxlib buffer system.
// The "16384" workaround is taken from the LuaJIT source code.
pub const LUAL_BUFFERSIZE: usize = if BUFSIZ > 16384 { 8192 } else { BUFSIZ };

#[repr(C)]
pub struct luaL_Buffer {
    pub p: *mut c_char, // current position in buffer
    pub lvl: c_int,     // number of strings in the stack
    pub L: *mut lua_State,
    pub buffer: [c_char; LUAL_BUFFERSIZE],
}

#[inline(always)]
pub unsafe fn luaL_addchar(B: *mut luaL_Buffer, c: c_char) {
    let buffer_end = (*B).buffer.as_mut_ptr().add(LUAL_BUFFERSIZE);
    if (*B).p >= buffer_end {
        luaL_prepbuffer(B);
    }
    *(*B).p = c;
    (*B).p = (*B).p.add(1);
}

#[inline(always)]
pub unsafe fn luaL_addsize(B: *mut luaL_Buffer, n: usize) {
    (*B).p = (*B).p.add(n);
}
