use libloading::Library;
use lovely_core::sys::{get_lua_lib, set_lua_lib, LuaState};
use std::ptr::null;
use std::panic;
use lovely_core::{log::*, LovelyConfig};

use lovely_core::Lovely;
use once_cell::sync::{Lazy, OnceCell};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static RECALL: Lazy<
    unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32,
> = Lazy::new(|| unsafe { *get_lua_lib().get(b"luaL_loadbufferx").unwrap() });

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "C" fn luaL_loadbuffer(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: isize,
    name_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, null())
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "C" fn luaL_loadbufferx(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: isize,
    name_ptr: *const u8,
    mode_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, mode_ptr)
}

#[ctor::ctor]
unsafe fn construct() {
    panic::set_hook(Box::new(|x| {
        let message = format!("lovely-injector has crashed: \n{x}");
        error!("{message}");
    }));

    if cfg!(target_os = "linux") {
        set_lua_lib(Library::new("libluajit-5.1.so.2").unwrap());
    } else if cfg!(target_os = "macos") {
        set_lua_lib(Library::new("../Frameworks/Lua.framework/Versions/A/Lua").unwrap());
    }

    let rt = Lovely::init(&|a, b, c, d, e| RECALL(a, b, c, d, e), LovelyConfig::init_from_environment());
    RUNTIME
        .set(rt)
        .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));
}
