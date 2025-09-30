mod lualib;

use lovely_core::log::*;
use lovely_core::sys::LuaState;
use lualib::LUA_LIBRARY;
use std::{
    env,
    ffi::c_void,
    mem, panic,
    sync::{LazyLock, OnceLock},
};

use lovely_core::Lovely;

static RUNTIME: OnceLock<Lovely> = OnceLock::new();

type LoadBuffer =
    unsafe extern "C" fn(*mut LuaState, *const u8, usize, *const u8, *const u8) -> u32;
static LUA_LOADBUFFERX: LazyLock<LoadBuffer> =
    LazyLock::new(|| unsafe { *LUA_LIBRARY.get(b"luaL_loadbufferx").unwrap() });

static RECALL: LazyLock<LoadBuffer> = LazyLock::new(|| unsafe {
    let orig = dobby_rs::hook(
        *LUA_LOADBUFFERX as *mut c_void,
        lua_loadbufferx_detour as *mut c_void,
    )
    .unwrap();
    mem::transmute(orig)
});

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "C" fn luaL_loadbuffer(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: usize,
    name_ptr: *const u8,
) -> u32 {
    RUNTIME.get().map_or_else(
        || (LUA_LOADBUFFERX)(state, buf_ptr, size, name_ptr, std::ptr::null()),
        |rt| rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, std::ptr::null()),
    )
}

unsafe extern "C" fn lua_loadbufferx_detour(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: usize,
    name_ptr: *const u8,
    mode_ptr: *const u8,
) -> u32 {
    RUNTIME.get().map_or_else(
        || (LUA_LOADBUFFERX)(state, buf_ptr, size, name_ptr, mode_ptr),
        |rt| rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, mode_ptr),
    )
}

#[ctor::ctor]
unsafe fn construct() {
    // panic::set_hook(Box::new(|x| {
    //     let message = format!("lovely-injector has crashed: \n{x}");
    //     error!("{message}");
    // }));

    let args: Vec<_> = env::args().collect();

    if args.contains(&"--vanilla".to_string()) || args.contains(&"-v".to_string()) {
        info!("running in vanilla mode");
        return;
    }

    let dump_all = args.contains(&"--dump-all".to_string());

    let rt = Lovely::init(
        &|a, b, c, d, e| RECALL(a, b, c, d, e),
        lualib::get_lualib(),
        dump_all,
    );
    RUNTIME
        .set(rt)
        .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));
}
