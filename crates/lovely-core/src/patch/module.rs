use std::{ffi::CString, fs, path::PathBuf, ptr, slice};

use crate::sys::{self, LuaState};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModulePatch {
    pub source: PathBuf,
    pub before: String,
    pub name: String,
}

impl ModulePatch {
    /// Apply a module patch by loading the input file(s) into memory, calling lual_loadbuffer
    /// on them, and then injecting them into the global `package.preload` table.
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

        // Push the global package.preload table onto the top of the stack, saving its index.
        let stack_top = sys::lua_gettop(state);
        sys::lua_getfield(state, sys::LUA_GLOBALSINDEX, b"package\0".as_ptr() as _);
        sys::lua_getfield(state, -1, b"preload\0".as_ptr() as _);

        // This is the index of the `package.preload` table.
        let field_index = sys::lua_gettop(state);

        // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
        lual_loadbuffer(
            state,
            buf_cstr.into_raw() as _,
            buf_len as _,
            name_cstr.into_raw() as _,
            ptr::null(),
        );

        let return_code = sys::lua_pcall(state, 0, -1, 0);

        if return_code == 0 {
            // Call succeeded, insert results onto the package.preload global table.
            let module_cstr = CString::new(self.name.clone()).unwrap();

            sys::lua_setfield(state, field_index, module_cstr.into_raw() as _);
            sys::lua_settop(state, stack_top);

            true
        } else {
            // We might quietly convert a number to a string, but this is fine,
            // as we're just printing it as a diagnostic, and popping it off
            // immediately afterwards
            let mut error_cstr_len = 0;
            let error_cstr = sys::lua_tolstring(state, field_index, &mut error_cstr_len);
            if !error_cstr.is_null() {
                log::error!(
                    "Loading module {} failed with error {}",
                    self.name,
                    String::from_utf8_lossy(slice::from_raw_parts(
                        error_cstr as *const u8,
                        error_cstr_len.try_into().unwrap()
                    ))
                );
            } else {
                log::error!("Loading module {} failed with unknown error", self.name);
            }
            sys::lua_settop(state, stack_top);
            false
        }
    }
}
