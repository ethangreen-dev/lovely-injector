use std::ptr;
use std::sync::OnceLock;
use std::slice;
use std::ffi::{c_char, c_int, c_void, CString};
use std::collections::VecDeque;

use itertools::Itertools;
use libloading::Library;

use log::info;

pub static LUA: OnceLock<LuaLib> = OnceLock::new();

pub type LuaState = c_void;

pub const LUA_GLOBALSINDEX: c_int = -10002;
pub const LUA_TNIL: c_int = 0;
pub const LUA_TBOOLEAN: c_int = 1;

macro_rules! generate {
    ($libname:ident {
        $(
            $vis:vis unsafe extern "C" fn $method:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;
        )*
    }) => {
        pub struct $libname {
            $(
                $vis $method: unsafe extern "C" fn($($arg: $ty),*) $(-> $ret)?,
            )*
        }

        $(
            /// # Safety
            $vis unsafe extern "C" fn $method($($arg: $ty),*) $(-> $ret)? {
                let lua = LUA.get().unwrap_or_else(|| panic!("Failed to access Lua lib defs"));
                (lua.$method)($($arg),*)
            }
        )*
    };
}

generate! (LuaLib {
    pub unsafe extern "C" fn lua_call(state: *mut LuaState, nargs: c_int, nresults: c_int);
    pub unsafe extern "C" fn lua_pcall(state: *mut LuaState, nargs: c_int, nresults: c_int, errfunc: c_int) -> c_int;
    pub unsafe extern "C" fn lua_getfield(state: *mut LuaState, index: c_int, k: *const c_char);
    pub unsafe extern "C" fn lua_setfield(state: *mut LuaState, index: c_int, k: *const c_char);
    pub unsafe extern "C" fn lua_gettop(state: *mut LuaState) -> c_int;
    pub unsafe extern "C" fn lua_settop(state: *mut LuaState, index: c_int);
    pub unsafe extern "C" fn lua_pushvalue(state: *mut LuaState, index: c_int);
    pub unsafe extern "C" fn lua_pushcclosure(state: *mut LuaState, f: unsafe extern "C" fn(*mut LuaState) -> c_int, n: c_int);
    pub unsafe extern "C" fn lua_tolstring(state: *mut LuaState, index: c_int, len: *mut usize) -> *const c_char;
    pub unsafe extern "C" fn lua_type(state: *mut LuaState, index: c_int) -> c_int;
    pub unsafe extern "C" fn lual_register(state: *mut LuaState, libname: *const char, l: *const c_void);
    pub unsafe extern "C" fn lua_pushstring(state: *mut LuaState, string: *const char);
    pub unsafe extern "C" fn lua_pushnumber(state: *mut LuaState, number: f64);
    pub unsafe extern "C" fn lua_settable(state: *mut LuaState, index: isize);
});

impl LuaLib {
    /// Construct a LuaLib from a loaded library.
    /// # Safety
    /// The library must define Lua symbols.
    pub unsafe fn from_library(library: &Library) -> Self {
        LuaLib {
            lua_call: *library.get(b"lua_call").unwrap(),
            lua_pcall: *library.get(b"lua_pcall").unwrap(),
            lua_getfield: *library.get(b"lua_getfield").unwrap(),
            lua_setfield: *library.get(b"lua_setfield").unwrap(),
            lua_gettop: *library.get(b"lua_gettop").unwrap(),
            lua_settop: *library.get(b"lua_settop").unwrap(),
            lua_pushvalue: *library.get(b"lua_pushvalue").unwrap(),
            lua_pushcclosure: *library.get(b"lua_pushcclosure").unwrap(),
            lua_tolstring: *library.get(b"lua_tolstring").unwrap(),
            lua_type: *library.get(b"lua_type").unwrap(),
            lual_register: *library.get(b"luaL_register").unwrap(),
            lua_pushstring: *library.get(b"lua_pushstring").unwrap(),
            lua_pushnumber: *library.get(b"lua_pushnumber").unwrap(),
            lua_settable: *library.get(b"lua_settable").unwrap(),
        }
    }
}

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
pub unsafe fn load_module<F: Fn(*mut LuaState, *const u8, usize, *const u8, *const u8) -> u32>(
    state: *mut LuaState,
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
pub unsafe fn is_module_preloaded(state: *mut LuaState, name: &str) -> bool {
    let name_cstr = CString::new(name).unwrap();
    let stack_top = lua_gettop(state);
    lua_getfield(state, LUA_GLOBALSINDEX, c"package".as_ptr());
    lua_getfield(state, -1, c"preload".as_ptr());
    lua_getfield(state, -1, name_cstr.as_ptr());

    let res = lua_type(state, -1) != LUA_TNIL;

    lua_settop(state, stack_top);
    return res;
}

/// An override print function, copied piecemeal from the Lua 5.1 source, but in Rust.
/// # Safety
/// Native lua API access. It's unsafe, it's unchecked, it will probably eat your firstborn.
pub unsafe extern "C" fn override_print(state: *mut LuaState) -> c_int {
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
pub unsafe extern "C" fn lua_identity_closure(state: *mut LuaState) -> c_int {
    // LUA_GLOBALSINDEX - 1 is where the first upvalue is located
    lua_pushvalue(state, LUA_GLOBALSINDEX - 1);
    // We just return that value
    1
}
