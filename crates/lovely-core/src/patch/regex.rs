use std::path::Path;

use regex_cursor::Input;
use regex_cursor::engines::meta::Regex;
use regex_cursor::regex_automata::util::syntax;
use regex_cursor::regex_automata::util::interpolate;

use itertools::Itertools;
use crop::Rope;
use serde::{Serialize, Deserialize};

use crate::chunk_vec_cursor::IntoCursor;

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

    // Apply patch at most `times` times, warn if the number of matches differs from `times`.
    pub times: Option<usize>,

    // If enabled, whitespace is ignored unless escaped
    #[serde(default)]
    pub verbose: bool,
}

impl RegexPatch {
    pub fn apply(&self, target: &str, rope: &mut Rope, path: &Path) -> bool {
        if self.target != target {
            return false;
        }

        let input = Input::new(rope.into_cursor());
        let re = Regex::builder()
            .syntax(
                 syntax::Config::new()
                    .multi_line(true)
                    .crlf(true)
                    .ignore_whitespace(self.verbose)
            )
            .build(&self.pattern)
            .unwrap_or_else(|e| panic!("Failed to compile Regex '{}' for regex patch from {}: {e:?}", path.display(), self.pattern));

        let mut captures = re.captures_iter(input).collect_vec();
        if captures.is_empty() {
            log::warn!("Regex '{}' on target '{target}' for regex patch from {} resulted in no matches", self.pattern.escape_debug(), path.display());
            return false;
        }
        if let Some(times) = self.times {
            fn warn_regex_mismatch(pattern: &str, target: &str, found_matches: usize, wanted_matches: usize, path: &Path) {
                let warn_msg: String = if pattern.lines().count() > 1 {
                    format!("Regex '''\n{pattern}''' on target '{target}' for regex patch from {} resulted in {found_matches} matches, wanted {wanted_matches}", path.display())
                } else {
                    format!("Regex '{pattern}' on target '{target}' for regex patch from {} resulted in {found_matches} matches, wanted {wanted_matches}", path.display())
                };
                for line in warn_msg.lines() {
                    log::warn!("{}", line)
                }
            }
            if captures.len() < times {
                warn_regex_mismatch(&self.pattern, target, captures.len(), times, path);
            }
            if captures.len() > times {
                warn_regex_mismatch(&self.pattern, target, captures.len(), times, path);
                log::warn!("Ignoring excess matches");
                captures.truncate(times);
            }
        }

        // This is our running byte offset. We use this to ensure that byte references
        // within the capture group remain valid even after the rope has been mutated.
        let mut delta = 0_isize;

        for groups in captures {
            // Get the entire captured span (index 0);
            let base = groups.get_group(0).unwrap();
            let base_start = (base.start as isize + delta) as usize;
            let base_end = (base.end as isize + delta) as usize;

            let base_str = rope.byte_slice(base_start..base_end).to_string();

            // Interpolate capture groups into self.line_prepend, if any capture groups exist within.
            let mut line_prepend = String::new();
            interpolate::string(
                &self.line_prepend,
                |index, dest| {
                    let span = groups.get_group(index).unwrap();
                    let start = (span.start as isize + delta) as usize;
                    let end = (span.end as isize + delta) as usize;

                    let rope_slice = rope.byte_slice(start..end);

                    dest.push_str(&rope_slice.to_string());
                },
                |name| {
                    let pid = groups.pattern().unwrap();
                    groups.group_info().to_index(pid, name)
                },
                &mut line_prepend
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
                            panic!("The capture group at index {idx} could not be found in '{base_str}' with the Regex '{}' for regex patch from {}", self.pattern, path.display()))
                } else {
                    groups.get_group_by_name(&group_name)
                        .unwrap_or_else(|| 
                            panic!("The capture group with name '{group_name}' could not be found in '{base_str}' with the Regex '{}' for regex patch from {}", self.pattern, path.display()))
                }
            };

            let target_start = (target_group.start as isize + delta) as usize;
            let target_end = (target_group.end as isize + delta) as usize;

            let new_payload = std::format!("{}", 
                self
                    .payload
                    .split_inclusive('\n')
                    .format_with("", |x, f| f(&format_args!("{}{}", line_prepend, x)))
            );

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

                    let rope_slice = rope.byte_slice(start..end);

                    dest.push_str(&rope_slice.to_string());
                },
                |name| {
                    let pid = groups.pattern().unwrap();
                    groups.group_info().to_index(pid, name)
                },
                &mut payload
            );

            // If left border of insertion is a wordchar -> non-wordchar 
            // boundary and our patch starts with a wordchar, prepend space so 
            // it doesn't unintentionally concatenate with characters to its 
            // left to create a larger identifier.
            if payload.starts_with(|x: char| x.is_ascii_alphanumeric() || x == '_') {
                let pre_pt = if let InsertPosition::After = self.position {
                    target_end
                } else {
                    target_start
                };
                if pre_pt > 0 {
                    let byte_on_left = rope.byte(pre_pt - 1);
                    if byte_on_left.is_ascii_alphanumeric() || byte_on_left == b'_' {
                        payload.insert(0, ' ');
                    }
                }
            }

            // If right border of insertion is a non-wordchar -> wordchar 
            // boundary and our patch ends with a wordchar, append space so 
            // it doesn't unintentionally concatenate with characters to its 
            // right to create a larger identifier.     
            if payload.ends_with(|x: char| x.is_ascii_alphanumeric() || x == '_') {
                let post_pt = if let InsertPosition::Before = self.position {
                    target_start
                } else {
                    target_end
                };
                if post_pt < rope.byte_len() {
                    let byte_on_right = rope.byte(post_pt);
                    if byte_on_right.is_ascii_alphanumeric() || byte_on_right == b'_' {
                        payload.push(' ');
                    }
                }
            }

            match self.position {
                InsertPosition::Before => {
                    rope.insert(target_start, &payload);
                    let new_len = payload.len();
                    delta += new_len as isize;
                }
                InsertPosition::After => {
                    rope.insert(target_end, &payload);
                    let new_len = payload.len();
                    delta += new_len as isize;
                }
                InsertPosition::At => {
                    rope.delete(target_start..target_end);
                    rope.insert(target_start, &payload);
                    let old_len = target_group.end - target_group.start;
                    let new_len = payload.len();
                    delta -= old_len as isize;
                    delta += new_len as isize;
                }
            }
        }
        true
    }
}
