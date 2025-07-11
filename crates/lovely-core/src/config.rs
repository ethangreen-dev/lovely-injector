use std::path::PathBuf;

use getargs::{Arg, Options};

pub struct LovelyConfig {
    pub dump_all: bool,
    pub vanilla: bool,
    pub mod_dir: Option<PathBuf>,
}

impl LovelyConfig {
    pub fn parse_args(args: &[String]) -> Self {
        let mut config = LovelyConfig {
            dump_all: false,
            vanilla: false,
            mod_dir: None,
        };
        
        let mut opts = Options::new(args.iter().skip(1).map(String::as_str));
        while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
            match opt {
                Arg::Long("mod-dir") => {
                    config.mod_dir = opts.value().map(PathBuf::from).ok()
                }
                Arg::Long("vanilla") => config.vanilla = true,
                Arg::Long("--dump-all") => config.dump_all = true,
                _ => (),
            }
        };

        config
    }
}