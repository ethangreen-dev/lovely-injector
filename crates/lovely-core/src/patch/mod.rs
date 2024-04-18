use std::borrow::{Borrow, Cow};
use std::{fs, sync::Mutex};
use std::path::PathBuf;
use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};

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

