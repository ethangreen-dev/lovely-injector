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
    #[serde(default)]
    pub vars: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Patch {
    Pattern(PatternPatch),
    Copy(CopyPatch),
    Module(ModulePatch),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PatternPatch {
    pub pattern: String,
    pub position: PatternAt,
    pub target: String,
    pub payload_files: Option<Vec<String>>,
    pub payload: String,
    pub match_indent: bool,
    pub overwrite: bool,
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
