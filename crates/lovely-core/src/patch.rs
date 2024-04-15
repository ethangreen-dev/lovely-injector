use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use std::path::PathBuf;

use once_cell::sync::Lazy;
use regex_cursor::engines::meta::Regex;
use regex_cursor::regex_automata::util::interpolate;
use regex_cursor::regex_automata::util::syntax::Config;
use regex_cursor::{Input, IntoCursor};
use ropey::Rope;
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
    // let re: Lazy<regex_lite::Regex> = Lazy::new(|| regex_lite::Regex::new(r"\{\{lovely:(\w+)\}\}").unwrap());
    
    // let line_copy = line.to_string();
    // let captures = re
    //     .captures_iter(&line_copy).map(|x| x.extract());

    // for (cap, [var]) in captures {
    //     let Some(val) = vars.get(var) else {
    //         panic!("Failed to interpolate an unregistered variable '{var}'");
    //     };

    //     // This clones the string each time, not efficient. A more efficient solution
    //     // would be to use something like mem::take to interpolate the string in-place,
    //     // but the complexity would not be worth the performance gain.
    //     *line = line.replace(cap, val);
    // }
}

impl PatternPatch {
    /// Apply the pattern patch onto the rope.
    /// The return value will be `true` if the rope was modified.
    pub fn apply(&self, target: &str, rope: &mut Rope) -> bool {
        if self.target != target {
            return false;
        }

        let wm = WildMatch::new(&self.pattern);
        let matches = rope
            .lines()
            .enumerate()
            .map(|(i, line)| (i, line.to_string()))
            .filter(|(_, line)| wm.matches(line.trim()))
            .collect::<Vec<(_, _)>>();

        if matches.is_empty() {
            return false;
        }

        // Track the +/- index offset caused by previous line injections.
        let mut line_delta = 0;

        for (line_idx, line) in matches {
            let start = rope.line_to_byte(line_idx) + line_delta;
            let end = start + line.len();
            let payload_lines = self.payload.lines().count();

            let indent = if self.match_indent {
                line.chars().take_while(|x| *x == ' ' || *x == '\t').collect::<String>()
            } else {
                String::new()
            };

            let payload = self.payload.split('\n')
                .map(|x| format!("{indent}{x}"))
                .collect::<Vec<_>>()
                .join("\n");

            let newline = if self.payload.ends_with('\n') {
                ""
            } else {
                "\n"
            };

            let replace = match self.position {
                PatternAt::Before => { 
                    line_delta += payload_lines;
                    format!("{}{newline}{line}", payload)
                }
                PatternAt::After => {
                    line_delta += payload_lines;
                    format!("{line}{}{newline}", payload)
                }
                PatternAt::At => {
                    line_delta += payload_lines - 1;
                    format!("{indent}{}{newline}", payload)
                }
            };

            rope.remove(start..end);
            rope.insert(start, &replace);
        }

        true
    }

    pub fn apply_complex(&self, target: &str, rope: &mut Rope) -> bool {
        if self.target != target {
            return false;
        }

        let input = Input::new(rope.into_cursor());
        let re = Regex::new(&self.pattern)
            .unwrap_or_else(|e| panic!("Failed to compile Regex pattern '{}': {e:?}", self.pattern));

        let captures = re.captures_iter(input).collect::<Vec<_>>();
        if captures.is_empty() {
            log::info!("Regex query '{}' on target '{target}' did not result in any matches", self.pattern);
            return false;
        }

        // This is our running byte offset. We use this to ensure that byte references
        // within the capture group remain valid even after the rope has been mutated.
        let mut delta = 0_isize;

        for groups in captures {
            // Get the entire captured span (index 0);
            let base = groups.get_group(0).unwrap();
            let base_start = (base.start as isize + delta) as usize;
            let base_end = (base.end as isize + delta) as usize;

            let base_str = rope.get_byte_slice(base_start..base_end).unwrap().to_string();

            // Interpolate capture groups into the payload.
            // We must use this method instead of Captures::interpolate_string because that
            // implementation seems to be broken when working with ropes.
            let mut payload = String::new();
            interpolate::string(
                &self.payload,
                |index, dest| {
                    let span = groups.get_group(index).unwrap();
                    let start = (span.start as isize + delta) as usize;
                    let end = (span.end as isize + delta) as usize;

                    let rope_slice = rope.get_byte_slice(start..end).unwrap();

                    dest.push_str(&rope_slice.to_string());
                },
                |name| {
                    let pid = groups.pattern().unwrap();
                    groups.group_info().to_index(pid, name)
                },
                &mut payload
            );

            let char_start = rope.byte_to_char(base_start);
            let char_end = rope.byte_to_char(base_end);

            rope.remove(char_start..char_end);
            rope.insert(char_start, &payload);

            let new_len = payload.len();
            let old_len = base.end - base.start;

            delta += new_len as isize - old_len as isize;
        }

        true
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
