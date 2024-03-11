use std::ffi::{CString, c_void};
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;

use once_cell::sync::OnceCell;
use regex_lite::Regex;
use wildmatch::WildMatch;
use windows::core::s;

use crate::manifest::{CopyAt, CopyPatch, ModulePatch, Patch, PatternAt, PatternPatch};
use crate::{sys, LuaLoadbuffer_Detour};

pub static PATCHES: OnceCell<Vec<Patch>> = OnceCell::new();
pub static PATCH_TABLE: OnceCell<HashMap<String, Vec<usize>>> = OnceCell::new();

pub static VAR_TABLE: OnceCell<HashMap<String, String>> = OnceCell::new();

pub fn is_patch_target(name: &str) -> bool {
    let name = name.replacen('@', "", 1);
    PATCH_TABLE.get().unwrap().get(&name).is_some()
}

pub fn apply(input: &str, name: &str) -> Option<String> {
    let name = name.replacen('@', "", 1);
    let patches = PATCH_TABLE
        .get()
        .expect("Failed to get PATCH_TABLE, this is a bug")
        .get(&name)?
        .iter()
        .map(|x| PATCHES.get().unwrap().get(*x))
        .collect::<Vec<_>>();

    // Is this superfluous? Yes.
    if patches.len() == 1 {
        println!("[LOVELY] Applying 1 patch to '{name}'");
    } else {
        println!("[LOVELY] Applying {} patches to '{name}'", patches.len());
    }

    let pattern_patches = patches.iter().filter_map(|patch| match patch {
        Some(Patch::Pattern(x)) => Some(x),
        _ => None,
    }).collect::<Vec<_>>();

    let lines = input.lines();
    let mut out = Vec::new();

    for line in lines {
        let mut new_line = apply_pattern_patches(&line, &pattern_patches[..]);
        out.append(&mut new_line);
    }

    let copy_patches = patches.iter().filter_map(|patch| match patch {
        Some(Patch::Copy(x)) => Some(x),
        _ => None,
    }).collect::<Vec<_>>();

    let out = out.join("\n");
    let out = apply_copy_patches(&out, &copy_patches[..])
        .lines()
        .map(|x| apply_var_interp(x))
        .collect::<Vec<_>>()
        .join("\n");

    Some(out)
}

fn apply_var_interp(line: &str) -> String {
    let re = Regex::new(r"\{\{lovely:(\w+)\}\}");
    let Ok(re) = re else {
        panic!("A regex error occured: {re:?}");
    };

    let var_table = VAR_TABLE.get().unwrap();
    let mut working = line.to_string();
    for (cap, [var]) in re.captures_iter(line).map(|x| x.extract()) {
        let Some(val) = var_table.get(var) else {
            panic!("Attempted to interpolate an unregistered variable '{var}'");
        };      

        // This clones the string each time. Not efficient.
        working = working.replace(cap, val);
    }

    working
}

fn apply_pattern_patches(line: &str, patches: &[&PatternPatch]) -> Vec<String> {
    // Perform pattern matching for each patch.
    let trimmed = line.trim_start();
    let matches = patches
        .iter()
        .filter(|x| WildMatch::new(&x.pattern).matches(trimmed));

    let mut line = line.to_string();
    let mut before: Vec<String> = Vec::new();
    let mut after: Vec<String> = Vec::new();

    for patch in matches {
        let indent = if patch.match_indent {
            line.chars().take_while(|x| *x == ' ' || *x == '\t').collect::<String>()
        } else {
            String::new()
        };

        let payload_ref = patch.payload.as_ref().unwrap();
        let mut payload_lines = payload_ref.split('\n')
            .map(|x| format!("{indent}{x}"))
            .collect::<Vec<_>>();
        // let payload = format!("{indent}{}", patch.payload.as_ref().unwrap());
        match patch.position {
            PatternAt::At => {
                let payload = format!("{indent}{}", patch.payload.as_ref().unwrap());
                line = payload
            }
            PatternAt::After => {
                after.append(&mut payload_lines)
            },
            PatternAt::Before => {
                before.append(&mut payload_lines)
            },
        }
    }

    before.push(line);
    before.append(&mut after);
    before
}


fn apply_copy_patches(input: &str, patches: &[&CopyPatch]) -> String {
    let mut out = input.to_string();

    for patch in patches {
        let payload = merge_payloads(&patch.sources);
        match patch.position {
            CopyAt::Append => {
                out = format!("{out}\n{payload}")
            },
            CopyAt::Prepend => {
                out = format!("{payload}\n{out}")
            }
        }
    }

    out
}
// Load the target path into the game as a new "file".
pub unsafe fn load_file(patch: &ModulePatch, lua_state: *mut c_void) {
    let src = &patch.source;
    let contents = fs::read_to_string(src)
        .unwrap_or_else(|_| panic!("Failed to read patch source at '{src:?}'"));

    let name = src.file_name().unwrap().to_string_lossy();
    print!("[LOVELY] Applying module injection for '{name}'");

    let buf = CString::new(contents).unwrap();
    let buf_len = buf.as_bytes().len();

    let name = format!("@{name}");
    let name_buf = CString::new(name).unwrap();

    let top = sys::lua_gettop(lua_state);

    // Push the global package.loaded table onto the stack, saving its index.
    sys::lua_getfield(lua_state, sys::LUA_GLOBALSINDEX, s!("package").0 as _);
    sys::lua_getfield(lua_state, -1, s!("loaded").0 as _);
    let field_index = sys::lua_gettop(lua_state);

    // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
    LuaLoadbuffer_Detour.call(lua_state, buf.into_raw() as _, buf_len as _, name_buf.into_raw() as _);
    let status = sys::lua_pcall(lua_state as _, 0, -1, 0);

    // Insert the top of the stack into package.loaded global table.
    let module_name = CString::new(patch.name.clone())
        .unwrap_or_else(|_| panic!("Module name '{}' contains malformed or otherwise invalid utf8.", patch.name));
    
    sys::lua_setfield(lua_state, field_index, module_name.into_raw() as _);
    sys::lua_settop(lua_state, top);

    println!(" - OK ({status:x?})");
}

fn merge_payloads(sources: &Vec<PathBuf>) -> String {
    let mut merged = Vec::new();
    for source in sources {
        let contents = fs::read_to_string(&source)
            .unwrap_or_else(|_| panic!("Failed to read payload file at '{source:?}'."));

        merged.push(contents);
    }

    merged.join("\n")
}
