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

