use std::path::Path;

use crop::Rope;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use wildmatch::WildMatch;

use crate::dump::{ByteDebugEntry, ByteRegion, PatchSource};

use super::{InsertPosition, Target};

#[derive(Serialize, Deserialize, Debug)]
pub struct PatternPatch {
    // The pattern that the line will be matched against. Very simple,
    // supports only `?` (one occurrence of any character) and `*` (any number of any character).
    // Patterns are matched against a left-trimmed version of the line, so whitespace does not
    // need to be considered.
    pub pattern: String,

    // The position to insert the target at. `PatternAt::At` replaces the matched line entirely.
    pub position: InsertPosition,
    pub target: Target,
    // pub payload_files: Option<Vec<String>>,
    pub payload: String,
    pub match_indent: bool,
    // Apply patch at most `times` times, warn if the number of matches differs from `times`.
    pub times: Option<usize>,

    /// We keep this field around for legacy compat. It doesn't do anything (and never has).
    #[serde(default)]
    pub overwrite: bool,

    // Currently unused.
    pub name: Option<String>
}

impl PatternPatch {
    pub fn debug_from_warning_string(&self, path: &Path, warning: String) -> ByteDebugEntry {
        log::warn!("{}", warning);
        ByteDebugEntry {
            patch_source: PatchSource {
                file: path.display().to_string(),
                pattern: self.pattern.clone(),
            },
            regions: Vec::new(),
            warnings: Some(vec![warning.to_string()]),
        }
    }

    /// Apply the pattern patch onto the rope.
    /// Returns `Some(ByteDebugEntry)` if the rope was modified, `None` otherwise.
    pub fn apply(&self, target: &str, rope: &mut Rope, path: &Path) -> Option<ByteDebugEntry> {
        if !self.target.can_apply(target) {
            return None;
        }

        let wm_lines = self
            .pattern
            .lines()
            .map(|x| x.trim())
            .map(WildMatch::new)
            .collect_vec();
        if wm_lines.is_empty() {
            return Some(self.debug_from_warning_string(path, format!(
                "Pattern on target '{target}' for pattern patch from {} has no lines",
                path.display()
            )));
        }
        let wm_lines_len = wm_lines.len();

        let mut line_index = 0usize;
        let rope_lines = rope.raw_lines().map(|x| x.to_string()).collect_vec();
        let mut matches = Vec::new();
        while let Option::Some(rope_window) = rope_lines.get(line_index..line_index + wm_lines_len)
        {
            if rope_window
                .iter()
                .zip(wm_lines.iter())
                .all(|(source, target)| target.matches(source.trim()))
            {
                if self.match_indent {
                    let leading_indent = String::from_utf8(
                        rope_window[0]
                            .bytes()
                            .take_while(|x| *x == b' ' || *x == b'\t')
                            .collect_vec(),
                    )
                    .unwrap();
                    matches.push((line_index, leading_indent));
                } else {
                    matches.push((line_index, String::new()));
                }
                line_index += wm_lines.len();
            } else {
                line_index += 1;
            }
        }

        if matches.is_empty() {
            return Some(self.debug_from_warning_string(path, format!(
                "Pattern '{}' on target '{target}' for pattern patch from {} resulted in no matches",
                self.pattern.escape_debug(),
                path.display(),
            )));
        }
        let mut warnings = Vec::new();
        if let Some(times) = self.times {
            fn warn_pattern_mismatch(
                pattern: &str,
                target: &str,
                found_matches: usize,
                wanted_matches: usize,
                path: &Path,
            ) -> String{
                let warn_msg: String = if pattern.lines().count() > 1 {
                    format!("Pattern '''\n{pattern}''' on target '{target}' for pattern patch from {} resulted in {found_matches} matches, wanted {wanted_matches}", path.display())
                } else {
                    format!("Pattern '{pattern}' on target '{target}' for pattern patch from {} resulted in {found_matches} matches, wanted {wanted_matches}", path.display())
                };
                for line in warn_msg.lines() {
                    log::warn!("{}", line)
                }
                warn_msg
            }
            if matches.len() < times {
                warnings.push(warn_pattern_mismatch(&self.pattern, target, matches.len(), times, path));
            }
            if matches.len() > times {
                warnings.push(warn_pattern_mismatch(&self.pattern, target, matches.len(), times, path));
                log::warn!("Ignoring excess matches");
                warnings.push("Ignoring excess matches".to_string());
                matches.truncate(times);
            }
        }

        // Track the +/- index offset caused by previous line injections.
        let mut line_delta: isize = 0;

        // Collect byte regions during patching.
        let mut byte_regions: Vec<ByteRegion> = Vec::new();

        for (line_idx, indent) in matches {
            let adjusted_line_idx = line_idx.saturating_add_signed(line_delta);
            let start = rope.byte_of_line(adjusted_line_idx);
            let end = rope.byte_of_line(adjusted_line_idx + wm_lines_len);

            let mut payload = self
                .payload
                .split_inclusive('\n')
                .format_with("", |x, f| f(&format_args!("{}{}", indent, x)))
                .to_string();
            if !self.payload.ends_with('\n') {
                payload.push('\n');
            }
            let payload_lines = payload.lines().count() as isize;
            let payload_bytes = payload.len();

            match self.position {
                InsertPosition::Before => {
                    rope.insert(start, &payload);
                    byte_regions.push(ByteRegion { start, end: start + payload_bytes, delta: payload_bytes as isize });
                    line_delta += payload_lines;
                }
                InsertPosition::After => {
                    rope.insert(end, &payload);
                    byte_regions.push(ByteRegion { start: end, end: end + payload_bytes, delta: payload_bytes as isize });
                    line_delta += payload_lines;
                }
                InsertPosition::At => {
                    let removed_bytes = end - start;
                    rope.delete(start..end);
                    rope.insert(start, &payload);
                    byte_regions.push(ByteRegion { start, end: start + payload_bytes, delta: payload_bytes as isize - removed_bytes as isize });
                    line_delta += payload_lines - wm_lines_len as isize;
                }
            };
        }

        Some(ByteDebugEntry {
            patch_source: PatchSource {
                file: path.display().to_string(),
                pattern: self.pattern.clone(),
            },
            regions: byte_regions,
            warnings: if warnings.is_empty() {None} else {Some(warnings)},
        })
    }
}
