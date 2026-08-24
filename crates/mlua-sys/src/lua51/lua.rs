//! Contains definitions from `lua.h`.

use std::ffi::CStr;
use std::marker::{PhantomData, PhantomPinned};
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::ptr;
use std::sync::OnceLock;
use libloading::Library;
use crate::luaL_Buffer;
use crate::luaL_Reg;
use core::ffi::VaList;

// Mark for precompiled code (`<esc>Lua`)
#[cfg(not(feature = "luajit"))]
pub const LUA_SIGNATURE: &[u8] = b"\x1bLua";
#[cfg(feature = "luajit")]
pub const LUA_SIGNATURE: &[u8] = b"\x1bLJ";

// Option for multiple returns in 'lua_pcall' and 'lua_call'
pub const LUA_MULTRET: c_int = -1;

//
// Pseudo-indices
//
pub const LUA_REGISTRYINDEX: c_int = -10000;
pub const LUA_ENVIRONINDEX: c_int = -10001;
pub const LUA_GLOBALSINDEX: c_int = -10002;

pub const fn lua_upvalueindex(i: c_int) -> c_int {
    LUA_GLOBALSINDEX - i
}

//
// Thread status
//
pub const LUA_OK: c_int = 0;
pub const LUA_YIELD: c_int = 1;
pub const LUA_ERRRUN: c_int = 2;
pub const LUA_ERRSYNTAX: c_int = 3;
pub const LUA_ERRMEM: c_int = 4;
pub const LUA_ERRERR: c_int = 5;

/// A raw Lua state associated with a thread.
#[repr(C)]
pub struct lua_State {
    _data: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

//
// Basic types
//
pub const LUA_TNONE: c_int = -1;

pub const LUA_TNIL: c_int = 0;
pub const LUA_TBOOLEAN: c_int = 1;
pub const LUA_TLIGHTUSERDATA: c_int = 2;
pub const LUA_TNUMBER: c_int = 3;
pub const LUA_TSTRING: c_int = 4;
pub const LUA_TTABLE: c_int = 5;
pub const LUA_TFUNCTION: c_int = 6;
pub const LUA_TUSERDATA: c_int = 7;
pub const LUA_TTHREAD: c_int = 8;

/// Type produced by LuaJIT FFI module
#[cfg(feature = "luajit")]
pub const LUA_TCDATA: c_int = 10;

/// Minimum Lua stack available to a C function
pub const LUA_MINSTACK: c_int = 20;

/// A Lua number, usually equivalent to `f64`
pub type lua_Number = c_double;

/// A Lua integer, usually equivalent to `i64`
#[cfg(target_pointer_width = "32")]
pub type lua_Integer = i32;
#[cfg(target_pointer_width = "64")]
pub type lua_Integer = i64;

/// Type for native C functions that can be passed to Lua.
pub type lua_CFunction = unsafe extern "C-unwind" fn(L: *mut lua_State) -> c_int;

// Type for functions that read/write blocks when loading/dumping Lua chunks
#[rustfmt::skip]
pub type lua_Reader =
    unsafe extern "C-unwind" fn(L: *mut lua_State, ud: *mut c_void, sz: *mut usize) -> *const c_char;
#[rustfmt::skip]
pub type lua_Writer =
    unsafe extern "C-unwind" fn(L: *mut lua_State, p: *const c_void, sz: usize, ud: *mut c_void) -> c_int;

/// Type for memory-allocation functions (no unwinding)
#[rustfmt::skip]
pub type lua_Alloc =
    unsafe extern "C" fn(ud: *mut c_void, ptr: *mut c_void, osize: usize, nsize: usize) -> *mut c_void;


pub static LUA: OnceLock<LuaLib> = OnceLock::new();
macro_rules! generate {
    ($libname:ident {
        $(
            $vis:vis unsafe extern "C-unwind" fn $method:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;
        )*
    } $(raw {
        $(
            $man_vis:vis $man_field:ident: $man_ty:ty,
        )*
    })?) => {
        #[repr(C)]
        pub struct $libname {
            $(
                $vis $method: unsafe extern "C-unwind" fn($($arg: $ty),*) $(-> $ret)?,
            )*
            $(
                $(
                    $man_vis $man_field: $man_ty,
                )*
            )?
        }

        $(
            /// # Safety
            $vis unsafe extern "C-unwind" fn $method($($arg: $ty),*) $(-> $ret)? {
                let lua = LUA.get().unwrap_or_else(|| panic!("Failed to access Lua lib defs"));
                (lua.$method)($($arg),*)
            }
        )*
    };
}   

generate! (LuaLib {
    //
    // State manipulation
    //
    pub unsafe extern "C-unwind" fn lua_newstate(f: lua_Alloc, ud: *mut c_void) -> *mut lua_State;
    pub unsafe extern "C-unwind" fn lua_close(L: *mut lua_State);
    pub unsafe extern "C-unwind" fn lua_newthread(L: *mut lua_State) -> *mut lua_State;
    
    pub unsafe extern "C-unwind" fn lua_atpanic(L: *mut lua_State, panicf: lua_CFunction) -> lua_CFunction;
    
    //
    // Basic stack manipulation
    //
    pub unsafe extern "C-unwind" fn lua_gettop(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn lua_settop(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_pushvalue(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_remove(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_insert(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_replace(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_checkstack(L: *mut lua_State, sz: c_int) -> c_int;
    
    pub unsafe extern "C-unwind" fn lua_xmove(from: *mut lua_State, to: *mut lua_State, n: c_int);
    
    //
    // Access functions (stack -> C)
    //
    pub unsafe extern "C-unwind" fn lua_isnumber(L: *mut lua_State, idx: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_isstring(L: *mut lua_State, idx: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_iscfunction(L: *mut lua_State, idx: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_isuserdata(L: *mut lua_State, idx: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_type(L: *mut lua_State, idx: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_typename(L: *mut lua_State, tp: c_int) -> *const c_char;
    
    pub unsafe extern "C-unwind" fn lua_equal(L: *mut lua_State, idx1: c_int, idx2: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_rawequal(L: *mut lua_State, idx1: c_int, idx2: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_lessthan(L: *mut lua_State, idx1: c_int, idx2: c_int) -> c_int;
    
    pub unsafe extern "C-unwind" fn lua_tonumber(L: *mut lua_State, idx: c_int) -> lua_Number;
    pub unsafe extern "C-unwind" fn lua_tointeger_(L: *mut lua_State, idx: c_int) -> lua_Integer;
    pub unsafe extern "C-unwind" fn lua_toboolean(L: *mut lua_State, idx: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_tolstring(L: *mut lua_State, idx: c_int, len: *mut usize) -> *const c_char;
    pub unsafe extern "C-unwind" fn lua_objlen(L: *mut lua_State, idx: c_int) -> usize;
    pub unsafe extern "C-unwind" fn lua_tocfunction(L: *mut lua_State, idx: c_int) -> Option<lua_CFunction>;
    pub unsafe extern "C-unwind" fn lua_touserdata(L: *mut lua_State, idx: c_int) -> *mut c_void;
    pub unsafe extern "C-unwind" fn lua_tothread(L: *mut lua_State, idx: c_int) -> *mut lua_State;
    pub unsafe extern "C-unwind" fn lua_topointer(L: *mut lua_State, idx: c_int) -> *const c_void;
    
    //
    // Push functions (C -> stack)
    //
    pub unsafe extern "C-unwind" fn lua_pushnil(L: *mut lua_State);
    pub unsafe extern "C-unwind" fn lua_pushnumber(L: *mut lua_State, n: lua_Number);
    pub unsafe extern "C-unwind" fn lua_pushinteger(L: *mut lua_State, n: lua_Integer);
    pub unsafe extern "C-unwind" fn lua_pushlstring_(L: *mut lua_State, s: *const c_char, l: usize);
    pub unsafe extern "C-unwind" fn lua_pushstring_(L: *mut lua_State, s: *const c_char);
    // lua_pushvfstring
    pub unsafe extern "C-unwind" fn lua_pushcclosure(L: *mut lua_State, f: lua_CFunction, n: c_int);
    pub unsafe extern "C-unwind" fn lua_pushboolean(L: *mut lua_State, b: c_int);
    pub unsafe extern "C-unwind" fn lua_pushlightuserdata(L: *mut lua_State, p: *mut c_void);
    pub unsafe extern "C-unwind" fn lua_pushthread(L: *mut lua_State) -> c_int;
    
    //
    // Get functions (Lua -> stack)
    //
    pub unsafe extern "C-unwind" fn lua_gettable_(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_getfield_(L: *mut lua_State, idx: c_int, k: *const c_char);
    pub unsafe extern "C-unwind" fn lua_rawget_(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_rawgeti_(L: *mut lua_State, idx: c_int, n: c_int);
    pub unsafe extern "C-unwind" fn lua_createtable(L: *mut lua_State, narr: c_int, nrec: c_int);
    pub unsafe extern "C-unwind" fn lua_newuserdata(L: *mut lua_State, sz: usize) -> *mut c_void;
    pub unsafe extern "C-unwind" fn lua_getmetatable(L: *mut lua_State, objindex: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_getfenv(L: *mut lua_State, idx: c_int);
    
    //
    // Set functions (stack -> Lua)
    //
    pub unsafe extern "C-unwind" fn lua_settable(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_setfield(L: *mut lua_State, idx: c_int, k: *const c_char);
    pub unsafe extern "C-unwind" fn lua_rawset(L: *mut lua_State, idx: c_int);
    pub unsafe extern "C-unwind" fn lua_rawseti_(L: *mut lua_State, idx: c_int, n: c_int);
    pub unsafe extern "C-unwind" fn lua_setmetatable(L: *mut lua_State, objindex: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_setfenv(L: *mut lua_State, idx: c_int) -> c_int;
    
    //
    // 'load' and 'call' functions (load and run Lua code)
    //
    pub unsafe extern "C-unwind" fn lua_call(L: *mut lua_State, nargs: c_int, nresults: c_int);
    pub unsafe extern "C-unwind" fn lua_pcall(L: *mut lua_State, nargs: c_int, nresults: c_int, errfunc: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_cpcall(L: *mut lua_State, f: lua_CFunction, ud: *mut c_void) -> c_int;
    pub unsafe extern "C-unwind" fn lua_load(L: *mut lua_State, reader: lua_Reader, data: *mut c_void, chunkname: *const c_char) -> c_int;
    
    pub unsafe extern "C-unwind" fn lua_dump_(L: *mut lua_State, writer: lua_Writer, data: *mut c_void) -> c_int;
    
    //
    // Coroutine functions
    //
    pub unsafe extern "C-unwind" fn lua_yield(L: *mut lua_State, nresults: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_resume_(L: *mut lua_State, narg: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_status(L: *mut lua_State) -> c_int;

    //
    // Debug API
    //

    pub unsafe extern "C-unwind" fn lua_getstack(L: *mut lua_State, level: c_int, ar: *mut lua_Debug) -> c_int;
    pub unsafe extern "C-unwind" fn lua_getinfo(L: *mut lua_State, what: *const c_char, ar: *mut lua_Debug) -> c_int;
    pub unsafe extern "C-unwind" fn lua_getlocal(L: *mut lua_State, ar: *const lua_Debug, n: c_int) -> *const c_char;
    pub unsafe extern "C-unwind" fn lua_setlocal(L: *mut lua_State, ar: *const lua_Debug, n: c_int) -> *const c_char;
    pub unsafe extern "C-unwind" fn lua_getupvalue(L: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;
    pub unsafe extern "C-unwind" fn lua_setupvalue(L: *mut lua_State, funcindex: c_int, n: c_int) -> *const c_char;

    pub unsafe extern "C-unwind" fn lua_sethook(L: *mut lua_State, func: Option<lua_Hook>, mask: c_int, count: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_gethook(L: *mut lua_State) -> Option<lua_Hook>;
    pub unsafe extern "C-unwind" fn lua_gethookmask(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn lua_gethookcount(L: *mut lua_State) -> c_int;

    //
    // Garbage-collection function and options
    //
    pub unsafe extern "C-unwind" fn lua_gc(L: *mut lua_State, what: c_int, data: c_int) -> c_int;

    //
    // Miscellaneous functions
    //
    unsafe extern "C-unwind" fn lua_error_(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn lua_next(L: *mut lua_State, idx: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn lua_concat(L: *mut lua_State, n: c_int);
    pub unsafe extern "C-unwind" fn lua_getallocf(L: *mut lua_State, ud: *mut *mut c_void) -> lua_Alloc;
    pub unsafe extern "C-unwind" fn lua_setallocf(L: *mut lua_State, f: lua_Alloc, ud: *mut c_void);


    //
    // Lua Aux Lib
    // 
    pub unsafe extern "C-unwind" fn luaL_register(L: *mut lua_State, libname: *const c_char, l: *const luaL_Reg);
    pub unsafe extern "C-unwind" fn luaL_getmetafield_(L: *mut lua_State, obj: c_int, e: *const c_char) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_callmeta(L: *mut lua_State, obj: c_int, e: *const c_char) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_typerror(L: *mut lua_State, narg: c_int, tname: *const c_char) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_argerror(L: *mut lua_State, narg: c_int, extramsg: *const c_char) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_checklstring(L: *mut lua_State, narg: c_int, l: *mut usize) -> *const c_char;
    pub unsafe extern "C-unwind" fn luaL_optlstring( L: *mut lua_State, narg: c_int, def: *const c_char, l: *mut usize) -> *const c_char;
    pub unsafe extern "C-unwind" fn luaL_checknumber(L: *mut lua_State, narg: c_int) -> lua_Number;
    pub unsafe extern "C-unwind" fn luaL_optnumber(L: *mut lua_State, narg: c_int, def: lua_Number) -> lua_Number;
    pub unsafe extern "C-unwind" fn luaL_checkinteger(L: *mut lua_State, narg: c_int) -> lua_Integer;
    pub unsafe extern "C-unwind" fn luaL_optinteger(L: *mut lua_State, narg: c_int, def: lua_Integer) -> lua_Integer;
    pub unsafe extern "C-unwind" fn luaL_checkstack_(L: *mut lua_State, sz: c_int, msg: *const c_char);
    pub unsafe extern "C-unwind" fn luaL_checktype(L: *mut lua_State, narg: c_int, t: c_int);
    pub unsafe extern "C-unwind" fn luaL_checkany(L: *mut lua_State, narg: c_int);

    pub unsafe extern "C-unwind" fn luaL_newmetatable_(L: *mut lua_State, tname: *const c_char) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_checkudata(L: *mut lua_State, ud: c_int, tname: *const c_char) -> *mut c_void;

    pub unsafe extern "C-unwind" fn luaL_where(L: *mut lua_State, lvl: c_int);

    pub unsafe extern "C-unwind" fn luaL_checkoption( L: *mut lua_State, narg: c_int, def: *const c_char, lst: *const *const c_char) -> c_int;

    pub unsafe extern "C-unwind" fn luaL_ref(L: *mut lua_State, t: c_int) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_unref(L: *mut lua_State, t: c_int, r#ref: c_int);

    pub unsafe extern "C-unwind" fn luaL_loadfile(L: *mut lua_State, filename: *const c_char) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_loadbuffer(L: *mut lua_State, buff: *const c_char, sz: usize, name: *const c_char) -> c_int;
    pub unsafe extern "C-unwind" fn luaL_loadstring(L: *mut lua_State, s: *const c_char) -> c_int;

    pub unsafe extern "C-unwind" fn luaL_newstate() -> *mut lua_State;

    pub unsafe extern "C-unwind" fn luaL_gsub( L: *mut lua_State, s: *const c_char, p: *const c_char, r: *const c_char) -> *const c_char;

    pub unsafe extern "C-unwind" fn luaL_findtable( L: *mut lua_State, idx: c_int, fname: *const c_char, szhint: c_int) -> *const c_char;
    pub unsafe extern "C-unwind" fn luaL_buffinit(L: *mut lua_State, B: *mut luaL_Buffer);
    pub unsafe extern "C-unwind" fn luaL_prepbuffer(B: *mut luaL_Buffer) -> *mut c_char;
    pub unsafe extern "C-unwind" fn luaL_addlstring(B: *mut luaL_Buffer, s: *const c_char, l: usize);
    pub unsafe extern "C-unwind" fn luaL_addstring(B: *mut luaL_Buffer, s: *const c_char);
    pub unsafe extern "C-unwind" fn luaL_addvalue(B: *mut luaL_Buffer);
    pub unsafe extern "C-unwind" fn luaL_pushresult(B: *mut luaL_Buffer);

    //
    // Lua lib
    //
    pub unsafe extern "C-unwind" fn luaopen_base(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn luaopen_table(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn luaopen_io(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn luaopen_os(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn luaopen_string(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn luaopen_math(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn luaopen_debug(L: *mut lua_State) -> c_int;
    pub unsafe extern "C-unwind" fn luaopen_package(L: *mut lua_State) -> c_int;

    // HACK: These don't work in our macro, we only support luajit for now so /shrug
    // #[cfg(feature = "luajit")]
    pub unsafe extern "C-unwind" fn luaopen_bit(L: *mut lua_State) -> c_int;
    // #[cfg(feature = "luajit")]
    pub unsafe extern "C-unwind" fn luaopen_jit(L: *mut lua_State) -> c_int;
    // #[cfg(feature = "luajit")]
    pub unsafe extern "C-unwind" fn luaopen_ffi(L: *mut lua_State) -> c_int;

    // open all builtin libraries
    pub unsafe extern "C-unwind" fn luaL_openlibs(L: *mut lua_State);

} raw {
    pub lua_pushvfstring: unsafe extern "C-unwind" fn(L: *mut lua_State, fmt: *const c_char, args: VaList) -> *const c_char,
});
pub unsafe extern "C-unwind" fn lua_pushfstring(L: *mut lua_State, fmt: *const c_char, mut args: ...) -> *const c_char {
    let lua = LUA.get().unwrap_or_else(|| panic!("Failed to access Lua lib defs"));
    (lua.lua_pushvfstring)(L, fmt, args.as_va_list())
}
// Reimplementation cause rust is stupid with variable args
pub unsafe extern "C-unwind" fn luaL_error(L: *mut lua_State, fmt: *const c_char, mut args: ...) -> c_int {
    let lua = LUA.get().unwrap_or_else(|| panic!("Failed to access Lua lib defs"));
    luaL_where(L, 1);
    (lua.lua_pushvfstring)(L, fmt, args.as_va_list());
    lua_concat(L, 2);
    lua_error(L);
}

impl LuaLib {
    /// Construct a LuaLib from a loaded library.
    /// # Safety
    /// The library must define Lua symbols.
    pub unsafe fn from_library(library: &Library) -> Self {
        LuaLib {
            lua_newstate: *library.get(b"lua_newstate").unwrap(),
            lua_close: *library.get(b"lua_close").unwrap(),
            lua_newthread: *library.get(b"lua_newthread").unwrap(),
            lua_atpanic: *library.get(b"lua_atpanic").unwrap(),
            lua_gettop: *library.get(b"lua_gettop").unwrap(),
            lua_settop: *library.get(b"lua_settop").unwrap(),
            lua_pushvalue: *library.get(b"lua_pushvalue").unwrap(),
            lua_remove: *library.get(b"lua_remove").unwrap(),
            lua_insert: *library.get(b"lua_insert").unwrap(),
            lua_replace: *library.get(b"lua_replace").unwrap(),
            lua_checkstack: *library.get(b"lua_checkstack").unwrap(),
            lua_xmove: *library.get(b"lua_xmove").unwrap(),
            lua_isnumber: *library.get(b"lua_isnumber").unwrap(),
            lua_isstring: *library.get(b"lua_isstring").unwrap(),
            lua_iscfunction: *library.get(b"lua_iscfunction").unwrap(),
            lua_isuserdata: *library.get(b"lua_isuserdata").unwrap(),
            lua_type: *library.get(b"lua_type").unwrap(),
            lua_typename: *library.get(b"lua_typename").unwrap(),
            lua_equal: *library.get(b"lua_equal").unwrap(),
            lua_rawequal: *library.get(b"lua_rawequal").unwrap(),
            lua_lessthan: *library.get(b"lua_lessthan").unwrap(),
            lua_tonumber: *library.get(b"lua_tonumber").unwrap(),
            lua_tointeger_: *library.get(b"lua_tointeger").unwrap(),
            lua_toboolean: *library.get(b"lua_toboolean").unwrap(),
            lua_tolstring: *library.get(b"lua_tolstring").unwrap(),
            lua_objlen: *library.get(b"lua_objlen").unwrap(),
            lua_tocfunction: *library.get(b"lua_tocfunction").unwrap(),
            lua_touserdata: *library.get(b"lua_touserdata").unwrap(),
            lua_tothread: *library.get(b"lua_tothread").unwrap(),
            lua_topointer: *library.get(b"lua_topointer").unwrap(),
            lua_pushnil: *library.get(b"lua_pushnil").unwrap(),
            lua_pushnumber: *library.get(b"lua_pushnumber").unwrap(),
            lua_pushinteger: *library.get(b"lua_pushinteger").unwrap(),
            lua_pushlstring_: *library.get(b"lua_pushlstring").unwrap(),
            lua_pushstring_: *library.get(b"lua_pushstring").unwrap(),
            lua_pushvfstring: *library.get(b"lua_pushvfstring").unwrap(),
            lua_pushcclosure: *library.get(b"lua_pushcclosure").unwrap(),
            lua_pushboolean: *library.get(b"lua_pushboolean").unwrap(),
            lua_pushlightuserdata: *library.get(b"lua_pushlightuserdata").unwrap(),
            lua_pushthread: *library.get(b"lua_pushthread").unwrap(),
            lua_gettable_: *library.get(b"lua_gettable").unwrap(),
            lua_getfield_: *library.get(b"lua_getfield").unwrap(),
            lua_rawget_: *library.get(b"lua_rawget").unwrap(),
            lua_rawgeti_: *library.get(b"lua_rawgeti").unwrap(),
            lua_createtable: *library.get(b"lua_createtable").unwrap(),
            lua_newuserdata: *library.get(b"lua_newuserdata").unwrap(),
            lua_getmetatable: *library.get(b"lua_getmetatable").unwrap(),
            lua_getfenv: *library.get(b"lua_getfenv").unwrap(),
            lua_settable: *library.get(b"lua_settable").unwrap(),
            lua_setfield: *library.get(b"lua_setfield").unwrap(),
            lua_rawset: *library.get(b"lua_rawset").unwrap(),
            lua_rawseti_: *library.get(b"lua_rawseti").unwrap(),
            lua_setmetatable: *library.get(b"lua_setmetatable").unwrap(),
            lua_setfenv: *library.get(b"lua_setfenv").unwrap(),
            lua_call: *library.get(b"lua_call").unwrap(),
            lua_pcall: *library.get(b"lua_pcall").unwrap(),
            lua_cpcall: *library.get(b"lua_cpcall").unwrap(),
            lua_load: *library.get(b"lua_load").unwrap(),
            lua_dump_: *library.get(b"lua_dump").unwrap(),
            lua_yield: *library.get(b"lua_yield").unwrap(),
            lua_resume_: *library.get(b"lua_resume").unwrap(),
            lua_status: *library.get(b"lua_status").unwrap(),
            lua_getstack: *library.get(b"lua_getstack").unwrap(),
            lua_getinfo: *library.get(b"lua_getinfo").unwrap(),
            lua_getlocal: *library.get(b"lua_getlocal").unwrap(),
            lua_setlocal: *library.get(b"lua_setlocal").unwrap(),
            lua_getupvalue: *library.get(b"lua_getupvalue").unwrap(),
            lua_setupvalue: *library.get(b"lua_setupvalue").unwrap(),
            lua_sethook: *library.get(b"lua_sethook").unwrap(),
            lua_gethook: *library.get(b"lua_gethook").unwrap(),
            lua_gethookmask: *library.get(b"lua_gethookmask").unwrap(),
            lua_gethookcount: *library.get(b"lua_gethookcount").unwrap(),
            lua_gc: *library.get(b"lua_gc").unwrap(),
            lua_error_: *library.get(b"lua_error").unwrap(),
            lua_next: *library.get(b"lua_next").unwrap(),
            lua_concat: *library.get(b"lua_concat").unwrap(),
            lua_getallocf: *library.get(b"lua_getallocf").unwrap(),
            lua_setallocf: *library.get(b"lua_setallocf").unwrap(),
            luaL_register: *library.get(b"luaL_register").unwrap(),
            luaL_getmetafield_: *library.get(b"luaL_getmetafield").unwrap(),
            luaL_callmeta: *library.get(b"luaL_callmeta").unwrap(),
            luaL_typerror: *library.get(b"luaL_typerror").unwrap(),
            luaL_argerror: *library.get(b"luaL_argerror").unwrap(),
            luaL_checklstring: *library.get(b"luaL_checklstring").unwrap(),
            luaL_optlstring: *library.get(b"luaL_optlstring").unwrap(),
            luaL_checknumber: *library.get(b"luaL_checknumber").unwrap(),
            luaL_optnumber: *library.get(b"luaL_optnumber").unwrap(),
            luaL_checkinteger: *library.get(b"luaL_checkinteger").unwrap(),
            luaL_optinteger: *library.get(b"luaL_optinteger").unwrap(),
            luaL_checkstack_: *library.get(b"luaL_checkstack").unwrap(),
            luaL_checktype: *library.get(b"luaL_checktype").unwrap(),
            luaL_checkany: *library.get(b"luaL_checkany").unwrap(),
            luaL_newmetatable_: *library.get(b"luaL_newmetatable").unwrap(),
            luaL_checkudata: *library.get(b"luaL_checkudata").unwrap(),
            luaL_where: *library.get(b"luaL_where").unwrap(),
            luaL_checkoption: *library.get(b"luaL_checkoption").unwrap(),
            luaL_ref: *library.get(b"luaL_ref").unwrap(),
            luaL_unref: *library.get(b"luaL_unref").unwrap(),
            luaL_loadfile: *library.get(b"luaL_loadfile").unwrap(),
            luaL_loadbuffer: *library.get(b"luaL_loadbuffer").unwrap(),
            luaL_loadstring: *library.get(b"luaL_loadstring").unwrap(),
            luaL_newstate: *library.get(b"luaL_newstate").unwrap(),
            luaL_gsub: *library.get(b"luaL_gsub").unwrap(),
            luaL_findtable: *library.get(b"luaL_findtable").unwrap(),
            luaL_buffinit: *library.get(b"luaL_buffinit").unwrap(),
            luaL_prepbuffer: *library.get(b"luaL_prepbuffer").unwrap(),
            luaL_addlstring: *library.get(b"luaL_addlstring").unwrap(),
            luaL_addstring: *library.get(b"luaL_addstring").unwrap(),
            luaL_addvalue: *library.get(b"luaL_addvalue").unwrap(),
            luaL_pushresult: *library.get(b"luaL_pushresult").unwrap(),
            luaopen_base: *library.get(b"luaopen_base").unwrap(),
            luaopen_table: *library.get(b"luaopen_table").unwrap(),
            luaopen_io: *library.get(b"luaopen_io").unwrap(),
            luaopen_os: *library.get(b"luaopen_os").unwrap(),
            luaopen_string: *library.get(b"luaopen_string").unwrap(),
            luaopen_math: *library.get(b"luaopen_math").unwrap(),
            luaopen_debug: *library.get(b"luaopen_debug").unwrap(),
            luaopen_package: *library.get(b"luaopen_package").unwrap(),
            luaopen_bit: *library.get(b"luaopen_bit").unwrap(),
            luaopen_jit: *library.get(b"luaopen_jit").unwrap(),
            luaopen_ffi: *library.get(b"luaopen_ffi").unwrap(),
            luaL_openlibs: *library.get(b"luaL_openlibs").unwrap(),
        }
    }
}


//
// Garbage-collection function and options
//
pub const LUA_GCSTOP: c_int = 0;
pub const LUA_GCRESTART: c_int = 1;
pub const LUA_GCCOLLECT: c_int = 2;
pub const LUA_GCCOUNT: c_int = 3;
pub const LUA_GCCOUNTB: c_int = 4;
pub const LUA_GCSTEP: c_int = 5;
pub const LUA_GCSETPAUSE: c_int = 6;
pub const LUA_GCSETSTEPMUL: c_int = 7;

//
// Miscellaneous functions
//

// lua_error does not return but is declared to return int, and Rust translates
// ! to void which can cause link-time errors if the platform linker is aware
// of return types and requires they match (for example: wasm does this).
#[inline(always)]
pub unsafe fn lua_error(L: *mut lua_State) -> ! {
    lua_error_(L);
    unreachable!();
}

//
// Some useful macros (implemented as Rust functions)
//
#[inline(always)]
pub unsafe fn lua_pop(L: *mut lua_State, n: c_int) {
    lua_settop(L, -n - 1)
}

#[inline(always)]
pub unsafe fn lua_newtable(L: *mut lua_State) {
    lua_createtable(L, 0, 0)
}

#[inline(always)]
pub unsafe fn lua_register(L: *mut lua_State, n: *const c_char, f: lua_CFunction) {
    lua_pushcfunction(L, f);
    lua_setglobal(L, n)
}

#[inline(always)]
pub unsafe fn lua_pushcfunction(L: *mut lua_State, f: lua_CFunction) {
    lua_pushcclosure(L, f, 0)
}

#[inline(always)]
pub unsafe fn lua_strlen(L: *mut lua_State, i: c_int) -> usize {
    lua_objlen(L, i)
}

#[inline(always)]
pub unsafe fn lua_isfunction(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TFUNCTION) as c_int
}

#[inline(always)]
pub unsafe fn lua_istable(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TTABLE) as c_int
}

#[inline(always)]
pub unsafe fn lua_islightuserdata(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TLIGHTUSERDATA) as c_int
}

#[inline(always)]
pub unsafe fn lua_isnil(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TNIL) as c_int
}

#[inline(always)]
pub unsafe fn lua_isboolean(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TBOOLEAN) as c_int
}

#[inline(always)]
pub unsafe fn lua_isthread(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TTHREAD) as c_int
}

#[inline(always)]
pub unsafe fn lua_isnone(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) == LUA_TNONE) as c_int
}

#[inline(always)]
pub unsafe fn lua_isnoneornil(L: *mut lua_State, n: c_int) -> c_int {
    (lua_type(L, n) <= LUA_TNIL) as c_int
}

#[inline(always)]
pub unsafe fn lua_pushliteral(L: *mut lua_State, s: &'static CStr) {
    lua_pushstring_(L, s.as_ptr());
}

#[inline(always)]
pub unsafe fn lua_setglobal(L: *mut lua_State, var: *const c_char) {
    lua_setfield(L, LUA_GLOBALSINDEX, var)
}

#[inline(always)]
pub unsafe fn lua_getglobal_(L: *mut lua_State, var: *const c_char) {
    lua_getfield_(L, LUA_GLOBALSINDEX, var)
}

#[inline(always)]
pub unsafe fn lua_tolightuserdata(L: *mut lua_State, idx: c_int) -> *mut c_void {
    if lua_islightuserdata(L, idx) != 0 {
        return lua_touserdata(L, idx);
    }
    ptr::null_mut()
}

#[inline(always)]
pub unsafe fn lua_tostring(L: *mut lua_State, i: c_int) -> *const c_char {
    lua_tolstring(L, i, ptr::null_mut())
}

#[inline(always)]
pub unsafe fn lua_xpush(from: *mut lua_State, to: *mut lua_State, idx: c_int) {
    lua_pushvalue(from, idx);
    lua_xmove(from, to, 1);
}

//
// Debug API
//

// Maximum size for the description of the source of a function in debug information.
const LUA_IDSIZE: usize = 60;

// Event codes
pub const LUA_HOOKCALL: c_int = 0;
pub const LUA_HOOKRET: c_int = 1;
pub const LUA_HOOKLINE: c_int = 2;
pub const LUA_HOOKCOUNT: c_int = 3;
pub const LUA_HOOKTAILCALL: c_int = 4;

// Event masks
pub const LUA_MASKCALL: c_int = 1 << (LUA_HOOKCALL as usize);
pub const LUA_MASKRET: c_int = 1 << (LUA_HOOKRET as usize);
pub const LUA_MASKLINE: c_int = 1 << (LUA_HOOKLINE as usize);
pub const LUA_MASKCOUNT: c_int = 1 << (LUA_HOOKCOUNT as usize);

/// Type for functions to be called on debug events.
pub type lua_Hook = unsafe extern "C-unwind" fn(L: *mut lua_State, ar: *mut lua_Debug);

#[repr(C)]
pub struct lua_Debug {
    pub event: c_int,
    pub name: *const c_char,
    pub namewhat: *const c_char,
    pub what: *const c_char,
    pub source: *const c_char,
    pub currentline: c_int,
    pub nups: c_int,
    pub linedefined: c_int,
    pub lastlinedefined: c_int,
    pub short_src: [c_char; LUA_IDSIZE],
    // lua.h mentions this is for private use
    i_ci: c_int,
}
