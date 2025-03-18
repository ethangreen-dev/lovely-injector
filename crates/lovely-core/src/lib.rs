#![allow(non_upper_case_globals)]

use core::slice;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Instant;
use std::{env, fs};

use log::*;

use crop::Rope;
use getargs::{Arg, Options};
use itertools::Itertools;
use patch::{Patch, PatchFile, Priority};
use regex_lite::Regex;
use sha2::{Digest, Sha256};
use sys::LuaState;

pub mod chunk_vec_cursor;
pub mod log;
pub mod patch;
pub mod sys;

pub const LOVELY_VERSION: &str = env!("CARGO_PKG_VERSION");

type LoadBuffer =
    dyn Fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32 + Send + Sync + 'static;

pub struct LovelyDumpInfo {
    pub dump_dir: PathBuf,
    pub dump_all: bool,
}

pub struct LovelyConfig {
    pub mod_dir: Option<PathBuf>,
    pub log_dir: Option<PathBuf>,

    pub dump: Option<LovelyDumpInfo>,

    pub is_vanilla: bool,
}

pub struct Lovely {
    pub mod_dir: PathBuf,
    pub is_vanilla: bool,
    pub dump: Option<LovelyDumpInfo>,
    loadbuffer: &'static LoadBuffer,
    patch_table: PatchTable,
    rt_init: Once,
}

impl Default for LovelyConfig {
    fn default() -> Self {
        return LovelyConfig {
            mod_dir: None,
            log_dir: None,
            dump: None,
            is_vanilla: false,
        };
    }
}

impl LovelyConfig {
    pub fn init_from_environment() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect_vec();
        let mut opts = Options::new(args.iter().map(String::as_str));
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

        let mut mod_dir: PathBuf = dirs::config_dir().expect("Platform doesn't have a config directory.").join(&game_name).join("Mods");

        let log_dir = mod_dir.join("lovely").join("log");

        log::init(&log_dir).unwrap_or_else(|e| panic!("Failed to initialize logger: {e:?}"));

        let mut is_vanilla = false;
        let mut do_dump = true;
        let mut dump_all: bool = false;

        while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
            match opt {
                Arg::Long("mod-dir") => {
                    mod_dir = opts.value().map(PathBuf::from).unwrap_or(mod_dir)
                }
                Arg::Long("vanilla") => is_vanilla = true,
                Arg::Long("no-dump") => do_dump = false,
                Arg::Long("dump-all") => {
                    dump_all = true;
                    do_dump = true
                }
                _ => (),
            }
        }

        let mut dump: Option<LovelyDumpInfo> = None;

        if do_dump {
            dump = Some(LovelyDumpInfo {
                dump_dir: mod_dir.join("lovely").join("dump"),
                dump_all,
            });
        }

        return LovelyConfig {
            mod_dir: Some(mod_dir),
            log_dir: Some(log_dir),
            dump,
            is_vanilla,
        };
    }
}

impl Lovely {
    /// Initialize the Lovely patch runtime.
    pub fn init(loadbuffer: &'static LoadBuffer, config: LovelyConfig) -> Self {
        let start = Instant::now();

        let mod_dir = config
            .mod_dir
            .expect("Config must include a mod directory.");

        if let Some(log_dir) = config.log_dir {
            log::init(&log_dir).unwrap_or_else(|e| panic!("Failed to initialize logger: {e:?}"));
            info!("Writing logs to {log_dir:?}");
        }

        info!("Lovely {LOVELY_VERSION}");

        // Stop here if we're running in vanilla mode.
        if config.is_vanilla {
            info!("Running in vanilla mode");

            return Lovely {
                mod_dir,
                is_vanilla: true,
                dump: config.dump,

                loadbuffer,
                patch_table: Default::default(),
                rt_init: Once::new(),
            };
        }

        if !mod_dir.is_dir() {
            info!("Creating mods directory at {mod_dir:?}",);
            fs::create_dir_all(&mod_dir).unwrap();
        }

        info!("Using mod directory at {mod_dir:?}",);
        let patch_table = PatchTable::load(&mod_dir).with_loadbuffer(loadbuffer);

        if let Some(dump) = &config.dump {
            if dump.dump_dir.is_dir() {
                info!(
                    "Cleaning up dumps directory at {dump_dir:?}",
                    dump_dir = dump.dump_dir
                );
                fs::remove_dir_all(&dump.dump_dir).unwrap_or_else(|e| {
                    panic!(
                        "Failed to recursively delete dumps directory at {dump_dir:?}: {e:?}",
                        dump_dir = dump.dump_dir
                    )
                });
            }
        }

        info!(
            "Initialization complete in {}ms",
            start.elapsed().as_millis()
        );

        Lovely {
            mod_dir: mod_dir,
            is_vanilla: false,
            dump: config.dump,
            loadbuffer,
            patch_table,
            rt_init: Once::new(),
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
        size: isize,
        name_ptr: *const u8,
        mode_ptr: *const u8,
    ) -> u32 {
        // Install native function overrides.
        self.rt_init.call_once(|| {
            let closure = sys::override_print as *const c_void;
            sys::lua_pushcclosure(state, closure, 0);
            sys::lua_setfield(state, sys::LUA_GLOBALSINDEX, b"print\0".as_ptr() as _);

            // Inject Lovely functions into the runtime.
            self.patch_table.inject_metadata(state);
        });

        let name = match CStr::from_ptr(name_ptr as _).to_str() {
            Ok(x) => x,
            Err(e) => {
                // There's practically 0 use-case for patching a target with a bad chunk name,
                // so pump a warning to the console and recall.
                warn!("The chunk name at {name_ptr:?} contains invalid UTF-8, skipping: {e}");
                return (self.loadbuffer)(state, buf_ptr, size, name_ptr, mode_ptr);
            }
        };

        info!("Hooking into module {name}!");

        // Stop here if no valid patch exists for this target.
        if !self.patch_table.needs_patching(name)
            && !(self.dump.as_ref().is_some() && self.dump.as_ref().unwrap().dump_all)
        {
            return (self.loadbuffer)(state, buf_ptr, size, name_ptr, mode_ptr);
        }

        // Prepare buffer for patching (Check and remove the last byte if it is a null terminator)
        let last_byte = *buf_ptr.offset(size - 1);
        let actual_size = if last_byte == 0 { size - 1 } else { size };

        // Convert the buffer from cstr ptr, to byte slice, to utf8 str.
        let buf = slice::from_raw_parts(buf_ptr, actual_size as _);
        let buf_str = CString::new(buf)
            .unwrap_or_else(|e| panic!("The byte buffer '{buf:?}' for target {name} contains a non-terminating null char: {e:?}"));
        let buf_str = buf_str.to_str().unwrap_or_else(|e| {
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

        if let Some(dump_info) = &self.dump {
            let patch_dump = dump_info.dump_dir.join(&pretty_name);

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
        }

        let raw = CString::new(patched).unwrap();
        let raw_size = raw.as_bytes().len();
        let raw_ptr = raw.into_raw();

        (self.loadbuffer)(state, raw_ptr as _, raw_size as _, name_ptr, mode_ptr)
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
        fn filename_cmp(first: &PathBuf, second: &PathBuf) -> Ordering {
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
            .flat_map(|dir| {
                
                debug!("Searching directory {dir:?}");

                let lovely_toml = dir.join("lovely.toml");
                let lovely_dir = dir.join("lovely");
                let mut toml_files = Vec::new();

                if lovely_toml.is_file() {
                    toml_files.push(lovely_toml);
                }

                if lovely_dir.is_dir() {
                    debug!("Found lovely directory {lovely_dir:?}");

                    let mut subfiles = fs::read_dir(&lovely_dir)
                        .unwrap_or_else(|_| {
                            panic!("Failed to read from lovely directory at '{lovely_dir:?}'.")
                        })
                        .filter_map(|x| x.ok())
                        .map(|x| x.path())
                        .filter(|x| x.is_file())
                        .filter(|x| x.extension().unwrap() == "toml")
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

            debug!("Loading patch file {patch_file:?}");

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
                        targets.insert(x.before.clone());
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

        debug!("Loaded {patchCount} patches from {mod_dir:?}", patchCount = patches.len());

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
        input_buffer: &str,
        lua_state: *mut LuaState,
    ) -> String {
        
        let buffer = input_buffer.replace("\r\n", "\n");

        let target = target.strip_prefix('@').unwrap_or(target);

        let module_patches = self
            .patches
            .iter()
            .filter_map(|(x, prio, path)| match x {
                Patch::Module(patch) => Some((patch, prio, path)),
                _ => None,
            })
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
        let loadbuffer = self.loadbuffer.unwrap();
        for (patch, path) in module_patches {
            let result = unsafe { patch.apply(target, lua_state, &path, &loadbuffer) };

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
