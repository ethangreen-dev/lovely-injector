#![allow(non_upper_case_globals)]

use core::slice;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs};

use log::*;

use crop::Rope;
use getargs::{Arg, Options};
use itertools::Itertools;
use patch::{Patch, PatchFile, Priority};
use regex_lite::Regex;
use sha2::{Digest, Sha256};
use sys::{LuaLib, LuaState, LUA};
use std::sync::Arc;
use std::sync::Mutex;

pub mod chunk_vec_cursor;
pub mod log;
pub mod patch;
pub mod sys;

pub const LOVELY_VERSION: &str = env!("CARGO_PKG_VERSION");

type LoadBuffer =
    dyn Fn(*mut LuaState, *const u8, usize, *const u8, *const u8) -> u32 + Send + Sync + 'static;

pub struct Lovely {
    pub mod_dir: PathBuf,
    pub is_vanilla: bool,
    loadbuffer: &'static LoadBuffer,
    patch_table: PatchTable,
    dump_all: bool,
    // Previously seen *LuaState pointers.
    // Note: can have false negatives. A new LuaState that happens to land in the
    // same memory location as another one won't be detected. We currently ignore this.
    seen_states: Arc<Mutex<HashSet<usize>>>,
}

pub struct LovelyConfig {
    pub dump_all: bool,
    pub vanilla: bool,
    pub mod_dir: Option<PathBuf>,
}

impl Lovely {
    /// Initialize the Lovely patch runtime.
    pub fn init(loadbuffer: &'static LoadBuffer, lualib: LuaLib, config: LovelyConfig) -> Self {
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
            config.mod_dir.unwrap_or_else(|| dirs::config_dir().unwrap().join(game_name).join("Mods"))
        };

        let log_dir = mod_dir.join("lovely").join("log");

        log::init(&log_dir).unwrap_or_else(|e| panic!("Failed to initialize logger: {e:?}"));

        info!("Lovely {LOVELY_VERSION}");

        // Stop here if we're running in vanilla mode.
        if config.vanilla {
            info!("Running in vanilla mode");

            return Lovely {
                mod_dir,
                is_vanilla: config.vanilla,
                loadbuffer,
                patch_table: Default::default(),
                dump_all: config.dump_all,
                seen_states: Arc::new(Mutex::new(HashSet::new())),
            };
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
        let patch_table = PatchTable::load(&mod_dir).with_loadbuffer(loadbuffer);

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

        Lovely {
            mod_dir,
            is_vanilla: config.vanilla,
            loadbuffer,
            patch_table,
            dump_all: config.dump_all,
            seen_states: Arc::new(Mutex::new(HashSet::new())),
        }
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
        {
            let states_mutex = Arc::clone(&self.seen_states);
            let mut states = states_mutex.lock().unwrap();
            if !states.contains(&(state as usize)) {
                states.insert(state as usize);
                let closure = sys::override_print;
                sys::lua_pushcclosure(state, closure, 0);
                sys::lua_setfield(state, sys::LUA_GLOBALSINDEX, c"print".as_ptr());

                // Inject Lovely functions into the runtime.
                self.patch_table.inject_metadata(state);

                // Inject mod modules into runtime
                let module_patches = self
                    .patch_table
                    .patches
                    .iter()
                    .filter_map(|(x, prio, path)| match x {
                        Patch::Module(patch) => Some((patch, prio, path)),
                        _ => None,
                    })
                    .filter(|(x, _, _)| !x.load_now)
                    .sorted_by_key(|(_, &prio, _)| prio)
                    .map(|(x, _, path)| (x, path));

                let loadbuffer = self.patch_table.loadbuffer.unwrap();

                for (patch, path) in module_patches {
                    unsafe { patch.apply("", state, path, &loadbuffer) };
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
        if !self.patch_table.needs_patching(name) && !self.dump_all {
            return (self.loadbuffer)(state, buf_ptr, size, name_ptr, mode_ptr);
        }

        // Prepare buffer for patching
        // Convert the buffer from [u8] to utf8 str.
        let buf = slice::from_raw_parts(buf_ptr, size);
        let buf_str = str::from_utf8(buf).unwrap_or_else(|e| {
            panic!("The byte buffer '{buf:?}' for target {name} contains invalid UTF-8: {e:?}")
        });

        // Apply patches onto this buffer.
        let patched = self.patch_table.apply_patches(name, buf_str, state);

        let regex = Regex::new(r#"=\[(\w+)(?: (\w+))? "([^"]+)"\]"#).unwrap();
        let pretty_name = if let Some(capture) = regex.captures(name) {
            let f1 = capture.get(1).map_or("", |x| x.as_str());
            let f2 = capture.get(2).map_or("", |x| x.as_str());
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

    pub fn parse_args(args: &[String]) -> LovelyConfig {
        let mut config = LovelyConfig {
            dump_all: false,
            vanilla: false,
            mod_dir: None,
        };
        
        let mut opts = Options::new(args.iter().skip(1).map(String::as_str));
        while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
            match opt {
                Arg::Long("mod-dir") => {
                    config.mod_dir = opts.value().map(PathBuf::from).ok()
                }
                Arg::Long("vanilla") => config.vanilla = true,
                Arg::Long("--dump-all") => config.dump_all = true,
                _ => (),
            }
        };

        config
    }
}

#[derive(Default)]
pub struct PatchTable {
    mod_dir: PathBuf,
    loadbuffer: Option<&'static LoadBuffer>,
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
            let first = first
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase();
            let second = second
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase();
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
            .flat_map(|dir| {
                let lovely_toml = dir.join("lovely.toml");
                let lovely_dir = dir.join("lovely");
                let mut toml_files = Vec::new();

                if lovely_toml.is_file() {
                    toml_files.push(lovely_toml);
                }

                if lovely_dir.is_dir() {
                    let mut subfiles = fs::read_dir(&lovely_dir)
                        .unwrap_or_else(|_| {
                            panic!("Failed to read from lovely directory at '{lovely_dir:?}'.")
                        })
                        .filter_map(|x| x.ok())
                        .map(|x| x.path())
                        .filter(|x| x.is_file())
                        .filter(|x| x.extension().is_some_and(|x| x == "toml"))
                        .sorted_by(|a, b| filename_cmp(a, b))
                        .collect_vec();
                    toml_files.append(&mut subfiles);
                }

                toml_files
            })
            .collect_vec();

        let mut targets: HashSet<String> = HashSet::new();
        let mut patches: Vec<(Patch, Priority, PathBuf)> = Vec::new();
        let mut var_table: HashMap<String, String> = HashMap::new();

        // Load n > 0 patch files from the patch directory, collecting them for later processing.
        for patch_file in patch_files {
            let mod_relative_path = patch_file.strip_prefix(mod_dir).unwrap_or_else(|e| {
                panic!(
                    "Base mod directory path {} expected to be a prefix of patch file path {}:\n{e:?}",
                    mod_dir.display(),
                    patch_file.display()
                )
            });

            let patch_dir = patch_file.parent().unwrap();

            // Determine the mod directory from the location of the lovely patch file.
            let mod_dir = if patch_dir.file_name().unwrap() == "lovely" {
                patch_dir.parent().unwrap()
            } else {
                patch_dir
            };

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
                        targets.insert(x.target.clone());
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
                        targets.insert(x.target.clone());
                    }
                    Patch::Regex(x) => {
                        targets.insert(x.target.clone());
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

        PatchTable {
            mod_dir: mod_dir.to_path_buf(),
            loadbuffer: None,
            targets,
            vars: var_table,
            // args: HashMap::new(),
            patches,
        }
    }

    /// Set an override for lual_loadbuffer.
    pub fn with_loadbuffer(self, loadbuffer: &'static LoadBuffer) -> Self {
        PatchTable {
            loadbuffer: Some(loadbuffer),
            ..self
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
        let table = vec![
            ("mod_dir", self.mod_dir.to_str().unwrap().replace('\\', "/")),
            ("version", env!("CARGO_PKG_VERSION").to_string()),
        ];

        let mut code = include_str!("../lovely.lua").to_string();
        for (field, value) in table {
            let field = format!("lovely_template:{field}");
            code = code.replace(&field, &value);
        }

        sys::load_module(state, "lovely", &code, self.loadbuffer.as_ref().unwrap())
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
            .chain(self
                .patches
                .iter()
                .filter(|(patch, _, _)| matches!(patch, Patch::Regex(..))))
            .sorted_by_key(|(_, prio, _)| prio)
            .map(|(patch, _, path)| (patch, path))
            .collect_vec();

        // For display + debug use. Incremented every time a patch is applied.
        let mut patch_count = 0;
        let mut rope = Rope::from(buffer);

        // Apply module injection patches.
        let loadbuffer = self.loadbuffer.unwrap();
        for (patch, path) in module_patches {
            let result = unsafe { patch.apply(target, lua_state, path, &loadbuffer) };

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
                _ => unreachable!()
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
        } else {
            info!("Applied {patch_count} patches to '{target}'");
        }

        // Compute the integrity hash of the patched file.
        let mut hasher = Sha256::new();
        hasher.update(patched.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        format!("LOVELY_INTEGRITY = '{hash}'\n\n{patched}")
    }
}
