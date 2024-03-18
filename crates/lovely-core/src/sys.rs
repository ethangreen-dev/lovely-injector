use std::ffi::{c_void, CString};

use libc::FILE;
use libloading::{Library, Symbol};
use once_cell::sync::Lazy;

pub const LUA_GLOBALSINDEX: isize = -10002;

pub const LUA_TNIL: isize = 0;
pub const LUA_TBOOLEAN: isize = 1;

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

pub static lua_call: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, isize)>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_call").unwrap()
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

pub static lua_pushvalue: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize)>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_pushvalue").unwrap()
    }
});

pub static lua_pushcclosure: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, *const c_void, isize)>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_pushcclosure").unwrap()
    }
});

pub static lua_tolstring: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, *mut isize) -> *const char>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_tolstring").unwrap()
    }
});

pub static lua_toboolean: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> bool>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_toboolean").unwrap()
    }
});

pub static lua_topointer: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> *const c_void>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_topointer").unwrap()
    }
});

pub static lua_type: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> isize>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_type").unwrap()
    }
});

pub static lua_typename: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> *const char>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_typename").unwrap()
    }
});

pub static lua_isstring: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> isize>> = Lazy::new(|| {
    unsafe {
        LUA_LIB.get(b"lua_isstring").unwrap()
    }
});

/// Load the provided buffer as a lua module with the specified name.
/// # Safety
/// Makes a lot of FFI calls, mutates internal C lua state.
pub unsafe fn load_module<F: Fn(*mut LuaState, *const u8, isize, *const u8) -> u32>(state: *mut LuaState, name: &str, buffer: &str, lual_loadbuffer: &F) {
    let buf_cstr = CString::new(buffer).unwrap();
    let buf_len = buf_cstr.as_bytes().len();

    let p_name = format!("@{name}");
    let p_name_cstr = CString::new(p_name).unwrap();

    // Push the global package.loaded table onto the top of the stack, saving its index.
    let stack_top = lua_gettop(state);
    lua_getfield(state, LUA_GLOBALSINDEX, b"package\0".as_ptr() as _);
    lua_getfield(state, -1, b"loaded\0".as_ptr() as _);

    // This is the index of the `package.loaded` table.
    let field_index = lua_gettop(state);

    // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
    lual_loadbuffer(state, buf_cstr.into_raw() as _, buf_len as _, p_name_cstr.into_raw() as _);

    lua_pcall(state, 0, -1, 0);

    // Insert pcall results onto the package.loaded global table.
    let module_cstr = CString::new(name).unwrap();

    lua_setfield(state, field_index, module_cstr.into_raw() as _);
    lua_settop(state, stack_top);
}
