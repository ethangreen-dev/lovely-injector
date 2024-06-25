use std::borrow::Cow;
use std::{fs, sync::Mutex};
use std::path::PathBuf;
use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex_lite::{Regex, Captures};
use serde::{Serialize, Deserialize};

pub use patch_types::*;

mod patch_types;
pub mod copy;
pub mod module;
pub mod pattern;
pub mod regex;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum InsertPosition {
    At,
    Before,
    After,
}
// This contains the cached contents of one or more source files. We use to reduce 
// runtime cost as we're now possibly reading from files EVERY line and not all at once.
static FILE_CACHE: Lazy<Mutex<HashMap<PathBuf, Cow<String>>>> = Lazy::new(Default::default);

pub(crate) fn get_cached_file(path: &PathBuf) -> Option<Cow<String>> {
    FILE_CACHE.lock().unwrap().get(path).cloned()
}

pub(crate) fn set_cached_file(path: &PathBuf) -> Cow<String> {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read patch file at {path:?}: {e:?}"));
    let mut locked = FILE_CACHE.lock().unwrap();

    locked.insert(path.clone(), Cow::Owned(contents));
    locked.get(path).cloned().unwrap()
}

/// Apply valid var interpolations to the provided line.
/// Interpolation targets are of form {{lovely:VAR_NAME}}.
pub fn apply_var_interp(line: &mut String, vars: &HashMap<String, String>) {
    // Cache the compiled regex.
    let re: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{\{lovely:(\w+)\}\}").unwrap());

    let line_replaced = re.replace_all(line, |captures: &Captures| {
        let (_, [var]) = captures.extract();
        let Some(val) = vars.get(var) else {
            panic!("Failed to interpolate an unregistered variable '{var}'");
        };
        val
    });
    *line = line_replaced.to_string();
}

