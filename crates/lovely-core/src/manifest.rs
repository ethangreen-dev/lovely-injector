use std::{collections::HashMap, path::PathBuf};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub version: String,
    pub dump_lua: bool,
    pub priority: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PatchManifest {
    pub manifest: Manifest,
    pub patches: Vec<Patch>,

    // A table of variable name = value bindings. These are interpolated
    // into injected source code as the *last* step in the patching process.
    #[serde(default)]
    pub vars: HashMap<String, String>,

    // A table of arguments, read and parsed from the environment command line.
    // Binds double-hyphenated argument names (--arg) to a value, with additional metadata
    // available to produce help messages, set default values, and apply other behavior.
    #[serde(default)]
    pub args: HashMap<String, PatchArgs>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PatchArgs {
    // An optional help string. This will be printed out in the calling console
    // (if available) when the --help argument is supplied.
    pub help: Option<String>,
    
    // An optional default value. Not including a default value will cause Lovely
    // to panic if this argument is missing or could not be parsed.
    // Consider this to be both a "default value" and a "required" field, depending
    // on whether or not it's set.
    pub default: Option<String>,

    // This field allows for a patch author to force lovely to parse incoming arguments
    // with the exact name that they are defined by.
    // This disables lovely's automatic underscore to hyphen conversion. 
    #[serde(default)]
    pub name_override: bool,

    // This field allows for arguments (--arg) to be passed without implicit values,
    // treating it essentially as a flag. If it exists in the args, it's true, if not,
    // then we set it to false.
    #[serde(default)]
    pub treat_as_flag: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Patch {
    // A patch which applies some change to a series of line(s) after a line with a match
    // to the provided pattern has been found.
    Pattern(PatternPatch),
    Copy(CopyPatch),
    Module(ModulePatch),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PatternPatch {
    // The pattern that the line will be matched against. Very simple,
    // supports only `?` (one occurance of any character) and `*` (any numver of any character).
    // Patterns are matched against a left-trimmed version of the line, so whitespace does not
    // need to be considered.
    pub pattern: String,

    // The position to insert the target at. `PatternAt::At` replaces the matched line entirely.
    pub position: PatternAt,
    pub target: String,
    pub payload_files: Option<Vec<String>>,
    pub payload: String,
    pub match_indent: bool,
    pub overwrite: bool,

    // Enable the regex pattern match / substitution engine.
    // Queries can be tested here: https://rustexp.lpil.uk/
    #[serde(default)]
    pub complex: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum PatternAt {
    At,
    Before,
    After,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct CopyPatch {
    pub position: CopyAt,
    pub target: String,
    pub sources: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum CopyAt {
    Append,
    Prepend,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModulePatch {
    pub source: PathBuf,
    pub before: String,
    pub name: String,
}
