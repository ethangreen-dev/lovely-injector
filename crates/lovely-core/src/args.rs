use std::{env, path::PathBuf};

use lexopt::prelude::*;
use log::{debug, warn};

#[derive(Debug)]
pub struct Args {
    pub disable_console: bool,
    pub dump_all: bool,
    pub mod_dir: PathBuf,
    pub vanilla: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            disable_console: false,
            dump_all: false,
            mod_dir: Self::get_default_mod_dir(),
            vanilla: false,
        }
    }
}

impl Args {
    pub fn try_parse() -> Result<Args, lexopt::Error> {
        let mut args = Args::default();
        let mut parser = lexopt::Parser::from_env();
        while let Some(arg) = parser.next()? {
            match arg {
                Long("disable-console") => {
                    args.disable_console = true;
                }
                Long("dump-all") => {
                    args.dump_all = true;
                }
                Long("mod-dir") => {
                    args.mod_dir = PathBuf::from(parser.value()?.parse::<String>()?);
                }
                Short('d') | Short('v') | Long("disable-mods") | Long("vanilla") => {
                    args.vanilla = true;
                }
                _ => {
                    // Should we error or ignore like how it's currently done?
                    warn!("Unknown argument: `${arg:?}`")
                    // If we want to error...
                    // return Err(arg.unexpected())
                }
            }
        }

        Ok(args)
    }

    fn get_default_mod_dir() -> PathBuf {
        if let Some(env_path) = env::var_os("LOVELY_MOD_DIR") {
            debug!("Mod dir loaded from env: `${env_path:?}`");
            PathBuf::from(env_path)
        } else {
            let current_exe =
                env::current_exe().expect("Failed to get the path of the current executable.");

            #[cfg(target_os = "macos")]
            let game_name = current_exe
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .expect("Couldn't find parent .app of current executable path")
                .file_name()
                .expect("Failed to get file_name of parent directory of current executable")
                .to_string_lossy()
                .strip_suffix(".app")
                .expect("Parent directory of current executable path was not an .app")
                .replace(".", "_");
            #[cfg(not(target_os = "macos"))]
            let game_name = current_exe
                .file_stem()
                .expect("Failed to get file_stem component of current executable path.")
                .to_string_lossy()
                .replace(".", "_");

            dirs::config_dir().unwrap().join(game_name).join("Mods")
        }
    }
}
