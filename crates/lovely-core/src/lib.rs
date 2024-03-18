#![allow(non_upper_case_globals)]

use std::collections::{HashMap, HashSet};
use std::{env, fs};
use std::path::{Path, PathBuf};

use log::*;

use getargs::{Arg, Options};
use manifest::{Patch, PatchArgs};
use sha2::{Digest, Sha256};
use sys::LuaState;

use crate::manifest::PatchManifest;

pub mod sys;
pub mod manifest;
pub mod patch;
pub mod hud;
pub mod log;

type LoadBuffer = dyn Fn(*mut LuaState, *const u8, isize, *const u8) -> u32 + Sync + Send;

pub struct PatchTable {
    mod_dir: PathBuf,
    loadbuffer: Option<Box<LoadBuffer>>,
    targets: HashSet<String>,
    patches: Vec<Patch>,
    vanilla: bool,
    vars: HashMap<String, String>,
    args: HashMap<String, String>,
}

impl PatchTable {
    /// Load patches from the provided mod directory. This scans for lovely patch files
    /// within each subdirectory that matches either:
    /// - MOD_DIR/lovely.toml
    /// - MOD_DIR/lovely/*.toml
    pub fn load(mod_dir: &Path) -> PatchTable {
        // Begin by parsing provided command line arguments. We'll parse patch args later.
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        let mut opts = Options::new(args.iter().map(String::as_str));
    
        let mut mod_dir = dirs::config_dir().unwrap().join("Balatro\\Mods");
        let mut vanilla = false;
    
        while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
            match opt {
                Arg::Long("mod-dir") => mod_dir = opts.value().map(PathBuf::from).unwrap_or(mod_dir),
                Arg::Long("vanilla") => vanilla = true,
                _ => (),
            }
        }

        let mod_dirs = fs::read_dir(&mod_dir)
            .unwrap_or_else(|e| panic!("Failed to read from mod directory within {mod_dir:?}:\n{e:?}"))
            .filter_map(|x| x.ok())
            .filter(|x| x.path().is_dir())
            .map(|x| x.path());

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
                        .unwrap_or_else(|_| panic!("Failed to read from lovely directory at '{lovely_dir:?}'."))
                        .filter_map(|x| x.ok())
                        .map(|x| x.path())
                        .filter(|x| x.is_file())
                        .filter(|x| x.extension().unwrap() == "toml")
                        .collect::<Vec<_>>();
                    toml_files.append(&mut subfiles);
                }

                toml_files
            })
            .collect::<Vec<_>>();

        let mut targets: HashSet<String> = HashSet::new();
        let mut patches: Vec<Patch> = Vec::new();
        let mut var_table: HashMap<String, String> = HashMap::new();

        // Load n > 0 patch files from the patch directory, collecting them for later processing.
        for patch_file in patch_files {
            let patch_dir = patch_file.parent().unwrap();
            
            // Determine the mod directory from the location of the lovely patch file.
            let mod_dir = if patch_dir.file_name().unwrap() == "lovely" {
                patch_dir.parent().unwrap()
            } else {
                patch_dir
            };

            let mut patch: PatchManifest = {
                let str = fs::read_to_string(&patch_file)
                    .unwrap_or_else(|e| panic!("Failed to read patch file at {patch_file:?}:\n{e:?}"));

                toml::from_str(&str)
                    .unwrap_or_else(|e| panic!("Failed to parse patch file at {patch_file:?}:\n{e:?}"))
            };

            // For each patch, map relative paths onto absolute paths, rooted within each's mod directory.
            // We also cache patch targets to short-circuit patching for files that don't need it.
            for patch in &mut patch.patches[..] {
                match patch {
                    Patch::Copy(ref mut x) => {
                        x.sources = x.sources.iter_mut().map(|x| mod_dir.join(x)).collect();
                        targets.insert(x.target.clone());
                    }
                    Patch::Module(ref mut x) => {
                        x.source = mod_dir.join(&x.source);
                        targets.insert(x.before.clone());
                    }
                    Patch::Pattern(x) => {
                        targets.insert(x.target.clone());
                    }
                }
            }

            let inner_patches = patch.patches.as_mut(); 
            patches.append(inner_patches);
            var_table.extend(patch.vars);
        }

        PatchTable {
            mod_dir: mod_dir.to_path_buf(),
            loadbuffer: None,
            targets,
            vars: var_table,
            args: HashMap::new(),
            patches,
            vanilla,
        }
    }

    /// Set an override for lual_loadbuffer.
    pub fn with_loadbuffer<F: Fn(*mut LuaState, *const u8, isize, *const u8) -> u32 + Sync + Send + 'static>(self, loadbuffer: F) -> Self {
        PatchTable {
            loadbuffer: Some(Box::new(loadbuffer)),
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
    pub unsafe fn apply_patches(&self, target: &str, buffer: &str, lua_state: *mut LuaState) -> String {
        let target = target.strip_prefix('@').unwrap_or(target);

        let module_patches = self
            .patches
            .iter()
            .filter_map(|x| match x {
                Patch::Module(patch) => Some(patch),
                _ => None,
            })
            .collect::<Vec<_>>();
        let copy_patches = self
            .patches
            .iter()
            .filter_map(|x| match x {
                Patch::Copy(patch) => Some(patch),
                _ => None
            })
            .collect::<Vec<_>>();
        let pattern_patches = self
            .patches
            .iter()
            .filter_map(|x| match x {
                Patch::Pattern(patch) => Some(patch),
                _ => None
            })
            .collect::<Vec<_>>();

        // For display + debug use. Incremented every time a patch is applied.
        let mut patch_count = 0;

        // Apply module injection patches.
        let loadbuffer = self.loadbuffer.as_ref().unwrap();
        for patch in module_patches {
            let result = unsafe {
                patch.apply(target, lua_state, &loadbuffer)
            };

            if result {
                patch_count += 1;
            }
        }

        // Apply copy patches.
        let mut lines = buffer.lines().map(String::from).collect::<Vec<_>>();
        for patch in copy_patches {
            let result = patch.apply(target, &mut lines);
            if result {
                patch_count += 1;
            }
        }

        // Allocate a new buffer. We'll fill this out as we apply line-based patches.
        let mut new_buffer: Vec<String> = Vec::new();
        for line in lines.iter_mut() {
            let mut before_lines: Vec<String> = vec![];
            let mut after_lines: Vec<String> = vec![];
            let mut new_line = line.to_string();

            // Apply pattern patches to each line.
            for patch in &pattern_patches {
                let patched = patch.apply(line);
                new_line = line.to_string();

                // Yes, we are nesting too much here.
                if patched.is_none() {
                    continue;
                }

                let (mut before, mut after) = patched.unwrap();
                before_lines.append(&mut before);
                after_lines.append(&mut after);
            }

            new_buffer.append(&mut before_lines);
            new_buffer.push(new_line);
            new_buffer.append(&mut after_lines);
        }

        // Apply variable interpolation.
        for line in new_buffer.iter_mut() {
            patch::apply_var_interp(line, &self.vars);
        }

        let patched = new_buffer.join("\n");
        info!("[LOVELY] Applied {patch_count} patches to '{target}'");
        
        // Compute the integrity hash of the patched file.
        let mut hasher = Sha256::new();
        hasher.update(patched.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        format!(
            "LOVELY_INTEGRITY = '{hash}'\n\n{patched}"
        )
    }
}
