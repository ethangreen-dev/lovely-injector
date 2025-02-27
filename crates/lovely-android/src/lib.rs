#[macro_use] extern crate log;
extern crate android_log;

use std::panic;

use lovely_core::{sys::{LuaState, LUA_LIB}, Lovely};
use once_cell::sync::{Lazy, OnceCell};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static RECALL: Lazy<
    unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32,
> = Lazy::new(|| unsafe { *LUA_LIB.get(b"luaL_loadbufferx").unwrap() });

#[no_mangle]
unsafe extern "C" fn init() {

    android_log::init("Lovely").unwrap();

    panic::set_hook(Box::new(|x| {
        error!("lovely-injector has crashed: \n{x}");
    }));

    RUNTIME
        .set(Lovely::init(&|a, b, c, d, e| RECALL(a, b, c, d, e), false))
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
