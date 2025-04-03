use lovely_core::log::*;
use lovely_core::sys::{LuaState, LUA_LIB};
use std::{env, ffi::c_void, mem, panic};

use dobby_rs::{hook, resolve_symbol};
use lovely_core::Lovely;
use once_cell::sync::{Lazy, OnceCell};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static RECALL: Lazy<
    unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32,
> = Lazy::new(|| unsafe {
    let lua_loadbufferx: unsafe extern "C" fn(
        *mut LuaState,
        *const u8,
        isize,
        *const u8,
        *const u8,
    ) -> u32 = *LUA_LIB.get(b"luaL_loadbufferx").unwrap();
    // let lua_loadbufferx_2: unsafe extern "C" fn(
    //     *mut LuaState,
    //     *const u8,
    //     isize,
    //     *const u8,
    //     *const u8,
    // ) -> u32 = mem::transmute(
    //     resolve_symbol(
    //         "/Users/english5040/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Frameworks/Lua.framework/Versions/A/Lua",
    //         "luaL_loadbufferx",
    //     )
    //     .unwrap(),
    // );
    // assert!(lua_loadbufferx == lua_loadbufferx_2);
    let orig = hook(
        lua_loadbufferx as *mut c_void,
        lua_loadbufferx_detour as *mut c_void,
    )
    .unwrap();
    mem::transmute(orig)
});

unsafe extern "C" fn lua_loadbufferx_detour(
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
    let args: Vec<_> = env::args().collect();
    let dump_all = args.contains(&"--dump-all".to_string());

    let recall = Lazy::force(&RECALL);

    let rt = Lovely::init(&|a, b, c, d, e| RECALL(a, b, c, d, e), dump_all);
    RUNTIME
        .set(rt)
        .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));

    info!("old func addr: {recall:p}");
}
