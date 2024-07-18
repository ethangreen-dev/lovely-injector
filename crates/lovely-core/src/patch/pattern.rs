use crop::Rope;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use wildmatch::WildMatch;

use super::InsertPosition;

#[derive(Serialize, Deserialize, Debug)]
pub struct PatternPatch {
    // The pattern that the line will be matched against. Very simple,
    // supports only `?` (one occurrence of any character) and `*` (any number of any character).
    // Patterns are matched against a left-trimmed version of the line, so whitespace does not
    // need to be considered.
    pub pattern: String,

    // The position to insert the target at. `PatternAt::At` replaces the matched line entirely.
    pub position: InsertPosition,
    pub target: String,
    // pub payload_files: Option<Vec<String>>,
    pub payload: String,
    pub match_indent: bool,
    // Apply patch at most `times` times, warn if the number of matches differs from `times`.
    pub times: Option<usize>,

    /// We keep this field around for legacy compat. It doesn't do anything (and never has).
    #[serde(default)]
    pub overwrite: bool,
}

impl PatternPatch {
    /// Apply the pattern patch onto the rope.
    /// The return value will be `true` if the rope was modified.
    pub fn apply(&self, target: &str, rope: &mut Rope) -> bool {
        if self.target != target {
            return false;
        }

        let wm = WildMatch::new(&self.pattern);
        let mut matches = rope
            .raw_lines()
            .enumerate()
            .map(|(i, line)| (i, line.to_string()))
            .filter(|(_, line)| wm.matches(line.trim()))
            .collect::<Vec<(_, _)>>();

        if matches.is_empty() {
            log::warn!(
                "Pattern '{}' on target '{target}' resulted in no matches",
                self.pattern
            );
            return false;
        }
        if let Some(times) = self.times {
            if matches.len() < times {
                log::warn!(
                    "Pattern '{}' on target '{target}' resulted in {} matches, wanted {}",
                    self.pattern,
                    matches.len(),
                    times
                );
            }
            if matches.len() > times {
                log::warn!(
                    "Pattern '{}' on target '{target}' resulted in {} matches, wanted {}",
                    self.pattern,
                    matches.len(),
                    times
                );
                log::warn!("Ignoring excess matches");
                matches.truncate(times);
            }
        }

        // Track the +/- index offset caused by previous line injections.
        let mut line_delta = 0;

        for (line_idx, line) in matches {
            let start = rope.byte_of_line(line_idx + line_delta);
            let end = start + line.len();
            let payload_lines = self.payload.lines().count();

            let indent = if self.match_indent {
                line.chars()
                    .take_while(|x| *x == ' ' || *x == '\t')
                    .collect::<String>()
            } else {
                String::new()
            };
            let mut payload = String::new();
            if !self.payload.starts_with('\n') {
                payload.push('\n');
            }
            payload.push_str(
                &self
                    .payload
                    .split_inclusive('\n')
                    .format_with("", |x, f| f(&format_args!("{}{}", indent, x)))
                    .to_string(),
            );
            if !self.payload.ends_with('\n') {
                payload.push('\n');
            }

            match self.position {
                InsertPosition::Before => {
                    line_delta += payload_lines;
                    rope.insert(start, &payload);
                }
                InsertPosition::After => {
                    line_delta += payload_lines;
                    rope.insert(end, &payload);
                }
                InsertPosition::At => {
                    line_delta += payload_lines - 1;
                    rope.delete(start..end);
                    rope.insert(start, &payload);
                }
            };
        }

        true
    }
}
