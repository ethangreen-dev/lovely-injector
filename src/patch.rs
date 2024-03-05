use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;

use once_cell::sync::OnceCell;
use wildmatch::WildMatch;

use crate::manifest::{CopyAt, CopyPatch, Patch, PatternAt, PatternPatch};

pub static PATCHES: OnceCell<Vec<Patch>> = OnceCell::new();
pub static PATCH_TABLE: OnceCell<HashMap<String, Vec<usize>>> = OnceCell::new();

pub fn is_patch_target(name: &str) -> bool {
    let name = name.replacen('@', "", 1);
    PATCH_TABLE.get().unwrap().get(&name).is_some()
}

pub fn apply(input: &str, name: &str) -> Option<String> {
    let name = name.replacen('@', "", 1);
    let patches = PATCH_TABLE
        .get()
        .expect("Failed to get PATCH_TABLE, this is a bug")
        .get(&name)?
        .iter()
        .map(|x| PATCHES.get().unwrap().get(*x))
        .collect::<Vec<_>>();

    // Is this superfluous? Yes.
    if patches.len() == 1 {
        println!("[LOVELY] Applying 1 patch to '{name}'");
    } else {
        println!("[LOVELY] Applying {} patches to '{name}'", patches.len());
    }

    let pattern_patches = patches.iter().filter_map(|patch| match patch {
        Some(Patch::Pattern(x)) => Some(x),
        _ => None,
    }).collect::<Vec<_>>();

    let lines = input.lines();
    let mut out = Vec::new();

    for line in lines {
        let mut new_line = apply_pattern_patches(line, &pattern_patches[..]);
        out.append(&mut new_line);
    }

    let copy_patches = patches.iter().filter_map(|patch| match patch {
        Some(Patch::Copy(x)) => Some(x),
        _ => None,
    }).collect::<Vec<_>>();

    let out = out.join("\n");
    let out = apply_copy_patches(&out, &copy_patches[..]);

    Some(out)
}

fn apply_pattern_patches(line: &str, patches: &[&PatternPatch]) -> Vec<String> {
    // Perform pattern matching for each patch.
    let trimmed = line.trim_start();
    let matches = patches
        .iter()
        .filter(|x| WildMatch::new(&x.pattern).matches(trimmed));

    let mut line = line.to_string();
    let mut before: Vec<String> = Vec::new();
    let mut after: Vec<String> = Vec::new();

    for patch in matches {
        let indent = if patch.match_indent {
            line.chars().take_while(|x| *x == ' ' || *x == '\t').collect::<String>()
        } else {
            String::new()
        };

        let payload = format!("{indent}{}", patch.payload.as_ref().unwrap());
        match patch.position {
            PatternAt::At => {
                line = payload.clone()
            }
            PatternAt::After => {
                after.push(payload.clone()) 
            },
            PatternAt::Before => {
                before.push(payload.clone())
            },
        }
    }

    before.push(line);
    before.append(&mut after);
    before
}


fn apply_copy_patches(input: &str, patches: &[&CopyPatch]) -> String {
    let mut out = input.to_string();

    for patch in patches {
        let payload = merge_payloads(&patch.sources);
        match patch.position {
            CopyAt::Append => {
                out = format!("{out}\n{payload}")
            },
            CopyAt::Prepend => {
                out = format!("{payload}\n{out}")
            }
        }
    }

    out
}

fn merge_payloads(sources: &Vec<PathBuf>) -> String {
    let mut merged = Vec::new();
    for source in sources {
        let contents = fs::read_to_string(&source)
            .unwrap_or_else(|_| panic!("Failed to read payload file at '{source:?}'."));

        merged.push(contents);
    }

    merged.join("\n")
}
