#![allow(non_upper_case_globals)]

use core::slice;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, c_int};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs};
use std::sync::{RwLock, Arc, OnceLock};
use std::panic;

use log::*;

use serde::Deserialize;
use walkdir::WalkDir;

use crop::Rope;
use getargs::{Arg, Options};
use itertools::Itertools;
use patch::{Patch, PatchFile, Priority};
use regex_lite::Regex;

use sys::{LuaLib, LuaState, LuaModule, LUA, LuaFunc, LuaStateTrait, check_lua_string};

use crate::patch::Target;
use crate::sys::{lua_getfield, lua_gettable, lua_gettop, lua_isstring, lua_remove, lua_settop, lua_tolstring, lua_type, Pushable, LUA_TNIL, LUA_TSTRING, LUA_TTABLE};

pub mod chunk_vec_cursor;
pub mod log;
pub mod patch;
pub mod sys;

pub const LOVELY_VERSION: &str = env!("CARGO_PKG_VERSION");

pub static RUNTIME: OnceLock<Lovely> = OnceLock::new();

type LoadBuffer =
dyn Fn(*mut LuaState, *const u8, usize, *const u8, *const u8) -> u32 + Send + Sync + 'static;

unsafe extern "C" fn reload_patches(state: *mut LuaState) -> c_int {
    let result = panic::catch_unwind(|| {
        let lovely = &RUNTIME.get().unwrap();
        let new_table = PatchTable::load(&lovely.mod_dir);
        let binding = Arc::clone(&lovely.patch_table);
        let mut patch_table = binding.write().unwrap();
        *patch_table = new_table;
    });
    let success = result.is_ok();
    if success {
        state.push(true);
        return 1;
    }
    let err = result.unwrap_err();
    let msg = if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    };
    state.push(false);
    state.push(msg);
    2
}

unsafe extern "C" fn getvar(state: *mut LuaState) -> c_int {
    let key = check_lua_string(state, 1);
    let lovely = &RUNTIME.get().unwrap();
    let vars = lovely.lua_vars.read().unwrap();
    let val = vars.get(&key);
    if val.is_some() {
        state.push(val.unwrap());
        return 1;
    }
    0
}

unsafe extern "C" fn setvar(state: *mut LuaState) -> c_int {
    let key = check_lua_string(state, 1);
    let val = check_lua_string(state, 2);
    let lovely = &RUNTIME.get().unwrap();
    let mut vars = lovely.lua_vars.write().unwrap();
    vars.insert(key, val);
    0
}

unsafe extern "C" fn removevar(state: *mut LuaState) -> c_int {
    let key = check_lua_string(state, 1);
    let lovely = &RUNTIME.get().unwrap();
    let mut vars = lovely.lua_vars.write().unwrap();
    let val = vars.remove(&key);
    if val.is_some() {
        state.push(val.unwrap());
        return 1;
    }
    0
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

        LUA.set(lualib).unwrap_or_else(|_| panic!("LUA static var has already been set."));

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
            dirs::config_dir().unwrap().join(game_name).join("Mods")
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
            RUNTIME.set(lovely).unwrap_or_else(|_| panic!("Shit's erroring"));
            return RUNTIME.get().unwrap();
        }

        // Validate that an older Lovely install doesn't already exist within the game directory.
        let exe_path = env::current_exe().unwrap();
        let game_dir = exe_path.parent().unwrap();
        let dwmapi = game_dir.join("dwmapi.dll");

        if dwmapi.is_file() {
            panic!(
                "An old Lovely installation was detected within the game directory. \
                This problem MUST BE FIXED before you can start the game.\n\nTO FIX: Delete the file at {dwmapi:?}"
            );
        }

        info!("Game directory is at {game_dir:?}");
        info!("Writing logs to {log_dir:?}");

        if !mod_dir.is_dir() {
            info!("Creating mods directory at {mod_dir:?}");
            fs::create_dir_all(&mod_dir).unwrap();
        }

        info!("Using mod directory at {mod_dir:?}");
        let patch_table = Arc::new(RwLock::new(PatchTable::load(&mod_dir)));

        let dump_dir = mod_dir.join("lovely").join("dump");
        if dump_dir.is_dir() {
            info!("Cleaning up dumps directory at {dump_dir:?}");
            fs::remove_dir_all(&dump_dir).unwrap_or_else(|e| {
                panic!("Failed to recursively delete dumps directory at {dump_dir:?}: {e:?}")
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
        RUNTIME.set(lovely).unwrap_or_else(|_| panic!("Shit's erroring"));
        RUNTIME.get().unwrap()
    }

    /// Apply patches onto the raw buffer.
    ///
    /// # Safety
    /// This function is unsafe because
    /// - It interacts and manipulates memory directly through native pointers
    /// - It interacts, calls, and mutates native lua state through native pointers
    pub unsafe fn apply_buffer_patches(
        &self,
        state: *mut LuaState,
        buf_ptr: *const u8,
        size: usize,
        name_ptr: *const u8,
        mode_ptr: *const u8,
    ) -> u32 {
        // Install native function overrides.
        let binding = Arc::clone(&self.patch_table);
        let patch_table = binding.read().unwrap();
        {
            if !sys::is_module_preloaded(state, "lovely") {
                let closure: LuaFunc = sys::override_print;
                state.push(closure);
                sys::lua_setfield(state, sys::LUA_GLOBALSINDEX, c"print".as_ptr());

                // Inject Lovely functions into the runtime.
                patch_table.inject_metadata(state);

                // Inject mod modules into runtime
                let module_patches = patch_table
                    .patches
                    .iter()
                    .filter_map(|(x, prio, path)| match x {
                        Patch::Module(patch) => Some((patch, prio, path)),
                        _ => None,
                    })
                .filter(|(x, _, _)| !x.load_now)
                    .sorted_by_key(|(_, &prio, _)| prio)
                    .map(|(x, _, path)| (x, path));

                for (patch, path) in module_patches {
                    unsafe { patch.apply("", state, path) };
                }
            }
        }
        let name = match CStr::from_ptr(name_ptr as _).to_str() {
            Ok(x) => x,
            Err(e) => {
                // There's practically 0 use-case for patching a target with a bad chunk name,
                // so pump a warning to the console and recall.
                warn!("The chunk name at {name_ptr:?} contains invalid UTF-8, skipping: {e}");
                return (self.loadbuffer)(state, buf_ptr, size, name_ptr, mode_ptr);
            }
        };

        // Stop here if no valid patch exists for this target.
        if !patch_table.needs_patching(name) && !self.dump_all {
            return (self.loadbuffer)(state, buf_ptr, size, name_ptr, mode_ptr);
        }

        // Prepare buffer for patching
        // Convert the buffer from [u8] to utf8 str.
        let buf = slice::from_raw_parts(buf_ptr, size);
        let buf_str = str::from_utf8(buf).unwrap_or_else(|e| {
            panic!("The byte buffer '{buf:?}' for target {name} contains invalid UTF-8: {e:?}")
        });

        // Apply patches onto this buffer.
        let patched = patch_table.apply_patches(name, buf_str, state);

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

        let patch_dump = self.mod_dir.join("lovely").join("dump").join(&pretty_name);

        // Check to see if the dump file already exists to fix a weird panic specific to Wine.
        if pretty_name.chars().count() <= 100 && !fs::exists(&patch_dump).unwrap() {
            let dump_parent = patch_dump.parent().unwrap();
            if !dump_parent.is_dir() {
                if let Err(e) = fs::create_dir_all(dump_parent) {
                    error!("Failed to create directory at {dump_parent:?}: {e:?}");
                }
            }

            // Write the patched file to the dump, moving on if an error occurs.
            if let Err(e) = fs::write(&patch_dump, &patched) {
                error!("Failed to write patched buffer to {patch_dump:?}: {e:?}");
            }

            let mut patch_meta = patch_dump;
            patch_meta.set_extension("txt");

            // HACK: Replace the @ symbol on the fly because that's what devs are used to.
            if let Err(e) = fs::write(&patch_meta, name.replacen("@", "", 1)) {
                error!("Failed to write patch metadata to {patch_meta:?}: {e:?}");
            };
        }

        (self.loadbuffer)(state, patched.as_ptr(), patched.len(), name_ptr, mode_ptr)
    }
}

#[derive(Default)]
pub struct PatchTable {
    mod_dir: PathBuf,
    targets: HashSet<String>,
    // Unsorted
    patches: Vec<(Patch, Priority, PathBuf)>,
    vars: HashMap<String, String>,
    // args: HashMap<String, String>,
}

impl PatchTable {
    /// Load patches from the provided mod directory. This scans for lovely patch files
    /// within each subdirectory that matches either:
    /// - MOD_DIR/lovely.toml
    /// - MOD_DIR/lovely/*.toml
    pub fn load(mod_dir: &Path) -> PatchTable {
        fn filename_cmp(first: &Path, second: &Path) -> Ordering {
            let first = first.file_name().unwrap().to_string_lossy().to_lowercase();
            let second = second.file_name().unwrap().to_string_lossy().to_lowercase();
            first.cmp(&second)
        }

        let mod_dirs = fs::read_dir(mod_dir)
            .unwrap_or_else(|e| {
                panic!("Failed to read from mod directory within {mod_dir:?}:\n{e:?}")
            })
        .filter_map(|x| x.ok())
            .filter(|x| x.path().is_dir())
            .map(|x| x.path())
            .filter(|x| {
                let ignore_file = x.join(".lovelyignore");
                let dirname = x
                    .file_name()
                    .unwrap_or_else(|| panic!("Failed to read directory name of {x:?}"))
                    .to_string_lossy();
                if ignore_file.is_file() {
                    info!("Found .lovelyignore in '{dirname}', skipping it.");
                }
                !ignore_file.is_file()
            })
        .sorted_by(|a, b| filename_cmp(a, b));

        let patch_files = mod_dirs
            .map(|dir| {
                let lovely_toml = dir.join("lovely.toml");
                let lovely_dir = dir.join("lovely");
                let mut toml_files = Vec::new();

                if lovely_toml.is_file() {
                    toml_files.push(lovely_toml);
                }

                if lovely_dir.is_dir() {
                    let mut subfiles = WalkDir::new(&lovely_dir)
                        .into_iter()
                        .filter_map(|x| x.ok())
                        .map(|x| x.path().to_path_buf())
                        .filter(|x| x.is_file())
                        .filter(|x| x.extension().is_some_and(|x| x == "toml"))
                        .sorted_by(|a, b| filename_cmp(a, b))
                        .collect_vec();
                    toml_files.append(&mut subfiles);
                }

                (dir, toml_files)
            })
        .collect_vec();

        let mut targets: HashSet<String> = HashSet::new();
        let mut patches: Vec<(Patch, Priority, PathBuf)> = Vec::new();
        let mut var_table: HashMap<String, String> = HashMap::new();

        // Load n > 0 patch files from the patch directory, collecting them for later processing.
        for (ref mod_path, patch_file_vec) in patch_files {
            for patch_file in patch_file_vec {
                let mod_relative_path = patch_file.strip_prefix(mod_dir).unwrap_or_else(|e| {
                    panic!(
                        "Base mod directory path {} expected to be a prefix of patch file path {}:\n{e:?}",
                        mod_dir.display(),
                        patch_file.display()
                    )
                });

                let mod_dir = mod_path;

                let mut patch_file: PatchFile = {
                    let str = fs::read_to_string(&patch_file).unwrap_or_else(|e| {
                        panic!("Failed to read patch file at {patch_file:?}:\n{e:?}")
                    });

                    // HACK: Replace instances of {{lovely_hack:patch_dir}} with mod directory.
                    let clean_mod_dir = &mod_dir.to_string_lossy().replace("\\", "\\\\");
                    let str = str.replace("{{lovely_hack:patch_dir}}", clean_mod_dir);

                    // Handle invalid fields in a non-explosive way.
                    let ignored_key_callback = |key: serde_ignored::Path| {
                        warn!("Unknown key `{key}` found in patch file at {patch_file:?}, ignoring it");
                    };

                    serde_ignored::deserialize(toml::Deserializer::new(&str), ignored_key_callback)
                        .unwrap_or_else(|e| {
                            panic!("Failed to parse patch file at {patch_file:?}:\n{}", e)
                        })
                };

                // For each patch, map relative paths onto absolute paths, rooted within each's mod directory.
                // We also cache patch targets to short-circuit patching for files that don't need it.
                for patch in &mut patch_file.patches[..] {
                    match patch {
                        Patch::Copy(ref mut x) => {
                            x.sources = x.sources.iter_mut().map(|x| mod_dir.join(x)).collect();
                            x.target.insert_into(&mut targets);
                        }
                        Patch::Module(ref mut x) => {
                            x.display_source = x
                                .source
                                .clone()
                                .into_os_string()
                                .into_string()
                                .unwrap_or_default();
                            x.source = mod_dir.join(&x.source);
                            targets.insert(x.before.clone().unwrap_or_default());
                        }
                        Patch::Pattern(x) => {
                            x.target.insert_into(&mut targets);
                        }
                        Patch::Regex(x) => {
                            x.target.insert_into(&mut targets);
                        }
                    }
                }

                let priority = patch_file.manifest.priority;
                patches.extend(
                    patch_file
                    .patches
                    .into_iter()
                    .map(|patch| (patch, priority, mod_relative_path.to_path_buf())),
                );
                // TODO concerned about var name conflicts
                var_table.extend(patch_file.vars);
                }
        }

        PatchTable {
            mod_dir: mod_dir.to_path_buf(),
            targets,
            vars: var_table,
            // args: HashMap::new(),
            patches,
        }
    }

    /// Determine if the provided target file / name requires patching.
    pub fn needs_patching(&self, target: &str) -> bool {
        let target = target.strip_prefix('@').unwrap_or(target);
        self.targets.contains(target)
    }

    /// Inject lovely metadata into the game.
    /// # Safety
    /// Unsafe due to internal unchecked usages of raw lua state.
    pub unsafe fn inject_metadata(&self, state: *mut LuaState) {
        let mod_dir = self.mod_dir.to_str().unwrap().replace('\\', "/");
        let repo = "https://github.com/ethangreen-dev/lovely-injector";

        LuaModule::new()
            .add_var("repo", repo)
            .add_var("version", env!("CARGO_PKG_VERSION"))
            .add_var("mod_dir", mod_dir)
            .add_var("reload_patches", reload_patches as LuaFunc)
            .add_var("apply_patches", apply_patches as LuaFunc)
            .add_var("set_var", setvar as LuaFunc)
            .add_var("get_var", getvar as LuaFunc)
            .add_var("remove_var", removevar as LuaFunc)
            .add_var("create_patch", create_patch as LuaFunc)
            .commit(state, "lovely");
    }

    /// Apply one or more patches onto the target's buffer.
    /// # Safety
    /// Unsafe due to internal unchecked usages of raw lua state.
    pub unsafe fn apply_patches(
        &self,
        target: &str,
        buffer: &str,
        lua_state: *mut LuaState,
    ) -> String {
        let target = target.strip_prefix('@').unwrap_or(target);

        let module_patches = self
            .patches
            .iter()
            .filter_map(|(x, prio, path)| match x {
                Patch::Module(patch) => Some((patch, prio, path)),
                _ => None,
            })
        .filter(|(x, _, _)| x.load_now)
            .sorted_by_key(|(_, &prio, _)| prio)
            .map(|(x, _, path)| (x, path));

        let copy_patches = self
            .patches
            .iter()
            .filter_map(|(x, prio, path)| match x {
                Patch::Copy(patch) => Some((patch, prio, path)),
                _ => None,
            })
        .sorted_by_key(|(_, &prio, _)| prio)
            .map(|(x, _, path)| (x, path));

        let pattern_and_regex = self
            .patches
            .iter()
            .filter(|(patch, _, _)| matches!(patch, Patch::Pattern(..)))
            .chain(
                self.patches
                .iter()
                .filter(|(patch, _, _)| matches!(patch, Patch::Regex(..))),
            )
            .sorted_by_key(|(_, prio, _)| prio)
            .map(|(patch, _, path)| (patch, path))
            .collect_vec();

        // For display + debug use. Incremented every time a patch is applied.
        let mut patch_count = 0;
        let mut rope = Rope::from(buffer);

        // Apply module injection patches.
        for (patch, path) in module_patches {
            let result = unsafe { patch.apply(target, lua_state, path) };

            if result {
                patch_count += 1;
            }
        }

        // Apply copy patches.
        for (patch, path) in copy_patches {
            if patch.apply(target, &mut rope, path) {
                patch_count += 1;
            }
        }

        for (patch, path) in pattern_and_regex {
            let result = match patch {
                Patch::Pattern(x) => x.apply(target, &mut rope, path),
                Patch::Regex(x) => x.apply(target, &mut rope, path),
                _ => unreachable!(),
            };

            if result {
                patch_count += 1;
            }
        }

        let mut patched_lines = {
            let inner = rope.to_string();
            inner.split_inclusive('\n').map(String::from).collect_vec()
        };

        // Apply variable interpolation.
        // TODO I don't think it's necessary to split into lines
        // and convert the rope to Strings? seems overcomplicated
        for line in patched_lines.iter_mut() {
            patch::vars::apply_var_interp(line, &self.vars);
        }

        let patched = patched_lines.concat();

        if patch_count == 1 {
            info!("Applied 1 patch to '{target}'");
        } else if patch_count > 1 { // We definitely don't need to know when 0 patches are applied. Quick speedup
            info!("Applied {patch_count} patches to '{target}'");
        }

        patched
    }
}

unsafe extern "C" fn apply_patches(lua_state: *mut LuaState) -> c_int {
    let buf_name = check_lua_string(lua_state, 1);
    let buf = check_lua_string(lua_state, 2);
    let result = panic::catch_unwind(|| {
        let binding = RUNTIME.get().unwrap().patch_table.try_read().unwrap();
        lua_state.push(binding.apply_patches(&buf_name, &buf, lua_state));
    });
    if result.is_ok() {
        1
    } else {
        lua_state.push(false);
        lua_state.push("Internal lovely error: Failed to acquire the lovely runtime");
        2
    }
}




impl Target {
    pub fn can_apply(&self, target: &str) -> bool {
        match self {
            Self::Single(str) => str == target,
            Self::Multi(strs) => strs.iter().any(|x| x == target)
        }
    }

    pub fn insert_into(&self, targets: &mut HashSet<String>) {
        match self {
            Self::Single(str) => {
                targets.insert(str.clone());
            },
            Self::Multi(strs) => {
                for target in strs.iter() {
                    targets.insert(target.clone());
                }
            }
        }
    }
}

unsafe extern "C" fn create_patch(state: *mut LuaState) -> c_int {

    // Gets [stack+index][field] and if it is a String, returns Some(that_string). Otherwise returns None
    let string_from_field_specific_index = |field: &CStr, index: c_int| -> Option<String> {
        lua_getfield(state, index, field.as_ptr());
        if lua_isstring(state, lua_gettop(state)) != 1 {
            lua_remove(state, lua_gettop(state));
            None
        } else {
            let mut len = 0;
            let cstr = lua_tolstring(state, lua_gettop(state), &mut len);
            let str_buf = slice::from_raw_parts(cstr as *const u8, len);
            lua_remove(state, lua_gettop(state));
            Some(String::from_utf8_lossy(str_buf).to_string())
        }
    };

    //Above, but it's [stack+1][field]. stack+1 is the table passed to create_patch.
    let string_from_field = |field: &CStr| -> Option<String> {
        string_from_field_specific_index(field, 1)
    };

    // Collect desired fields into a HashMap.
    // The boolean value denotes whether the desired field should be naked.
    // Values that are strings (target, payload, pattern) are not naked and are wrapped in '''.
    // Values that are booleans, numbers, etc are naked and don't get wrapped in anything.
    let mut fields: HashMap<&'static str, (String, bool)> = vec![
        (c"type", false),
        (c"priority", true),
        (c"pattern", false),
        (c"position", false),
        (c"target", false),
        (c"payload", false),
        (c"match_indent", true),
        (c"times", true),
        (c"overwrite", true),
        (c"name", false),
        (c"root_capture", true),
        (c"line_prepend", true),
        (c"verbose", false),
        (c"sources", true),
    ]
    .into_iter()
    .filter_map(|x| string_from_field(x.0).map(|val| (x.0.to_str().unwrap(), (val, x.1)))).collect();
    
    if !fields.contains_key("type") {
        "Missing field 'type'".push(state);
        return 1
    }

    let required_fields = match fields.get("type").unwrap().0.as_str() {
        "pattern" | "regex" => {
            vec!["pattern", "position", "target", "payload", "match_indent"]
        },
        "module" => {
            // We would need to call patch.apply from this function, which would be big bad.
            // I tried to support them, but it caused a weird deadlock that I couldn't figure out.
            "Modules aren't supported here. Directly assign to package.preload.".push(state);
            return 1;
        }
        "copy" => {
            // Sources is checked separately, as it is a table.
            vec!["position", "target"]
        }
        _ => {
            "Invalid argument 'type' provided".push(state);
            return 1;
        }
    };

    
    if fields.get("type").unwrap().0.as_str() == "copy" {
        lua_getfield(state, 1, c"sources".as_ptr());
        let is_table = lua_type(state, 2) == LUA_TTABLE;
        if !is_table {
            lua_remove(state, 2);
            "Missing argument or argument type invalid for field 'sources'".push(state);
            return 1
        }

        let mut sources = Vec::new();
        // Don't use lua_next as we aren't interested in keys.
        let mut i = 1;
        loop {
            i.push(state);
            lua_gettable(state, 2);
             if lua_type(state, 3) == LUA_TNIL {
                lua_settop(state, 1);
                break
             }
            if lua_type(state, 3) != LUA_TSTRING {
                lua_settop(state, 1);
                "Found non-string item in sources".push(state);
                return 1;
            }
            let mut len = 0;
            let cstr = lua_tolstring(state, lua_gettop(state), &mut len);
            let str_buf = slice::from_raw_parts(cstr as *const u8, len);
            let as_string = String::from_utf8_lossy(str_buf).to_string();
            lua_remove(state, lua_gettop(state));
            sources.push(as_string);
            i += 1;
        }
        fields.insert("sources", (format!("{sources:?}"), true));
    };

    let fields = fields;
    let patch_type = fields.get("type").unwrap().0.as_str();

    for field in required_fields {
        if !fields.contains_key(field) {
            format!("Missing argument or argument type invalid for field {}: {field}", patch_type).push(state);
            return 1;
        }
    }

    // Translate into toml

    let mut toml = String::new();
    toml.push_str(
        "[manifest]
        version = \"1.0.0\"
        dump_lua = true
        priority = "
    );
    toml.push_str(fields.get("priority").map(|(x, _)| x.as_str()).unwrap_or("0"));

    toml.push_str(format!("
    [[patches]]
    [patches.{patch_type}]").as_str());
    for (field_key, (field_value, is_naked)) in fields {
        String::push(&mut toml, '\n');
        if is_naked {
            toml.push_str(format!("{field_key} = {field_value}").as_str())
        } else {
            toml.push_str(format!("{field_key} = '''{field_value}'''").as_str())
        }
    }
    
    // There won't ever be redundant keys, don't use serde_ignored.
    let pf: Result<PatchFile, toml::de::Error> = PatchFile::deserialize(toml::Deserializer::new(&toml));
    let result_runtime = panic::catch_unwind(|| {
        // We shouldn't ever be blocked, but if we are I don't think it'll be released if we lock. Just panic (to be caught) if this happens.
        RUNTIME.get().unwrap().patch_table.try_write().unwrap()
    });
    if let Ok(mut rt) = result_runtime {
        if let Ok(mut patch_file) = pf {
            let opt_patch = patch_file.patches.first_mut();
            let targets = &mut rt.targets;
            if let Some(patch) = opt_patch {
                match patch {
                    Patch::Copy(x) => {
                        x.target.insert_into(targets);
                    }
                    Patch::Module(_) => (),
                    Patch::Pattern(x) => {
                        x.target.insert_into(targets);
                    }
                    Patch::Regex(x) => {
                        x.target.insert_into(targets);
                    }
                }
            } else {
                format!("Failed to parse generated toml:\n{toml}").push(state);
                return 1;
            }
            let patch_owned = patch_file.patches.drain(0..1).next().unwrap();
            rt.patches.push((patch_owned, patch_file.manifest.priority, Path::new("NO_PATH").to_path_buf()));
        } else {
            format!("Failed to parse generated toml: \n{toml}").push(state);
            return 1
        }
    } else {
        "Failed to acquire lovely runtime".push(state);
        return 1;
    }

    0
}