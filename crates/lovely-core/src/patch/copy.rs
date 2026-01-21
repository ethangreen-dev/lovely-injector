use super::Target;
use crop::Rope;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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
    pub name: Option<String>,

    // Buffer contents read at load time. We do this to support zip-based mods that we can't arbitrarily read from.
    #[serde(skip)]
    pub contents: Vec<String>,
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

        // Combine contents and payload into a single iterator. Contents, then payload (if defined).
        let payloads = self
            .contents
            .iter()
            .map(|s| s.as_str())
            .chain(self.payload.as_deref().into_iter());

        for content in payloads {
            match self.position {
                CopyPosition::Prepend => {
                    before += content.len() + 1;
                    rope.insert(0, "\n");
                    rope.insert(0, content);
                }
                CopyPosition::Append => {
                    after += content.len() + 1;
                    rope.insert(rope.byte_len(), "\n");
                    rope.insert(rope.byte_len(), content);
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
