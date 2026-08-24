use std::{collections::HashMap, sync::LazyLock};
use anyhow::{anyhow, Result};

use regex_lite::{Captures, Regex};

/// Apply valid var interpolations to the provided line.
/// Interpolation targets are of form {{lovely:VAR_NAME}}.
pub fn apply_var_interp(line: &mut String, vars: &HashMap<String, String>) -> Result<()> {
    // Cache the compiled regex.
    let re: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{lovely:(\w+)\}\}").unwrap());


    let mut err = Ok(());
    let line_replaced = re.replace_all(line, |captures: &Captures| {
        let (_, [var]) = captures.extract();
        let Some(val) = vars.get(var) else {
            if err.is_ok() {
                err = Err(anyhow!("Failed to interpolate an unregistered variable '{var}'"));
            }
            return ""
        };
        val
    });
    if err.is_err() {
        return err;
    }
    *line = line_replaced.to_string();
    Ok(())
}
