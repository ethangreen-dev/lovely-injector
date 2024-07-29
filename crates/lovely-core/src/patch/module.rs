use std::{ffi::CString, fs, path::PathBuf};
use std::ptr::null;
use crate::sys::{self, LuaState};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModulePatch {
    pub source: PathBuf,
    pub before: String,
    pub name: String,
}

impl ModulePatch {
    /// Apply a module patch by loading the input file(s) into memory, calling lual_loadbuffer
    /// on them, and then injecting them into the global `package.loaded` table.
    /// 
    /// # Safety
    /// This function is unsafe as it interfaces directly with a series of dynamically loaded
    /// native lua functions.
    pub unsafe fn apply<F: Fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32>(
        &self, 
        file_name: &str, 
        state: *mut LuaState, 
        lual_loadbuffer: &F,
    ) -> bool {
        // Stop if we're not at the correct insertion point.
        if self.before != file_name {
            return false;
        }

        // Read the source file in, converting it to a CString and computing its nulled length.
        let source = fs::read_to_string(&self.source)
            .unwrap_or_else(|e| panic!("Failed to read patch file at {:?}: {e:?}", &self.source));


        let buf_cstr = CString::new(source.as_str()).unwrap();
        let buf_len = buf_cstr.as_bytes().len();

        let name = format!("@{file_name}");
        let name_cstr = CString::new(name).unwrap();

        // Push the global package.loaded table onto the top of the stack, saving its index.
        let stack_top = sys::lua_gettop(state);
        sys::lua_getfield(state, sys::LUA_GLOBALSINDEX, b"package\0".as_ptr() as _);
        sys::lua_getfield(state, -1, b"loaded\0".as_ptr() as _);

        // This is the index of the `package.loaded` table.
        let field_index = sys::lua_gettop(state);

        // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
        lual_loadbuffer(state, buf_cstr.into_raw() as _, buf_len as _, name_cstr.into_raw() as _, null());

        sys::lua_pcall(state, 0, -1, 0);

        // Insert pcall results onto the package.loaded global table.
        let module_cstr = CString::new(self.name.clone()).unwrap();

        sys::lua_setfield(state, field_index, module_cstr.into_raw() as _);
        sys::lua_settop(state, stack_top);

        true
    }
}
