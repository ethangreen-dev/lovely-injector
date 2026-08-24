#![allow(non_upper_case_globals)]

use core::slice;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr};
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;
use std::{env, fs};
use anyhow::{Context, Result};

use log::*;

use getargs::{Arg, Options};
use itertools::Itertools;
use patch::{ModulePatch, Patch};
use regex_lite::Regex;
use mlua::Lua;

use sys::{lua_CFunction, LuaLib, lua_State, LuaStateTrait, LUA};

use crate::patch::Target;
use crate::dump::write_dump;

pub mod chunk_vec_cursor;
pub mod dump;
pub mod log;
pub mod patch;
pub mod sys;

pub const LOVELY_VERSION: &str = env!("CARGO_PKG_VERSION");

pub static RUNTIME: OnceLock<Lovely> = OnceLock::new();

type LoadBuffer =
    dyn Fn(*mut lua_State, *const u8, usize, *const u8, *const u8) -> i32 + Send + Sync + 'static;


fn reload_patches(_lua: &Lua, _: ()) -> Result<bool> {
    let lovely = &RUNTIME.get().unwrap();
    let new_table = PatchTable::load(&lovely.mod_dir)?;
    let binding = Arc::clone(&lovely.patch_table);
    let mut patch_table = binding.write().unwrap();
    *patch_table = new_table;

    Ok(true)
}

fn get_var(_lua: &Lua, key: String) -> Option<String> {
    let lovely = &RUNTIME.get().unwrap();
    let vars = lovely.lua_vars.read().unwrap();
    let val = vars.get(&key);
    val.cloned()
}

fn set_var(_lua: &Lua, (key, val): (String, String)) {
    let lovely = &RUNTIME.get().unwrap();
    let mut vars = lovely.lua_vars.write().unwrap();
    vars.insert(key, val);
}

fn remove_var(_lua: &Lua, key: String) -> Option<String> {
    let lovely = &RUNTIME.get().unwrap();
    let mut vars = lovely.lua_vars.write().unwrap();
    let val = vars.remove(&key);
    val
}

pub struct Lovely {
    pub mod_dir: PathBuf,
    pub is_vanilla: bool,
    loadbuffer: &'static LoadBuffer,
    patch_table: Arc<RwLock<PatchTable>>,
    dump_all: bool,
    lua_vars: Arc<RwLock<HashMap<String, String>>>,
}

impl Lovely {
    /// Initialize the Lovely patch runtime.
    pub fn init(loadbuffer: &'static LoadBuffer, lualib: LuaLib, dump_all: bool) -> &'static Self {
        assert!(RUNTIME.get().is_none());

        LUA.set(lualib)
            .unwrap_or_else(|_| panic!("LUA static var has already been set."));

        let start = Instant::now();

        let args = std::env::args().skip(1).collect_vec();
        let mut opts = Options::new(args.iter().map(String::as_str));

        let mut mod_dir = if let Some(env_path) = env::var_os("LOVELY_MOD_DIR") {
            PathBuf::from(env_path)
        } else {
            let cur_exe =
                env::current_exe().expect("Failed to get the path of the current executable.");
            let game_name = if env::consts::OS == "macos" {
                cur_exe
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::parent)
                    .expect("Couldn't find parent .app of current executable path")
                    .file_name()
                    .expect("Failed to get file_name of parent directory of current executable")
                    .to_string_lossy()
                    .strip_suffix(".app")
                    .expect("Parent directory of current executable path was not an .app")
                    .replace(".", "_")
            } else {
                cur_exe
                    .file_stem()
                    .expect("Failed to get file_stem component of current executable path.")
                    .to_string_lossy()
                    .replace(".", "_")
            };
            dirs::data_dir().unwrap().join(game_name).join("Mods")
        };

        let mut is_vanilla = false;

        while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
            match opt {
                Arg::Long("mod-dir") => {
                    mod_dir = opts.value().map(PathBuf::from).unwrap_or(mod_dir)
                }
                Arg::Long("vanilla") => is_vanilla = true,
                _ => (),
            }
        }

        let log_dir = mod_dir.join("lovely").join("log");

        log::init(&log_dir).unwrap_or_else(|e| panic!("Failed to initialize logger: {e:?}"));

        info!("Lovely {LOVELY_VERSION}");

        let lua_vars = Arc::new(RwLock::new(HashMap::new()));

        // Stop here if we're running in vanilla mode.
        if is_vanilla {
            info!("Running in vanilla mode");

            let lovely = Lovely {
                mod_dir,
                is_vanilla,
                loadbuffer,
                patch_table: Default::default(),
                dump_all,
                lua_vars,
            };
            RUNTIME
                .set(lovely)
                .unwrap_or_else(|_| panic!("Shit's erroring"));
            return RUNTIME.get().unwrap();
        }

        // Validate that an older Lovely install doesn't already exist within the game directory.
        let exe_path = env::current_exe().unwrap();
        let game_dir = exe_path.parent().unwrap();

        #[cfg(target_os = "windows")]
        {
            let dwmapi = game_dir.join("dwmapi.dll");

            if dwmapi.is_file() {
                panic!(
                    "An old Lovely installation was detected within the game directory. \
                    This problem MUST BE FIXED before you can start the game.\n\nTO FIX: Delete the file at {dwmapi:?}"
                );
            }
        }

        info!("Game directory is at {game_dir:?}");
        info!("Writing logs to {log_dir:?}");

        if !mod_dir.is_dir() {
            info!("Creating mods directory at {mod_dir:?}");
            fs::create_dir_all(&mod_dir).unwrap();
        }

        info!("Using mod directory at {mod_dir:?}");
        let patch_table = Arc::new(RwLock::new(PatchTable::load(&mod_dir).unwrap()));

        // Clean up dump dirs
        for dir_name in ["dump", "game-dump"] {
            let dump_dir = mod_dir.join("lovely").join(dir_name);
            if !dump_dir.is_dir() {
                continue;
            }

            info!("Cleaning up {dir_name} directory at {dump_dir:?}");
            fs::remove_dir_all(&dump_dir).unwrap_or_else(|e| {
                panic!("Failed to recursively delete {dir_name} directory at {dump_dir:?}: {e:?}")
            });
        }

        info!(
            "Initialization complete in {}ms",
            start.elapsed().as_millis()
        );

        let lovely = Lovely {
            mod_dir,
            is_vanilla,
            loadbuffer,
            patch_table,
            dump_all,
            lua_vars,
        };
        RUNTIME
            .set(lovely)
            .unwrap_or_else(|_| panic!("Shit's erroring"));
        RUNTIME.get().unwrap()
    }
    pub unsafe fn apply_buffer_patches(
        &self,
        state: *mut lua_State,
        buf_ptr: *const u8,
        size: usize,
        name_ptr: *const u8,
        mode_ptr: *const u8,
    ) -> i32 {
       let res = self.apply_buffer_patches_internal(state, buf_ptr, size, name_ptr);
       if res.is_err() {
           state.push(format!("{:?}", res.unwrap_err()));
           // NOTE: Not really the most correct error code but it doesn't handle the correcter errors right.
           return sys::LUA_ERRSYNTAX;
       }

       let res = res.unwrap();
       // No patching to be done, run directly
       if res.is_none() {
           return (self.loadbuffer)(state, buf_ptr, size, name_ptr, mode_ptr);
       }

       let patched = res.unwrap();
       (self.loadbuffer)(state, patched.as_ptr(), patched.len(), name_ptr, mode_ptr)
    }

    /// Apply patches onto the raw buffer.
    ///
    /// # Safety
    /// This function is unsafe because
    /// - It interacts and manipulates memory directly through native pointers
    /// - It interacts, calls, and mutates native lua state through native pointers
    /// 
    /// Return value is OK(Some()) if buffer is patched, Ok(None) if buffer is not patched and
    /// Err() if an error occurred
    pub unsafe fn apply_buffer_patches_internal(
        &self,
        state: *mut lua_State,
        buf_ptr: *const u8,
        size: usize,
        name_ptr: *const u8,
    ) -> Result<Option<String>> {
        // Install native function overrides.
        let binding = Arc::clone(&self.patch_table);
        let patch_table = binding.read().unwrap();
        {
            if !sys::is_module_preloaded(state, "lovely") {
                let closure: lua_CFunction = sys::override_print;
                state.push(closure);
                sys::lua_setfield(state, sys::LUA_GLOBALSINDEX, c"print".as_ptr());

                // Inject Lovely functions into the runtime.
                patch_table.inject_metadata(state)?;

                // Inject mod modules into runtime
                let module_patches: Vec<_> = patch_table
                    .patches
                    .iter()
                    .filter_map(|(x, prio, path)| match x {
                        Patch::Module(patch) => Some((patch, prio, path)),
                        _ => None,
                    })
                    .filter(|(x, _, _)| !x.load_now)
                    .sorted_by_key(|(_, &prio, _)| prio)
                    .map(|(x, _, path)| (x, path))
                    .collect();

                for (patch, path) in module_patches {
                    let patch: &ModulePatch = patch;
                    let _ = unsafe { patch.apply("", state, path) };
                }
            }
        }
        let name = match CStr::from_ptr(name_ptr as _).to_str() {
            Ok(x) => x,
            Err(e) => {
                // There's practically 0 use-case for patching a target with a bad chunk name,
                // so pump a warning to the console and recall.
                warn!("The chunk name at {name_ptr:?} contains invalid UTF-8, skipping: {e}");
                return Ok(None);
            }
        };

        // Stop here if no valid patch exists for this target.
        if !patch_table.needs_patching(name) && !self.dump_all {
            return Ok(None);
        }

        // Prepare buffer for patching
        // Convert the buffer from [u8] to utf8 str.
        let buf = slice::from_raw_parts(buf_ptr, size);
        let buf_str = str::from_utf8(buf).with_context(|| {
            format!("The byte buffer '{buf:?}' for target {name} contains invalid UTF-8")
        })?;

        let regex = Regex::new(r#"=\[(\w+)(?: (\S+))? "([^"]+)"\]"#).unwrap();
        let pretty_name = if let Some(capture) = regex.captures(name) {
            let f1 = capture.get(1).map_or("", |x| x.as_str());
            // Replace . in module names because it means /
            let f2 = capture.get(2).map_or("", |x| x.as_str()).replace(".", "/");
            let f3 = capture.get(3).map_or("", |x| x.as_str());
            format!("{f1}/{f2}/{f3}")
        } else {
            name.replace("@", "")
        };

        // Apply patches onto this buffer.
        write_dump(&self.mod_dir, "game-dump", &pretty_name, &buf_str, None);
        let (patched, debug)  = patch_table.apply_patches(name, buf_str, state)?;

        write_dump(&self.mod_dir, "dump", &pretty_name, &patched, Some(&debug));

        return Ok(Some(patched));
    }
}

// Import PatchTable from the new location
use crate::patch::table::PatchTable;

fn apply_patches(lua: &Lua, (name, buf): (String, String)) -> Result<String> {
    let binding = RUNTIME.get().unwrap().patch_table.read().unwrap();
    if binding.needs_patching(&name) {
        let res = unsafe {
            lua.exec_raw_lua(|rawlua| {
                let state = rawlua.state();
                binding.apply_patches(&name, &buf, state)
            })
        };

        let (patched, _debug) = res?;
        return Ok(patched);
    } else {
        return Ok(buf);
    }
}

impl Target {
    pub fn can_apply(&self, target: &str) -> bool {
        match self {
            Self::Single(str) => str == target,
            Self::Multi(strs) => strs.iter().any(|x| x == target),
        }
    }

    pub fn insert_into(&self, targets: &mut HashSet<String>) {
        match self {
            Self::Single(str) => {
                targets.insert(str.clone());
            }
            Self::Multi(strs) => {
                for target in strs.iter() {
                    targets.insert(target.clone());
                }
            }
        }
    }
}
