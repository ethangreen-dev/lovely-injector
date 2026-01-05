use std::fs;
use std::path::Path;

use serde::Serialize;

// Sidecar debug entry. Written to the dump dir.
#[derive(Serialize, Debug)]
pub struct PatchDebugEntry {
    pub patch_source: PatchSource,
    pub regions: Vec<PatchRegion>,
}

#[derive(Serialize, Debug)]
pub struct PatchSource {
    pub file: String,
    pub pattern: String,
}

#[derive(Serialize, Debug)]
pub struct PatchRegion {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Serialize, Debug)]
pub struct PatchDebug {
    pub buffer_name: String,
    pub entries: Vec<PatchDebugEntry>,
}

impl PatchDebug {
    pub fn new(buffer_name: &str) -> Self {
        Self {
            buffer_name: buffer_name.to_string(),
            entries: Vec::new(),
        }
    }
}

/// Dump the buffer and its sidecar.
pub fn write_dump(
    mod_dir: &Path,
    dir_name: &str,
    name: &str,
    buffer: &str,
    debug: &PatchDebug,
) {
    if name.chars().count() > 100 {
        return;
    }

    let dump_path = mod_dir.join("lovely").join(dir_name).join(name);
    if fs::exists(&dump_path).unwrap_or(false) {
        return;
    }

    if let Some(parent) = dump_path.parent() {
        if !parent.is_dir() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::error!("Failed to create directory at {parent:?}: {e:?}");
                return;
            }
        }
    }

    if let Err(e) = fs::write(&dump_path, buffer) {
        log::error!("Failed to write dump to {dump_path:?}: {e:?}");
        return;
    }

    let mut json_path = dump_path;
    json_path.add_extension("json");

    if fs::exists(&json_path).unwrap_or(false) {
        return;
    }

    match serde_json::to_string_pretty(debug) {
        Ok(json) => {
            if let Err(e) = fs::write(&json_path, json) {
                log::error!("Failed to write debug JSON to {json_path:?}: {e:?}");
            }
        }
        Err(e) => {
            log::error!("Failed to serialize debug info: {e:?}");
        }
    }
}
