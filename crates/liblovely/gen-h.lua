local sys_file = assert(io.open("../mlua-sys/src/lua51/lua.rs"))
local inMacro = false

local types = {
	LuaState = "lua_State",
	LuaFunc = "lua_CFunction",
	c_int = "int",
	c_char = "char",
	c_void = "void",
	char = "char",
	usize = "size_t",
	isize = "ssize_t",
	f64 = "double",
	VaList = "va_list",
	-- HACK: We love pointer pointers (i'm to lazy to fix this properly)
	["*mut c_void"] = "void*",
	["*const c_char"] = "char*",
}

-- Direct maps (as mlua-sys uses)
for _, v in ipairs{
	"lua_Alloc",
	"lua_State",
	"lua_CFunction",
	"lua_Number",
	"lua_Integer",
	"lua_Reader",
	"lua_Writer",
	"lua_Debug",
	"lua_Hook",
	"luaL_Reg",
	"luaL_Buffer",
} do
	assert(not types[v], "Duplicated type register");
	types[v] = v
end

local functionPatches = { -- Functions where rust has different types
	luaL_checkoption = {
		args = {
			[4] = "const char *const lst[]"
		}
	},
}

local function convertType(str, name)
	local pointer, mod, type = str:match("(%*)(%S+)%s+(.+)")
	if not pointer then
		str = str:match("^Option<([%w_]+)>$") or str -- Drop options (NULLPTR)
		type = types[str]
		assert(type, "Unknown type " .. str)
		if name then
			return type .. " " .. name
		else
			return type
		end
	end
	if mod ~= "const" then
		mod = ""
	end
	type = assert(types[type], "Unknown type " .. type)
	if #mod ~= 0 then
		type = mod .. " " .. type
	end
	if name then
		type = type .. " "
	end
	if #pointer ~= 0 then
		type = type .. "*"
	end
	if name then
		type = type .. name
	end
	return type
end

local functions = {
	{
		name = "luaL_loadbufferx",
		args = {
			"lua_State *L",
			"const char *buff",
			"size_t sz",
			"const char *name",
			"const char *mode",
		},
		ret = "int",
	},
}

local luaLib = ""
local names = {}
local funcMatch = "p?u?b? unsafe extern \"C%-unwind\" fn ([%w_]+)(%b())([^;]*)"

for line in sys_file:lines() do
	if not inMacro then
		if line == "generate! (LuaLib {" then inMacro = true end
	elseif line:match("^%s*//") or line:match("^%s*$") then
		-- Comment or blank line, do nothing
	elseif line:match("%} raw %{") then
		-- Raw functions are defined a bit differently
		funcMatch = "p?u?b? ?([%w_]+): unsafe extern \"C%-unwind\" fn(%b())([^,]*)"
	else
		if line == "});" then break end
		local name, argsStr, ret = line:match(funcMatch)
		assert(name, "Line did not match pattern!\n" .. line)
		name = name:gsub("lual", "luaL")
		name = name:gsub("_$", "")
		local args = {}
		for name, type in string.gmatch(argsStr, "([%w_]+):%s(.-)[,)]") do
			table.insert(args, convertType(type, name))
		end
		ret = ret:match("-> (.+)")
		if ret then
			ret = convertType(ret)
		else
			ret = "void"
		end
		table.insert(functions, {
			name = name,
			args = args,
			ret = ret,
		})
		table.insert(names, name)
		luaLib = luaLib .. "    " .. name .. "_ptr " .. name .. ";\n"
	end
end
sys_file:close()

for k, f in pairs(functions) do
	local patches = functionPatches[f.name]
	if patches then
		for i, v in pairs(patches.args) do
			f.args[i] = v
		end
	end
end

local function genTypeDef(func)
	local args = ""
	for k, v in ipairs(func.args) do
		if args ~= "" then
			args = args .. ", "
		end
		args = args .. v
	end
	return "typedef " .. func.ret .. " (*" .. func.name .. "_ptr)(" .. args .. ");"
end

local function genTypeDefs(funcs)
	local str = ""
	for k, v in ipairs(funcs) do
		str = str .. genTypeDef(v) .. "\n"
	end
	return str
end

local file = [[// This file was generated using gen-h.lua

#ifndef LOVELY_H
#define LOVELY_H

]] .. genTypeDefs(functions) .. [[

struct LuaLib {
]] .. luaLib ..
[[};

void lovely_init(luaL_loadbufferx_ptr, struct LuaLib);

int lovely_apply_patches(lua_State *L, const char *buff, size_t sz, const char *name, const char *mode);
#endif // LOVELY_H
]]

local out_file = assert(io.open("lovely.h", "w"))
out_file:write(file)
out_file:close()

print "Lovely.h updated"
print "If functions were added/removed, here is the declartion for the struct for you."
print ""
print("struct LuaLib lua = {" .. table.concat(names, ", ") .. "};")
