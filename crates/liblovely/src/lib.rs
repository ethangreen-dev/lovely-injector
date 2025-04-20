use lovely_core::log::*;
use lovely_core::sys::{LuaLib, LuaState};
use std::panic;

use lovely_core::Lovely;
use once_cell::sync::OnceCell;

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

type LoadBufferX =
    unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32;

static RECALL: OnceCell<LoadBufferX> = OnceCell::new();

#[no_mangle]
unsafe extern "C" fn lovely_init(
    loadbufferx: LoadBufferX,
    lua_call: *const std::ffi::c_void,
    lua_pcall: *const std::ffi::c_void,
    lua_getfield: *const std::ffi::c_void,
    lua_setfield: *const std::ffi::c_void,
    lua_gettop: *const std::ffi::c_void,
    lua_settop: *const std::ffi::c_void,
    lua_pushvalue: *const std::ffi::c_void,
    lua_pushcclosure: *const std::ffi::c_void,
    lua_tolstring: *const std::ffi::c_void,
    lua_toboolean: *const std::ffi::c_void,
    lua_topointer: *const std::ffi::c_void,
    lua_type: *const std::ffi::c_void,
    lua_typename: *const std::ffi::c_void,
    lua_isstring: *const std::ffi::c_void,
) {
    if RUNTIME.get().is_none() {
        panic::set_hook(Box::new(|x| {
            let message = format!("lovely-injector has crashed: \n{x}");
            error!("{message}");
        }));

        RECALL.set(loadbufferx).expect("Shit's erroring");


        let lua = LuaLib {
            lua_call: std::mem::transmute(lua_call),
            lua_pcall: std::mem::transmute(lua_pcall),
            lua_getfield: std::mem::transmute(lua_getfield),
            lua_setfield: std::mem::transmute(lua_setfield),
            lua_gettop: std::mem::transmute(lua_gettop),
            lua_settop: std::mem::transmute(lua_settop),
            lua_pushvalue: std::mem::transmute(lua_pushvalue),
            lua_pushcclosure: std::mem::transmute(lua_pushcclosure),
            lua_tolstring: std::mem::transmute(lua_tolstring),
            lua_toboolean: std::mem::transmute(lua_toboolean),
            lua_topointer: std::mem::transmute(lua_topointer),
            lua_type: std::mem::transmute(lua_type),
            lua_typename: std::mem::transmute(lua_typename),
            lua_isstring: std::mem::transmute(lua_isstring),
        };

        let rt = Lovely::init(
            &|a, b, c, d, e| RECALL.get_unchecked()(a, b, c, d, e),
            lua,
            false,
        );
        RUNTIME
            .set(rt)
            .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));
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
