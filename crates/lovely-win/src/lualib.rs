use libloading::Library;
use lovely_core::sys::LuaLib;


pub unsafe fn get_lualib() -> LuaLib {
    let lua = Library::new("lua51.dll").unwrap();
    LuaLib {
        lua_call: *lua.get(b"lua_call").unwrap(),
        lua_pcall: *lua.get(b"lua_pcall").unwrap(),
        lua_getfield: *lua.get(b"lua_getfield").unwrap(),
        lua_setfield: *lua.get(b"lua_setfield").unwrap(),
        lua_gettop: *lua.get(b"lua_gettop").unwrap(),
        lua_settop: *lua.get(b"lua_settop").unwrap(),
        lua_pushvalue: *lua.get(b"lua_pushvalue").unwrap(),
        lua_pushcclosure: *lua.get(b"lua_pushcclosure").unwrap(),
        lua_tolstring: *lua.get(b"lua_tolstring").unwrap(),
        lua_toboolean: *lua.get(b"lua_toboolean").unwrap(),
        lua_topointer: *lua.get(b"lua_topointer").unwrap(),
        lua_type: *lua.get(b"lua_type").unwrap(),
        lua_typename: *lua.get(b"lua_typename").unwrap(),
        lua_isstring: *lua.get(b"lua_isstring").unwrap(),
    }
}
