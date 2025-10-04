#ifndef LOVELY_H
#define LOVELY_H

void lovely_init(void *loadbufferx, void *lua_call, void *lua_pcall, void *lua_getfield, void *lua_setfield, void *lua_gettop, void *lua_settop, void *lua_pushvalue, void *lua_pushcclosure, void *lua_tolstring, void *lua_createtable, void *lua_pushboolean, void *lua_pushnumber, void *lua_pushstring, void *lua_settable, void *lua_type, void *luaL_register);

int lovely_apply_patches(lua_State *L, const char *buff, size_t sz,
    const char *name, const char *mode);
#endif // LOVELY_H
