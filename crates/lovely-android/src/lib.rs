#[macro_use] extern crate log;
extern crate android_log;

use std::{ffi::{c_char, CStr}, panic, path::PathBuf};

use libloading::Library;
use lovely_core::{sys::{get_lua_lib, set_lua_lib, LuaState}, Lovely, LovelyConfig};
use once_cell::sync::{Lazy, OnceCell};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static RECALL: Lazy<
    unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32,
> = Lazy::new(|| unsafe { *get_lua_lib().get(b"luaL_loadbufferx").unwrap() });

#[no_mangle]
unsafe extern "C" fn init(path: *const c_char) {

    android_log::init("Lovely").unwrap();

    panic::set_hook(Box::new(|x| {
        error!("lovely-injector has crashed: \n{x}");
    }));

    set_lua_lib(Library::new("libluajit.so").unwrap());

    RUNTIME
        .set(Lovely::init(&|a, b, c, d, e| RECALL(a, b, c, d, e), LovelyConfig {
            mod_dir: Some(PathBuf::from(CStr::from_ptr(path).to_str().unwrap().to_owned())),
            log_dir: None,
            dump: None,
            is_vanilla: false
        }))
        .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));
}

#[no_mangle]
unsafe extern "C" fn hooked_loadbufferx(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: isize,
    name_ptr: *const u8,
    mode_ptr: *const u8,
) -> u32 {
    RUNTIME.get_unchecked().apply_buffer_patches(state, buf_ptr, size, name_ptr, mode_ptr)
}
