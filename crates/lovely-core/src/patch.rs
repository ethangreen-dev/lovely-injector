use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use std::path::PathBuf;

use once_cell::sync::Lazy;
use regex_lite::Regex;
use wildmatch::WildMatch;

use crate::manifest::{CopyAt, CopyPatch, ModulePatch, PatternAt, PatternPatch};
use crate::sys::{self, LuaState};

// This contains the cached contents of one or more source files. We use to reduce 
// runtime cost as we're now possibly reading from files EVERY line and not all at once.
static mut FILE_CACHE: Lazy<HashMap<PathBuf, String>> = Lazy::new(HashMap::new);

fn get_cached_file(path: &PathBuf) -> Option<&String> {
    unsafe {
        FILE_CACHE.get(path)
    }
}

fn set_cached_file(path: &PathBuf) -> &String {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read patch file at {path:?}: {e:?}"));

    unsafe {
        FILE_CACHE.insert(path.clone(), contents);
        FILE_CACHE.get(path).unwrap()
    }
}

/// Apply valid var interpolations to the provided line.
/// Interpolation targets are of form {{lovely:VAR_NAME}}.
pub fn apply_var_interp(line: &mut String, vars: &HashMap<String, String>) {
    // Cache the compiled regex.
    let re: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{\{lovely:(\w+)\}\}").unwrap());
    
    let line_copy = line.to_string();
    let captures = re
        .captures_iter(&line_copy).map(|x| x.extract());

    for (cap, [var]) in captures {
        let Some(val) = vars.get(var) else {
            panic!("Failed to interpolate an unregistered variable '{var}'");
        };

        // This clones the string each time, not efficient. A more efficient solution
        // would be to use something like mem::take to interpolate the string in-place,
        // but the complexity would not be worth the performance gain.
        *line = line.replace(cap, val);
    }
}

impl PatternPatch {
    /// Apply the pattern patch onto the provided line.
    /// The return value will be Option::Some when the given line was prepended or appended onto.
    /// The vec contains a series of lines that will be inserted in-place, replacing the provided line.
    /// If Option::None, the line itself may or may not have been mutated.
    pub fn apply(&self, target: &str, line: &mut String) -> Option<(Vec<String>, Vec<String>)> {
        let trimmed = line.trim();
        let is_match = WildMatch::new(&self.pattern).matches(trimmed);

        // Stop here if there's no match on this line.
        if !is_match || self.target != target {
            return None;
        }

        // Determine the indent of the provided line. If an indent is not requested use an empty one.
        let indent = if self.match_indent {
            line.chars().take_while(|x| *x == ' ' || *x == '\t').collect::<String>()
        } else {
            String::new()
        };

        // If we're replacing *only* the provided line then we stop here, no need for added allocs.
        if matches!(self.position, PatternAt::At) {
            *line = format!("{indent}{}", self.payload);
            return None;
        }

        let mut payload_lines = self.payload.split('\n')
            .map(|x| format!("{indent}{x}"))
            .collect::<Vec<_>>();

        let mut before = vec![];
        let mut after = vec![];

        // Insert the payload into position in the output vec either *before* or *after*
        // the provided line.
        match self.position {
            PatternAt::Before => {
                before.append(&mut payload_lines);
            }
            PatternAt::After => {
                after.append(&mut payload_lines);
            }
            _ => unreachable!()
        }

        Some((before, after))
    }
}

impl CopyPatch {
    /// Apply a copy patch onto the provided buffer and name.
    /// If the name is *not* a valid target of this patch, return false and do not
    /// modify the buffer.
    /// If the name *is* a valid target of this patch, prepend or append the source file(s) contents
    /// and return true.
    pub fn apply(&self, file_name: &str, buffer: &mut Vec<String>) -> bool {
        if self.target != file_name {
            return false;
        }

        // Merge the provided payloads into a single buffer. Each source path should
        // be made absolute by the patch loader.
        for source in self.sources.iter() {
            let contents = get_cached_file(source).unwrap_or(set_cached_file(source));
            let mut lines = contents.lines().map(String::from).collect::<Vec<_>>();

            // Append or prepend the patch's lines onto the provided buffer.
            match self.position {
                CopyAt::Append => buffer.append(&mut lines),

                // This is horribly inefficient as it pushes the entire contents of the buffer onto
                // the read lines, requiring huge allocations.
                CopyAt::Prepend => {
                    lines.append(buffer);
                    *buffer = lines;
                }
            }
        }

        true
    }
}

impl ModulePatch {
    /// Apply a module patch by loading the input file(s) into memory, calling lual_loadbuffer
    /// on them, and then injecting them into the global `package.loaded` table.
    /// 
    /// # Safety
    /// This function is unsafe as it interfaces directly with a series of dynamically loaded
    /// native lua functions.
    pub unsafe fn apply<F: Fn(*mut LuaState, *const u8, isize, *const u8) -> u32>(
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
        let source = get_cached_file(&self.source).unwrap_or(set_cached_file(&self.source));

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
        lual_loadbuffer(state, buf_cstr.into_raw() as _, buf_len as _, name_cstr.into_raw() as _);

        sys::lua_pcall(state, 0, -1, 0);

        // Insert pcall results onto the package.loaded global table.
        let module_cstr = CString::new(self.name.clone()).unwrap();

        sys::lua_setfield(state, field_index, module_cstr.into_raw() as _);
        sys::lua_settop(state, stack_top);

        true
    }
}
