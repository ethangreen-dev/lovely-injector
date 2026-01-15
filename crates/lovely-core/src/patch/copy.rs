use super::Target;
use crop::Rope;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
    pub fn apply(&self, target: &str, rope: &mut Rope, path: &Path) -> bool {
        if !self.target.can_apply(target) {
            return false;
        }

        // Combine contents and payload into a single iterator. Contents, then payload (if defined).
        let payloads = self
            .contents
            .iter()
            .map(|s| s.as_str())
            .chain(self.payload.as_deref().into_iter());

        for content in payloads {
            match self.position {
                CopyPosition::Prepend => {
                    rope.insert(0, "\n");
                    rope.insert(0, content);
                }
                CopyPosition::Append => {
                    rope.insert(rope.byte_len(), "\n");
                    rope.insert(rope.byte_len(), content);
                }
            }
        }

        true
    }
}
