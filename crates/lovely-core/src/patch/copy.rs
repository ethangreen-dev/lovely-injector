use std::path::PathBuf;
use ropey::Rope;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum CopyPosition {
    Prepend,
    Append,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CopyPatch {
    pub position: CopyPosition,
    pub target: String,
    pub sources: Vec<PathBuf>,
}
impl CopyPatch {
    /// Apply a copy patch onto the provided buffer and name.
    /// If the name is *not* a valid target of this patch, return false and do not
    /// modify the buffer.
    /// If the name *is* a valid target of this patch, prepend or append the source file(s)'s contents
    /// and return true.
    pub fn apply(&self, target: &str, rope: &mut Rope) -> bool {
        if self.target != target {
            return false;
        }

        // Merge the provided payloads into a single buffer. Each source path should
        // be made absolute by the patch loader.
        for source in self.sources.iter() {
            let contents = super::get_cached_file(source).unwrap_or(super::set_cached_file(source));

            // Append or prepend the patch's lines onto the provided buffer.
            match self.position {
                CopyPosition::Prepend => { 
                    rope.insert(0, &contents);

                    let last_char = rope.byte_to_char(contents.len());
                    rope.insert_char(last_char, '\n');
                },
                CopyPosition::Append => {
                    let last_char = rope.len_chars();
                    rope.insert_char(last_char, '\n');
                    rope.insert(last_char + 1, &contents);
                }
            }
        }

        true
    }
}
