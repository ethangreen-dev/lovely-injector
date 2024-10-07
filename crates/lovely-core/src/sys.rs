use std::ptr;
use std::slice;
use std::collections::VecDeque;
use std::ffi::{c_void, CString};

use itertools::Itertools;
use libloading::{Library, Symbol};
use log::info;
use once_cell::sync::Lazy;

pub const LUA_GLOBALSINDEX: isize = -10002;

pub const LUA_TNIL: isize = 0;
pub const LUA_TBOOLEAN: isize = 1;

pub type LuaState = c_void;

#[cfg(target_os = "windows")]
pub static LUA_LIB: Lazy<Library> = Lazy::new(|| unsafe { Library::new("lua51.dll").unwrap() });

#[cfg(target_os = "macos")]
pub static LUA_LIB: Lazy<Library> =
    Lazy::new(|| unsafe { Library::new("../Frameworks/Lua.framework/Versions/A/Lua").unwrap() });

pub static lua_call: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, isize)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_call").unwrap() });

pub static lua_pcall: Lazy<
    Symbol<unsafe extern "C" fn(*mut LuaState, isize, isize, isize) -> isize>,
> = Lazy::new(|| unsafe { LUA_LIB.get(b"lua_pcall").unwrap() });

pub static lua_getfield: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, *const char)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_getfield").unwrap() });

pub static lua_setfield: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize, *const char)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_setfield").unwrap() });

pub static lua_gettop: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState) -> isize>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_gettop").unwrap() });

pub static lua_settop: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> isize>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_settop").unwrap() });

pub static lua_pushvalue: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_pushvalue").unwrap() });

pub static lua_pushcclosure: Lazy<
    Symbol<unsafe extern "C" fn(*mut LuaState, *const c_void, isize)>,
> = Lazy::new(|| unsafe { LUA_LIB.get(b"lua_pushcclosure").unwrap() });

pub static lua_tolstring: Lazy<
    Symbol<unsafe extern "C" fn(*mut LuaState, isize, *mut isize) -> *const char>,
> = Lazy::new(|| unsafe { LUA_LIB.get(b"lua_tolstring").unwrap() });

pub static lua_toboolean: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> bool>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_toboolean").unwrap() });

pub static lua_topointer: Lazy<
    Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> *const c_void>,
> = Lazy::new(|| unsafe { LUA_LIB.get(b"lua_topointer").unwrap() });

pub static lua_type: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> isize>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_type").unwrap() });

pub static lua_typename: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> *const char>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_typename").unwrap() });

pub static lua_isstring: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize) -> isize>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_isstring").unwrap() });

pub static lual_register: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, *const char, *const c_void)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"luaL_register").unwrap() });

pub static lua_pushstring: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, *const char)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_pushstring").unwrap() });

pub static lua_pushnumber: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, f64)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_pushnumber").unwrap() });

pub static lua_settable: Lazy<Symbol<unsafe extern "C" fn(*mut LuaState, isize)>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"lua_settable").unwrap() });

/// A trait which allows the implementing value to generically push its value onto the Lua stack.
pub trait Pushable {
    /// Push this value onto the Lua stack.
    /// 
    /// # Safety
    /// Directly interacts with native Lua state.
    unsafe fn push(&self, state: *mut LuaState);
}

impl Pushable for String {
    unsafe fn push(&self, state: *mut LuaState) {
        let value = format!("{self}\0");
        lua_pushstring(state, value.as_ptr() as _);
    }
}

impl Pushable for &str {
    unsafe fn push(&self, state: *mut LuaState) {
        let value = format!("{self}\0");
        lua_pushstring(state, value.as_ptr() as _);
    }
}

impl Pushable for isize {
    unsafe fn push(&self, state: *mut LuaState) {
        lua_pushnumber(state, *self as _);
    }
}

/// A lua FFI entry. Used specifically by lual_register to register
/// a module and its associated functions into the lua runtime.
pub struct LuaReg {
    name: String,
    func: unsafe extern "C" fn(*mut LuaState) -> isize,
}

pub struct LuaVar<P: Pushable> {
    name: String,
    val: P,
}

pub struct LuaModule<P: Pushable> {
    reg: Vec<LuaReg>,
    var: Vec<LuaVar<P>>,
}

impl<P: Pushable> LuaModule<P> {
    pub fn new() -> Self {
        LuaModule {
            reg: vec![],
            var: vec![],
        }
    }

    /// Register a native C FFI function to this Lua module.
    pub fn add_reg(self, name: &'static str, func: unsafe extern "C" fn(*mut LuaState) -> isize) -> Self {
        let name = format!("{name}\0");
        let mut reg = self.reg;
        reg.push(LuaReg {
            name,
            func,
        });

        Self {
            reg,
            var: self.var,
        }
    }

    /// Add a variable to this Lua module.
    pub fn add_var(self, name: &'static str, val: P) -> Self {
        let name = format!("{name}\0");
        let mut var = self.var;
        var.push(LuaVar {
            name,
            val,
        });

        LuaModule {
            reg: self.reg,
            var,
        }
    }

    /// Commit this Lua module to native Lua state.
    /// 
    /// # Safety
    /// Directly interacts and mutates native Lua state.
    pub unsafe fn commit(self, state: *mut LuaState) {
        // Convert self.reg into a raw array of name:func pairs, represented as native c_void pointers.
        let native_reg: Vec<*const c_void> = self
            .reg
            .iter()
            .map(|reg| {
                let name = &reg.name;
                let func = reg.func;

                vec![name.as_ptr() as *const c_void, func as *const c_void]
            })
            .flatten()
            .chain(vec![0 as _, 0 as _])
            .collect();

        // Register the name:func table within the Lua runtime.
        let reg_ptr = native_reg.as_ptr() as *const c_void;
        lual_register(state, b"lovely\0".as_ptr() as _, reg_ptr);

        // Now we register variables onto the lovely global table.
        let top = lua_gettop(state);
        lua_getfield(state, LUA_GLOBALSINDEX, b"lovely\0".as_ptr() as _);

        for lua_var in self.var.iter() {
            // Push the var name and value onto the stack.
            lua_pushstring(state, lua_var.name.as_ptr() as _);
            lua_var.val.push(state);

            // Set the table key:val from what we previously pushed onto the stack.
            lua_settable(state, -3);
        }

        // Reset the stack.
        lua_settop(state, top);
    }
}


/// Load the provided buffer as a lua module with the specified name.
/// # Safety
/// Makes a lot of FFI calls, mutates internal C lua state.
pub unsafe fn load_module<F: Fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32>(
    state: *mut LuaState,
    name: &str,
    buffer: &str,
    lual_loadbufferx: &F,
) {
    let buf_cstr = CString::new(buffer).unwrap();
    let buf_len = buf_cstr.as_bytes().len();

    let p_name = format!("@{name}");
    let p_name_cstr = CString::new(p_name).unwrap();

    // Push the global package.preload table onto the top of the stack, saving its index.
    let stack_top = lua_gettop(state);
    lua_getfield(state, LUA_GLOBALSINDEX, b"package\0".as_ptr() as _);
    lua_getfield(state, -1, b"preload\0".as_ptr() as _);

    // This is the index of the `package.loaded` table.
    let field_index = lua_gettop(state);

    // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
    lual_loadbufferx(
        state,
        buf_cstr.into_raw() as _,
        buf_len as _,
        p_name_cstr.into_raw() as _,
        ptr::null(),
    );

    let lua_pcall_return = lua_pcall(state, 0, -1, 0);
    if lua_pcall_return == 0 {
        lua_pushcclosure(state, lua_identity_closure as *const c_void, 1);
        // Insert wrapped pcall results onto the package.preload global table.
        let module_cstr = CString::new(name).unwrap();

        lua_setfield(state, field_index, module_cstr.into_raw() as _);
    }

    lua_settop(state, stack_top);
}

/// An override print function, copied piecemeal from the Lua 5.1 source, but in Rust.
/// # Safety
/// Native lua API access. It's unsafe, it's unchecked, it will probably eat your firstborn.
pub unsafe extern "C" fn override_print(state: *mut LuaState) -> isize {
    let argc = lua_gettop(state);
    let mut out = VecDeque::new();

    for _ in 0..argc {
        let mut str_len = 0_isize;
        let arg_str = lua_tolstring(state, -1, &mut str_len);

        let arg_str = match arg_str.is_null() {
            true => String::from("nil"),
            false => {
                let str_buf = slice::from_raw_parts(arg_str as *const u8, str_len as _);
                String::from_utf8_lossy(str_buf).to_string()
            }
        };

        out.push_front(arg_str);
        lua_settop(state, -(1) - 1);
    }

    let msg = out
        .into_iter()
        .join("\t");

    info!("[G] {msg}");

    0
}

/// A function, which as a Lua closure, returns the first upvalue. This lets it
/// be used to wrap lua values into a closure which returns that value.
/// # Safety
/// Makes some FFI calls, mutates internal C lua state.
pub unsafe extern "C" fn lua_identity_closure(state: *mut LuaState) -> isize {
    // LUA_GLOBALSINDEX - 1 is where the first upvalue is located
    lua_pushvalue(state, LUA_GLOBALSINDEX - 1);
    // We just return that value
    return 1;
}
