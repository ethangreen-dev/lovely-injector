use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use wildmatch::WildMatch;

use crate::dump::{ByteDebugEntry, PatchDebug};
use crate::patch::{loader, vars};
use crate::patch::{Patch, Priority};
use crate::sys::{preload_module, LuaFunc, LuaState, LuaTable};
use crop::Rope;
use itertools::Itertools;
use log::*;

/// Structure to manage patch tables for Lovely runtime
pub struct PatchTable {
    pub mod_dir: PathBuf,
    pub targets: HashSet<String>,
    pub glob_targets: Vec<WildMatch>,
    // Unsorted
    pub patches: Vec<(Patch, Priority, PathBuf)>,
    pub vars: HashMap<String, String>,
    // args: HashMap<String, String>,
}

impl Default for PatchTable {
    fn default() -> Self {
        Self {
            mod_dir: PathBuf::new(),
            targets: HashSet::new(),
            glob_targets: Vec::new(),
            patches: Vec::new(),
            vars: HashMap::new(),
        }
    }
}

impl PatchTable {
    /// Load patches from the provided mod directory.
    pub fn load(mod_dir: &Path) -> Result<PatchTable> {
        let raw_patches = loader::load_patches_new(mod_dir)?;
        let (patches, targets, vars, glob_targets) = loader::process_patches(raw_patches);

        Ok(PatchTable {
            mod_dir: mod_dir.to_path_buf(),
            targets,
            glob_targets,
            patches,
            vars,
        })
    }

    /// Determine if the provided target file / name requires patching.
    pub fn needs_patching(&self, target: &str) -> bool {
        let target = target.strip_prefix('@').unwrap_or(target);
        if self.targets.contains(target) {
            true
        } else {
            self.glob_targets.iter().any(|x| x.matches(target))
        }
    }

    /// Inject lovely metadata into the game.
    /// # Safety
    /// Unsafe due to internal unchecked usages of raw lua state.
    pub unsafe fn inject_metadata(&self, state: *mut LuaState) {
        let mod_dir = self.mod_dir.to_str().unwrap().replace('\\', "/");
        let repo = "https://github.com/ethangreen-dev/lovely-injector";

        // Import the functions needed for injection
        use crate::{apply_patches, get_log_path, getvar, reload_patches, removevar, setvar};

        preload_module(
            state,
            "lovely",
            LuaTable::new()
                .add_var("repo", repo)
                .add_var("version", env!("CARGO_PKG_VERSION"))
                .add_var("mod_dir", mod_dir)
                .add_var("reload_patches", reload_patches as LuaFunc)
                .add_var("apply_patches", apply_patches as LuaFunc)
                .add_var("set_var", setvar as LuaFunc)
                .add_var("get_var", getvar as LuaFunc)
                .add_var("remove_var", removevar as LuaFunc)
                .add_var("log_path", get_log_path().unwrap()),
        );
    }

    /// Apply one or more patches onto the target's buffer.
    /// Returns the patched content and debug info.
    /// # Safety
    /// Unsafe due to internal unchecked usages of raw lua state.
    pub unsafe fn apply_patches(
        &self,
        target: &str,
        buffer: &str,
        lua_state: *mut LuaState,
    ) -> Result<(String, PatchDebug), String> {
        // Buffer Content, Debug info, Error message
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

        // Collect byte-based debug entries, adjust after each patch.
        let mut byte_entries: Vec<ByteDebugEntry> = Vec::new();

        // Apply module injection patches.
        for (patch, path) in module_patches {
            let result = unsafe { patch.apply(target, lua_state, path) };

            if result? {
                patch_count += 1;
            }
        }

        // Apply copy patches.
        for (patch, path) in copy_patches {
            let result = patch.apply(target, &mut rope, path);
            if let Some(entry) = result {
                // Adjust all previous entries based on this patch's edits.
                for region in &entry.regions {
                    for prev_entry in &mut byte_entries {
                        prev_entry.adjust(region.start, region.delta);
                    }
                }

                patch_count += 1;
                byte_entries.push(entry);
            }
        }

        for (patch, path) in pattern_and_regex {
            let result = match patch {
                Patch::Pattern(x) => x.apply(target, &mut rope, path),
                Patch::Regex(x) => x.apply(target, &mut rope, path),
                _ => unreachable!(),
            };

            if let Some(entry) = result {
                // Adjust all previous entries based on this patch's edits.
                for region in &entry.regions {
                    for prev_entry in &mut byte_entries {
                        prev_entry.adjust(region.start, region.delta);
                    }
                }

                if !entry.regions.is_empty() {
                    patch_count += 1
                };
                byte_entries.push(entry);
            }
        }

        // Convert byte entries to line-based debug info using final rope state.
        let debug = PatchDebug::from_byte_entries(target, byte_entries, &rope);

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
        } else if patch_count >= 2 {
            info!("Applied {patch_count} patches to '{target}'");
        }

        Ok((patched, debug))
    }
}
