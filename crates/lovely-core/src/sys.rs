use std::collections::VecDeque;
use std::ffi::{c_int, CString};
use std::ptr;
use std::slice;
use mlua::Lua;

use itertools::Itertools;
pub use mlua_sys::*;
use log::info;

// TODO: Can we make this work with variable number of upvalues?
unsafe extern "C-unwind" fn lua_return_values(state: *mut lua_State) -> c_int {
    let index = lua_upvalueindex(1);
    lua_pushvalue(state, index);
    1
}

// TODO: implement all lua methods on this(?)
pub(crate) trait LuaStateTrait {
    unsafe fn push<P: Pushable>(self, obj: P);
    unsafe fn push_closure(self, func: lua_CFunction, vals: c_int);
    unsafe fn to_string(self, index: c_int) -> String;
}

impl LuaStateTrait for *mut lua_State {
    unsafe fn push<P: Pushable>(self, obj: P) {
        obj.push(self)
    }

    unsafe fn push_closure(self, func: lua_CFunction, vals: c_int) {
        lua_pushcclosure(self, func, vals);
    }

    unsafe fn to_string(self, index: c_int) -> String {
        let mut str_len = 0usize;
        let arg_str = lua_tolstring(self, index, &mut str_len);

        let str_buf = slice::from_raw_parts(arg_str as *const u8, str_len);
        String::from_utf8_lossy(str_buf).to_string()
    }
}
/// A trait which allows the implementing value to generically push its value onto the Lua stack.
pub trait Pushable {
    /// Push this value onto the Lua stack.
    ///
    /// # Safety
    /// Directly interacts with native Lua state.
    unsafe fn push(&self, state: *mut lua_State);
}

impl Pushable for String {
    unsafe fn push(&self, state: *mut lua_State) {
        lua_pushlstring(state, self.as_ptr() as _, self.len());
    }
}

impl Pushable for &String {
    unsafe fn push(&self, state: *mut lua_State) {
        lua_pushlstring(state, self.as_ptr() as _, self.len());
    }
}

impl Pushable for &str {
    unsafe fn push(&self, state: *mut lua_State) {
        lua_pushlstring(state, self.as_ptr() as _, self.len());
    }
}

impl Pushable for isize {
    unsafe fn push(&self, state: *mut lua_State) {
        lua_pushnumber(state, *self as _);
    }
}

impl Pushable for bool {
    unsafe fn push(&self, state: *mut lua_State) {
        lua_pushboolean(state, *self as _);
    }
}

impl Pushable for lua_CFunction {
    unsafe fn push(&self, state: *mut lua_State) {
        lua_pushcclosure(state, *self as _, 0);
    }
}

/// Commit this Lua module to native Lua state.
///
/// # Safety
/// Directly interacts and mutates native Lua state.
pub unsafe fn preload_module(state: *mut lua_State, name: &'static str, value: impl mlua::IntoLua) {
    let top = lua_gettop(state);

    // Get the package.preloads
    lua_getfield(state, LUA_GLOBALSINDEX, c"package".as_ptr());
    lua_getfield(state, -1, c"preload".as_ptr());
    let preload_index = lua_gettop(state);

    // Push the value to the stack
    // HACK: This is a messy solution cause we're half mlua
    {
        let lua = Lua::get_or_init_from_ptr(state);

        lua.exec_raw_lua(|rawlua| {
            value.push_into_stack(rawlua).unwrap();
        });
    }


    // Push our function to the stack
    let func: lua_CFunction = lua_return_values;

    state.push_closure(func, 1);

    let name = CString::new(name).unwrap();
    lua_setfield(state, preload_index, name.into_raw() as _);

    // Reset the stack.
    lua_settop(state, top);
}

/// Load the provided buffer as a lua module with the specified name.
/// # Safety
/// Makes a lot of FFI calls, mutates internal C lua state.
pub unsafe fn load_module<F: Fn(*mut lua_State, *const u8, usize, *const u8, *const u8) -> u32>(
    state: *mut lua_State,
    name: &str,
    buffer: &str,
    lual_loadbufferx: &F,
) {
    let p_name = format!("@{name}");
    let p_name_cstr = CString::new(p_name).unwrap();

    // Push the global package.preload table onto the top of the stack, saving its index.
    let stack_top = lua_gettop(state);
    lua_getfield(state, LUA_GLOBALSINDEX, c"package".as_ptr());
    lua_getfield(state, -1, c"preload".as_ptr());

    // This is the index of the `package.loaded` table.
    let field_index = lua_gettop(state);

    // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
    lual_loadbufferx(
        state,
        buffer.as_ptr(),
        buffer.len(),
        p_name_cstr.into_raw() as _,
        ptr::null(),
    );

    let lua_pcall_return = lua_pcall(state, 0, -1, 0);
    if lua_pcall_return == 0 {
        lua_pushcclosure(state, lua_identity_closure, 1);
        // Insert wrapped pcall results onto the package.preload global table.
        let module_cstr = CString::new(name).unwrap();

        lua_setfield(state, field_index, module_cstr.into_raw());
    }

    lua_settop(state, stack_top);
}

// Checks if a module is in the preload table. Used to check if lovely was already initalized
// # Safety
// Uses the native lua API. I'm also pretty sure I it bikes without a helmet.
pub(crate) unsafe fn is_module_preloaded(state: *mut lua_State, name: &str) -> bool {
    let name_cstr = CString::new(name).unwrap();
    let stack_top = lua_gettop(state);
    lua_getfield(state, LUA_GLOBALSINDEX, c"package".as_ptr());
    lua_getfield(state, -1, c"preload".as_ptr());
    lua_getfield(state, -1, name_cstr.as_ptr());

    let res = lua_type(state, -1) != LUA_TNIL;

    lua_settop(state, stack_top);
    res
}

/// An override print function, copied piecemeal from the Lua 5.1 source, but in Rust.
/// # Safety
/// Native lua API access. It's unsafe, it's unchecked, it will probably eat your firstborn.
pub unsafe extern "C-unwind" fn override_print(state: *mut lua_State) -> c_int {
    let argc = lua_gettop(state);
    let mut out = VecDeque::new();

    for _ in 0..argc {
        // We call Lua's builtin tostring function because we don't have access to the 5.3 luaL_tolstring
        // helper function. It's not pretty, but it works.
        lua_getfield(state, LUA_GLOBALSINDEX, c"tostring".as_ptr());
        lua_pushvalue(state, -2);
        lua_call(state, 1, 1);

        let mut str_len = 0usize;
        let arg_str = lua_tolstring(state, -1, &mut str_len);

        let str_buf = slice::from_raw_parts(arg_str as *const u8, str_len);
        let arg_str = String::from_utf8_lossy(str_buf).to_string();

        out.push_front(arg_str);
        lua_settop(state, -3);
    }

    let msg = out.into_iter().join("\t");

    info!("[G] {msg}");

    0
}

/// A function, which as a Lua closure, returns the first upvalue. This lets it
/// be used to wrap lua values into a closure which returns that value.
/// # Safety
/// Makes some FFI calls, mutates internal C lua state.
pub unsafe extern "C-unwind" fn lua_identity_closure(state: *mut lua_State) -> c_int {
    // LUA_GLOBALSINDEX - 1 is where the first upvalue is located
    lua_pushvalue(state, LUA_GLOBALSINDEX - 1);
    // We just return that value
    1
}

/// A function, which as a Lua closure, throws the first upvalue. This lets it
/// be used to wrap lua values into a closure which throws that value.
/// # Safety
/// Makes some FFI calls, mutates internal C lua state.
pub unsafe extern "C-unwind" fn lua_err_identity_closure(state: *mut lua_State) -> c_int {
    // LUA_GLOBALSINDEX - 1 is where the first upvalue is located
    lua_pushvalue(state, LUA_GLOBALSINDEX - 1);
    lua_error(state)
}

// Used to wrap the function passed to mlua's lua.create_function
// Removes the result (which mlua uses to cause an error) which allows
// a function to return a Result (which is translated to lua's 
// Ok(v) = v, Err(v) = false, v) allowing us to use the ? operator
pub fn no_err<F, A, R>(func: F) -> impl Fn(&Lua, A) -> mlua::Result<R> + mlua::MaybeSend + 'static
where
F: Fn(&Lua, A) -> R + mlua::MaybeSend + 'static,
    A: mlua::FromLuaMulti,
    R: mlua::IntoLuaMulti,
{
    move |lua, args| Ok(func(lua, args))
}
