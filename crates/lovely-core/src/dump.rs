use std::fs;
use std::path::Path;

use serde::Serialize;

// Sidecar debug entry. Written to the dump dir.
#[derive(Serialize, Debug)]
pub struct PatchDebugEntry {
    pub patch_source: PatchSource,
    pub regions: Vec<PatchRegion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

#[derive(Serialize, Debug, Clone)]
pub enum DebugPatchType {
    #[serde(rename = "pattern")]
    Pattern,
    #[serde(rename = "regex")]
    Regex,
    #[serde(rename = "copy")]
    Copy,
}

#[derive(Serialize, Debug, Clone)]
pub struct PatchSource {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub patch_type: DebugPatchType,
}

#[derive(Serialize, Debug)]
pub struct PatchRegion {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone)]
pub struct ByteRegion {
    pub start: usize,
    pub end: usize,
    pub delta: isize,
}

impl ByteRegion {
    /// Adjust this region based on an edit that occurred elsewhere.
    pub fn adjust(&mut self, edit_pos: usize, delta: isize) {
        if edit_pos <= self.start {
            self.start = self.start.saturating_add_signed(delta);
            self.end = self.end.saturating_add_signed(delta);
        } else if edit_pos < self.end {
            self.end = self.end.saturating_add_signed(delta);
        }
    }
}

/// Dirty byte-based debug entry.
#[derive(Debug)]
pub struct ByteDebugEntry {
    pub patch_source: PatchSource,
    pub regions: Vec<ByteRegion>,
    pub warnings: Option<Vec<String>>,
}

impl ByteDebugEntry {
    /// Adjust all regions in this entry based on the edit that occurred.
    pub fn adjust(&mut self, edit_pos: usize, delta: isize) {
        for region in &mut self.regions {
            region.adjust(edit_pos, delta);
        }
    }
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

    /// Convert from byte-based to line-based.
    pub fn from_byte_entries(buffer_name: &str, byte_entries: Vec<ByteDebugEntry>, rope: &crop::Rope) -> Self {
        let entries = byte_entries
            .into_iter()
            .map(|entry| PatchDebugEntry {
                patch_source: entry.patch_source,
                regions: entry
                    .regions
                    .into_iter()
                    .map(|r| PatchRegion {
                        start_line: rope.line_of_byte(r.start) + 1,
                        end_line: rope.line_of_byte(r.end.saturating_sub(1)) + 1,
                    })
                    .collect(),
                warnings: entry.warnings,
            })
            .collect();

        Self {
            buffer_name: buffer_name.to_string(),
            entries,
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
