use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs};
use anyhow::{Result, Context, bail};

use log::*;

use walkdir::WalkDir;
use crop::Rope;
use itertools::Itertools;
use crate::patch::{Patch, PatchFile, Priority};
use crate::sys::{LuaState, LuaTable, LuaFunc, preload_module};
use crate::patch::vars;

/// Structure to manage patch tables for Lovely runtime
pub struct PatchTable {
    pub mod_dir: PathBuf,
    pub targets: HashSet<String>,
    // Unsorted
    pub patches: Vec<(Patch, Priority, PathBuf)>,
    pub vars: HashMap<String, String>,
    // args: HashMap<String, String>,
}

impl PatchTable {
    /// Load patches from the provided mod directory. This scans for lovely patch files
    /// within each subdirectory that matches either:
    /// - MOD_DIR/lovely.toml
    /// - MOD_DIR/lovely/*.toml
    pub fn load(mod_dir: &Path) -> Result<PatchTable> {
        fn filename_cmp(first: &Path, second: &Path) -> Ordering {
            let first = first.file_name().unwrap().to_string_lossy().to_lowercase();
            let second = second.file_name().unwrap().to_string_lossy().to_lowercase();
            first.cmp(&second)
        }

        let blacklist_file = mod_dir.join("lovely").join("blacklist.txt");

        let mut blacklist: HashSet<String> = HashSet::new();
        if fs::exists(&blacklist_file)? {
            let text = fs::read_to_string(blacklist_file).context("Could not read blacklist")?;

            blacklist.extend(
                text.lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|line| line.to_string())
            );
        } else {
            info!("No blacklist.txt in Mods/lovely.");
        }

        let mod_dirs = fs::read_dir(mod_dir)
            .with_context(|| {
                format!("Failed to read from mod directory within {mod_dir:?}")
            })?
        .filter_map(|x| x.ok())
            .filter(|x| x.path().is_dir())
            .filter(|x| {
                let cname = x.file_name();
                let name = cname.to_str().unwrap();
                let blacklisted = blacklist.contains(name);
                if blacklisted {
                    info!("'{name}' was found in blacklist, skipping it.");
                }
                !blacklisted
            })
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
                let mod_relative_path = patch_file.strip_prefix(mod_dir).with_context(|| {
                    format!(
                        "Base mod directory path {} expected to be a prefix of patch file path {}",
                        mod_dir.display(),
                        patch_file.display()
                    )
                })?;

                let mod_dir = mod_path;

                let mut patch_file: PatchFile = {
                    let str = fs::read_to_string(&patch_file).with_context(|| {
                        format!("Failed to read patch file at {patch_file:?}")
                    })?;

                    // HACK: Replace instances of {{lovely_hack:patch_dir}} with mod directory.
                    let clean_mod_dir = &mod_dir.to_string_lossy().replace("\\", "\\\\");
                    let str = str.replace("{{lovely_hack:patch_dir}}", clean_mod_dir);

                    // Handle invalid fields in a non-explosive way.
                    let ignored_key_callback = |key: serde_ignored::Path| {
                        warn!("Unknown key `{key}` found in patch file at {patch_file:?}, ignoring it");
                    };

                    serde_ignored::deserialize(toml::Deserializer::new(&str), ignored_key_callback)
                        .with_context(|| {
                            format!("Failed to parse patch file at {patch_file:?}")
                        })?
                };

                // For each patch, map relative paths onto absolute paths, rooted within each's mod directory.
                // We also cache patch targets to short-circuit patching for files that don't need it.
                // For module patches, we verify that they are valid.
                for patch in &mut patch_file.patches[..] {
                    match patch {
                        Patch::Copy(ref mut x) => {
                            if let Some(ref mut sources) = x.sources {
                                x.sources = Some(sources.iter_mut().map(|x| mod_dir.join(x)).collect::<Vec<_>>())
                            }

                            if x.sources.is_none() && x.payload.is_none() {
                                let name = match &x.name {
                                    None => "".to_string(),
                                    Some(name ) => format!(" \"{name}\"")
                                };
                                bail!("Error at patch file {}:\nCopy{name} does not have a \"payload\" or \"sources\" parameter set.", mod_relative_path.display())
                            }

                            x.target.insert_into(&mut targets);
                        }
                        Patch::Module(ref mut x) => {
                            if x.load_now && x.before.is_none() {
                                bail!("Error at patch file {}:\nModule \"{}\" has \"load_now\" set to true, but does not have required parameter \"before\" set", mod_relative_path.display(), x.name);
                            }

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
                // TODO: concerned about var name conflicts
                var_table.extend(patch_file.vars);
            }
        }

        Ok(PatchTable {
            mod_dir: mod_dir.to_path_buf(),
            targets,
            vars: var_table,
            // args: HashMap::new(),
            patches,
        })
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

        // Import the functions needed for injection
        use crate::{reload_patches, apply_patches, setvar, getvar, removevar, get_log_path};

        preload_module(state, "lovely", LuaTable::new()
            .add_var("repo", repo)
            .add_var("version", env!("CARGO_PKG_VERSION"))
            .add_var("mod_dir", mod_dir)
            .add_var("reload_patches", reload_patches as LuaFunc)
            .add_var("apply_patches", apply_patches as LuaFunc)
            .add_var("set_var", setvar as LuaFunc)
            .add_var("get_var", getvar as LuaFunc)
            .add_var("remove_var", removevar as LuaFunc)
            .add_var("log_path", get_log_path().unwrap())
        );
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
            let result = patch.apply(target, &mut rope, path);
            if result {
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
            vars::apply_var_interp(line, &self.vars);
        }

        let patched = patched_lines.concat();

        if patch_count == 1 {
            info!("Applied 1 patch to '{target}'");
        } else {
            info!("Applied {patch_count} patches to '{target}'");
        }

        patched
    }
}

impl Default for PatchTable {
    fn default() -> Self {
        Self {
            mod_dir: PathBuf::new(),
            targets: HashSet::new(),
            patches: Vec::new(),
            vars: HashMap::new(),
        }
    }
}