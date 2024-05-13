use regex_cursor::{Input, IntoCursor};
use regex_cursor::engines::meta::Regex;
use regex_cursor::regex_automata::util::interpolate;

use ropey::Rope;
use serde::{Serialize, Deserialize};

use super::InsertPosition;

#[derive(Serialize, Deserialize, Debug)]
pub struct RegexPatch {
    pub target: String,

    // The Regex pattern that will be used to both match and create capture groups.
    pub pattern: String,

    // The position to insert the payload relative to the match/capture group.
    pub position: InsertPosition,

    // The target root capture group. The insert position is relative to this group.
    // Defaults to $0 if not set (the entire match).
    pub root_capture: Option<String>,

    // The payload that will be inserted. Regex capture groups can be interpolated
    // by $index.
    pub payload: String,

    // A string or Regex capture to prepend onto the start of each LINE of the payload.
    // This value defaults to an empty string.
    #[serde(default)]
    pub line_prepend: String,
}

impl RegexPatch {
    pub fn apply(&self, target: &str, rope: &mut Rope) -> bool {
        if self.target != target {
            return false;
        }

        let input = Input::new(rope.into_cursor());
        let re = Regex::new(&self.pattern)
            .unwrap_or_else(|e| panic!("Failed to compile Regex pattern '{}': {e:?}", self.pattern));

        let captures = re.captures_iter(input).collect::<Vec<_>>();
        if captures.is_empty() {
            log::info!("Regex query '{}' on target '{target}' did not result in any matches", self.pattern);
            return false;
        }

        // This is our running byte offset. We use this to ensure that byte references
        // within the capture group remain valid even after the rope has been mutated.
        let mut delta = 0_isize;

        for groups in captures {
            // Get the entire captured span (index 0);
            let base = groups.get_group(0).unwrap();
            let base_start = (base.start as isize + delta) as usize;
            let base_end = (base.end as isize + delta) as usize;

            let base_str = rope.get_byte_slice(base_start..base_end).unwrap().to_string();

            // Interpolate capture groups into self.line_prepend, if any capture groups exist within.
            let mut line_prepend = String::new();
            interpolate::string(
                &self.line_prepend,
                |index, dest| {
                    let span = groups.get_group(index).unwrap();
                    let start = (span.start as isize + delta) as usize;
                    let end = (span.end as isize + delta) as usize;

                    let rope_slice = rope.get_byte_slice(start..end).unwrap();

                    dest.push_str(&rope_slice.to_string());
                },
                |name| {
                    let pid = groups.pattern().unwrap();
                    groups.group_info().to_index(pid, name)
                },
                &mut line_prepend
            );

            // Prepend each line of the payload with line_prepend.
            let new_payload = self
                .payload
                .lines()
                .map(|x| if !x.is_empty() {
                    format!("{line_prepend}{x}")
                } else {
                    x.to_string()
                })
                .collect::<Vec<_>>()
                .join("\n");

            // Interpolate capture groups into the payload.
            // We must use this method instead of Captures::interpolate_string because that
            // implementation seems to be broken when working with ropes.
            let mut payload = String::new();
            interpolate::string(
                &new_payload,
                |index, dest| {
                    let span = groups.get_group(index).unwrap();
                    let start = (span.start as isize + delta) as usize;
                    let end = (span.end as isize + delta) as usize;

                    let rope_slice = rope.get_byte_slice(start..end).unwrap();

                    dest.push_str(&rope_slice.to_string());
                },
                |name| {
                    let pid = groups.pattern().unwrap();
                    groups.group_info().to_index(pid, name)
                },
                &mut payload
            );

            // Cleanup and convert the specified root capture to a span.
            let target_group = {
                let group_name = self
                    .root_capture
                    .as_deref()
                    .unwrap_or("0")
                    .replace('$', "");

                if let Ok(idx) = group_name.parse::<usize>() {
                    groups.get_group(idx)
                        .unwrap_or_else(|| 
                            panic!("The capture group at index {idx} could not be found in '{base_str}' with the Regex pattern '{}'", self.pattern))
                } else {
                    groups.get_group_by_name(&group_name)
                        .unwrap_or_else(|| 
                            panic!("The capture group with name '{group_name}' could not be found in '{base_str}' with the Regex pattern '{}'", self.pattern))
                }
            };

            let target_start = (target_group.start as isize + delta) as usize;
            let target_end = (target_group.end as isize + delta) as usize;

            let char_start = rope.byte_to_char(target_start);
            let char_end = rope.byte_to_char(target_end);

            match self.position {
                InsertPosition::Before => {
                    rope.insert(char_start - 1, &payload);
                }
                InsertPosition::After => {
                    rope.insert(char_end, &payload);
                }
                InsertPosition::At => {
                    rope.remove(char_start..char_end);
                    rope.insert(char_start, &payload);
                }
            }

            let new_len = payload.len();
            let old_len = target_group.end - target_group.start;

            delta += new_len as isize - old_len as isize;
        }

        true
    }
}
