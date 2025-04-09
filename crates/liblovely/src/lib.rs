use lovely_core::sys::{LuaState, LUA_LIB};
use std::{env, ptr::null};
use std::panic;
use lovely_core::log::*;

use lovely_core::Lovely;
use once_cell::sync::{Lazy, OnceCell};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

type loadbufferx = unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32;

static RECALL: OnceCell<loadbufferx> = OnceCell::new();

#[no_mangle]
unsafe extern "C" fn lovely_init(loadbufferx: loadbufferx) {
    if RUNTIME.get().is_none() {
        panic::set_hook(Box::new(|x| {
            let message = format!("lovely-injector has crashed: \n{x}");
            error!("{message}");
        }));

        RECALL.set(loadbufferx).expect("Shit's erroring");

        let rt = Lovely::init(&|a, b, c, d, e| RECALL.get_unchecked()(a, b, c, d, e), false);
        RUNTIME.set(rt).unwrap_or_else(|_| panic!("Failed to instantiate runtime."));
    }
}

#[no_mangle]
unsafe extern "C" fn lovely_apply_patches(
        state: *mut LuaState,
        buf_ptr: *const u8,
        size: isize,
        name_ptr: *const u8,
        mode_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, mode_ptr)
}
