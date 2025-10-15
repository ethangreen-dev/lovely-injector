use std::{
    ffi::CString,
    fs,
    path::{Path, PathBuf},
    ptr,
};

use crate::sys::{self, lua_identity_closure, LuaState};
use serde::{Deserialize, Serialize};
use crate::RUNTIME;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModulePatch {
    pub source: PathBuf,
    // Only has meaning if `load_now` is true. Evaluate the module immediately before
    // this file.
    #[serde(default)]
    pub before: Option<String>,
    pub name: String,

    // If enabled, evaluate the module immediately upon loading it
    #[serde(default)]
    pub load_now: bool,
    // Used for the display name of the source. Is the relative path to the source.
    #[serde(skip)]
    pub display_source: String,
}

impl ModulePatch {
    /// Apply a module patch by loading the input file(s) into memory, calling lual_loadbufferx
    /// on them, and then injecting them into the global `package.preload` table.
    ///
    /// # Safety
    /// This function is unsafe as it interfaces directly with a series of dynamically loaded
    /// native lua functions.
    pub unsafe fn apply(
        &self,
        file_name: &str,
        state: *mut LuaState,
        path: &Path,
    ) -> bool {
        // Stop if we're not at the correct insertion point.
        if self.load_now && self.before.as_ref().unwrap() != file_name {
            return false;
        }

        // Read the source file in as [u8]
        let source = fs::read(&self.source).unwrap_or_else(|e| {
            panic!(
                "Failed to read module source file for module patch from {} at {:?}: {e:?}",
                path.display(),
                &self.source
            )
        });
        let source_len = source.len();

        let name = format!("=[lovely {} \"{}\"]", &self.name, &self.display_source);
        let name_cstr = CString::new(name).unwrap();

        // Push the global package.preload table onto the top of the stack, saving its index.
        let stack_top = sys::lua_gettop(state);
        sys::lua_getfield(state, sys::LUA_GLOBALSINDEX, c"package".as_ptr() as _);
        sys::lua_getfield(state, -1, c"preload".as_ptr() as _);

        // This is the index of the `package.preload` table.
        let field_index = sys::lua_gettop(state);

        // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
        
        let lovely = &RUNTIME.get().unwrap();

        let return_code = lovely.apply_buffer_patches(
            state,
            source.as_ptr(),
            source_len,
            name_cstr.into_raw() as _,
            ptr::null(),
        );

        if return_code != 0 {
            log::error!(
                "Failed to load module {} for module patch from {}",
                self.name,
                path.display()
            );
            sys::lua_settop(state, stack_top);
            return false;
        }

        if self.load_now {
            // Evaluate the results of the buffer now
            let return_code = sys::lua_pcall(state, 0, 1, 0);
            if return_code != 0 {
                log::error!(
                    "Evaluation of module {} failed for module patch from {}",
                    self.name,
                    path.display()
                );
                sys::lua_settop(state, stack_top);
                return false;
            }
            // Wrap this in the identity closure function
            sys::lua_pushcclosure(state, lua_identity_closure, 1);
        }

        // Insert results onto the package.preload global table.
        let module_cstr = CString::new(self.name.clone()).unwrap();
        sys::lua_setfield(state, field_index, module_cstr.into_raw() as _);
        // Always ensure that the lua stack is in good order
        sys::lua_settop(state, stack_top);
        true
    }
}
