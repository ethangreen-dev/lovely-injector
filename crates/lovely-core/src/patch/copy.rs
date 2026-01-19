use crop::Rope;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use super::Target;
use crate::dump::{ByteDebugEntry, ByteRegion, PatchSource, DebugPatchType};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum CopyPosition {
    Prepend,
    Append,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CopyPatch {
    pub position: CopyPosition,
    pub target: Target,
    pub sources: Option<Vec<PathBuf>>,

    pub payload: Option<String>,

    // Currently unused.
    pub name: Option<String>
}

impl CopyPatch {
    /// Apply a copy patch onto the provided buffer and name.
    /// If the name is *not* a valid target of this patch, return false and do not
    /// modify the buffer.
    /// If the name *is* a valid target of this patch, prepend or append the source file(s)'s contents
    /// and return true.
    pub fn apply(&self, target: &str, rope: &mut Rope, path: &Path) -> Option<ByteDebugEntry> {
        if !self.target.can_apply(target) {
            return None;
        } 

        let mut before = 0;
        let mut after = 0;

        // Merge the provided payloads into a single buffer. Each source path should
        // be made absolute by the patch loader.
        if let Some(ref sources) = self.sources {
            for source in sources.iter() {
                let contents = fs::read_to_string(source).unwrap_or_else(|e| {
                    panic!(
                        "Failed to read source file at {source:?} for copy patch from {}: {e:?}",
                        path.display()
                    )
                });

                // Append or prepend the patch's lines onto the provided buffer.
                match self.position {
                    CopyPosition::Prepend => {
                        before += contents.len() + 1;
                        rope.insert(0, "\n");
                        rope.insert(0, &contents);
                    }
                    CopyPosition::Append => {
                        after += contents.len() + 1;
                        rope.insert(rope.byte_len(), "\n");
                        rope.insert(rope.byte_len(), &contents);
                    }
                }
            }
        }

        if let Some(ref payload) = self.payload {
            match self.position {
                CopyPosition::Prepend => {
                    before += payload.len() + 1;
                    rope.insert(0, "\n");
                    rope.insert(0, payload);
                }
                CopyPosition::Append => {
                    after += payload.len() + 1;
                    rope.insert(rope.byte_len(), "\n");
                    rope.insert(rope.byte_len(), payload);
                }
            }
        }

       let mut byte_regions = Vec::new();
       if before != 0 {
           byte_regions.push(ByteRegion { start: 0, end: before, delta: before as isize });
       }
       if after != 0 {
           let len = rope.byte_len();
           byte_regions.push(ByteRegion { start: len - after, end: len, delta: after as isize });
       }
        Some(ByteDebugEntry {
            patch_source: PatchSource {
                file: path.display().to_string(),
                pattern: None,
                patch_type: DebugPatchType::Copy,
            },
            regions: byte_regions,
            warnings: None,
        })
    }
}
