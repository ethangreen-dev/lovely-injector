use ropey::Rope;
use serde::{Serialize, Deserialize};
use wildmatch::WildMatch;

use super::InsertPosition;

#[derive(Serialize, Deserialize, Debug)]
pub struct PatternPatch {
    // The pattern that the line will be matched against. Very simple,
    // supports only `?` (one occurance of any character) and `*` (any numver of any character).
    // Patterns are matched against a left-trimmed version of the line, so whitespace does not
    // need to be considered.
    pub pattern: String,

    // The position to insert the target at. `PatternAt::At` replaces the matched line entirely.
    pub position: InsertPosition,
    pub target: String,
    pub payload_files: Option<Vec<String>>,
    pub payload: String,
    pub match_indent: bool,
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
        let matches = rope
            .lines()
            .enumerate()
            .map(|(i, line)| (i, line.to_string()))
            .filter(|(_, line)| wm.matches(line.trim()))
            .collect::<Vec<(_, _)>>();

        if matches.is_empty() {
            return false;
        }

        // Track the +/- index offset caused by previous line injections.
        let mut line_delta = 0;

        for (line_idx, line) in matches {
            let start = rope.line_to_char(line_idx + line_delta);
            let end = start + line.chars().count();
            let payload_lines = self.payload.lines().count();

            let indent = if self.match_indent {
                line.chars().take_while(|x| *x == ' ' || *x == '\t').collect::<String>()
            } else {
                String::new()
            };

            let payload = self.payload.split('\n')
                .map(|x| format!("{indent}{x}"))
                .collect::<Vec<_>>()
                .join("\n");

            let newline = if self.payload.ends_with('\n') {
                ""
            } else {
                "\n"
            };

            let new_payload = format!("{payload}{newline}");
            match self.position {
                InsertPosition::Before => { 
                    line_delta += payload_lines;
                    rope.insert(start, &new_payload);
                }
                InsertPosition::After => {
                    line_delta += payload_lines;
                    rope.insert(end, &new_payload);
                }
                InsertPosition::At => {
                    line_delta += payload_lines - 1;
                    rope.remove(start..end);
                    rope.insert(start, &new_payload);
                }
            };
        }

        true
    }
}
