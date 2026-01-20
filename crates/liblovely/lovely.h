// This file was generated using gen-h.lua

#ifndef LOVELY_H
#define LOVELY_H

typedef int (*luaL_loadbufferx_ptr)(lua_State *L, const char *buff, size_t sz, const char *name, const char *mode);
typedef void (*lua_call_ptr)(lua_State *state, int nargs, int nresults);
typedef int (*lua_pcall_ptr)(lua_State *state, int nargs, int nresults, int errfunc);
typedef void (*lua_getfield_ptr)(lua_State *state, int index, const char *k);
typedef void (*lua_setfield_ptr)(lua_State *state, int index, const char *k);
typedef int (*lua_gettop_ptr)(lua_State *state);
typedef void (*lua_settop_ptr)(lua_State *state, int index);
typedef void (*lua_pushvalue_ptr)(lua_State *state, int index);
typedef void (*lua_pushcclosure_ptr)(lua_State *state, lua_CFunction f, int n);
typedef const char* (*lua_tolstring_ptr)(lua_State *state, int index, size_t *len);
typedef int (*lua_type_ptr)(lua_State *state, int index);
typedef void (*lua_pushstring_ptr)(lua_State *state, const char *string);
typedef void (*lua_pushnumber_ptr)(lua_State *state, double number);
typedef void (*lua_pushboolean_ptr)(lua_State *state, int b);
typedef void (*lua_settable_ptr)(lua_State *state, int index);
typedef void (*lua_createtable_ptr)(lua_State *state, int narr, int nrec);
typedef int (*lua_error_ptr)(lua_State *state);
typedef void (*luaL_register_ptr)(lua_State *state, const char *libname, const luaL_Reg *l);
typedef const char* (*luaL_checklstring_ptr)(lua_State *state, int index, size_t *len);

struct LuaLib {
    lua_call_ptr lua_call;
    lua_pcall_ptr lua_pcall;
    lua_getfield_ptr lua_getfield;
    lua_setfield_ptr lua_setfield;
    lua_gettop_ptr lua_gettop;
    lua_settop_ptr lua_settop;
    lua_pushvalue_ptr lua_pushvalue;
    lua_pushcclosure_ptr lua_pushcclosure;
    lua_tolstring_ptr lua_tolstring;
    lua_type_ptr lua_type;
    lua_pushstring_ptr lua_pushstring;
    lua_pushnumber_ptr lua_pushnumber;
    lua_pushboolean_ptr lua_pushboolean;
    lua_settable_ptr lua_settable;
    lua_createtable_ptr lua_createtable;
    lua_error_ptr lua_error;
    luaL_register_ptr luaL_register;
    luaL_checklstring_ptr luaL_checklstring;
};

void lovely_init(luaL_loadbufferx_ptr, struct LuaLib);

int lovely_apply_patches(lua_State *L, const char *buff, size_t sz, const char *name, const char *mode);
#endif // LOVELY_H
