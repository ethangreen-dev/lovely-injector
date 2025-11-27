use anyhow::{bail, Context, Result};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::patch::{Patch, PatchFile, Priority};
use itertools::Itertools;
use log::*;
use walkdir::WalkDir;
use zip::ZipArchive;

/// Patch file with preloaded TOML content and referenced sources
struct IntermediatePatch {
    pub path: PathBuf,
    pub content: String,
    /// Preloaded source files referenced by module/copy patches (relative path -> content)
    pub sources: HashMap<PathBuf, String>,
}

/// Compare two file paths by their lowercase filenames
fn filename_cmp(first: &Path, second: &Path) -> Ordering {
    let first = first.file_name().unwrap().to_string_lossy().to_lowercase();
    let second = second.file_name().unwrap().to_string_lossy().to_lowercase();
    first.cmp(&second)
}

/// Load patch files from the specified mod directory.
fn get_dir_patches(mod_dir: &Path) -> Result<(PathBuf, Vec<IntermediatePatch>)> {
    let lovely_toml = mod_dir.join("lovely.toml");
    let lovely_dir = mod_dir.join("lovely");
    let mut toml_files = Vec::new();

    if lovely_toml.is_file() {
        toml_files.push(lovely_toml);
    }

    if lovely_dir.is_dir() {
        let mut subfiles = WalkDir::new(&lovely_dir)
            .into_iter()
            .filter_map(|x| x.ok())
            .map(|x| x.path().to_path_buf())
            .filter(|x| x.is_file())
            .filter(|x| x.extension().is_some_and(|x| x == "toml"))
            .sorted_by(|a, b| filename_cmp(a, b))
            .collect_vec();
        toml_files.append(&mut subfiles);
    }

    let intermediate_patches = toml_files
        .into_iter()
        .map(|toml_path| {
            let content = fs::read_to_string(&toml_path)
                .with_context(|| format!("Failed to read patch file at {:?}", toml_path))?;

            // Parse TOML to find referenced sources and preload them
            let mut sources: HashMap<PathBuf, String> = HashMap::new();
            let file_identifier = format!("{:?}", toml_path);
            if let Ok(patch_file) = parse_patch_file(&content, &file_identifier) {
                for patch in &patch_file.patches {
                    match patch {
                        Patch::Module(x) => {
                            let full_path = mod_dir.join(&x.source);
                            if let Ok(source_content) = fs::read_to_string(&full_path) {
                                sources.insert(x.source.clone(), source_content);
                            }
                        }
                        Patch::Copy(x) => {
                            let Some(ref copy_sources) = x.sources else { continue };
                            for source in copy_sources {
                                let full_path = mod_dir.join(source);
                                if let Ok(source_content) = fs::read_to_string(&full_path) {
                                    sources.insert(source.clone(), source_content);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            Ok(IntermediatePatch {
                path: toml_path,
                content,
                sources,
            })
        })
        .collect::<Result<Vec<IntermediatePatch>>>()?;

    Ok((mod_dir.to_path_buf(), intermediate_patches))
}

/// Load patch files from the specified zip.
fn get_zip_patches(zip_file: &Path) -> Result<(PathBuf, Vec<IntermediatePatch>)> {
    let file = fs::File::open(zip_file)
        .with_context(|| format!("Failed to open zip file at {:?}", zip_file))?;
    let mut zip = ZipArchive::new(file)
        .with_context(|| format!("Failed to open zip archive for {:?}", zip_file))?;

    // Grab files names
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    // Find the mod root. This is the dir that contains lovely.toml or lovely/
    let mod_root = names
        .iter()
        .find_map(|name| {
            let parent = get_parent(name);
            let lovely_toml = format!("{}lovely.toml", parent);
            let lovely_dir = format!("{}lovely/", parent);

            if names
                .iter()
                .any(|n| n == &lovely_toml || n.starts_with(&lovely_dir))
            {
                Some(parent)
            } else {
                None
            }
        })
        .with_context(|| format!("No mod root found in zip {:?}", zip_file))?;

    let lovely_toml_path = format!("{}lovely.toml", mod_root);
    let lovely_dir_prefix = format!("{}lovely/", mod_root);

    let mut toml_paths: Vec<String> = names
        .iter()
        .filter(|name| {
            !name.ends_with('/')
                && (name == &&lovely_toml_path
                    || (name.starts_with(&lovely_dir_prefix) && name.ends_with(".toml")))
        })
        .cloned()
        .collect();

    // Sort toml paths by filename
    toml_paths.sort_by(|a, b| {
        let a_path = Path::new(a);
        let b_path = Path::new(b);
        filename_cmp(a_path, b_path)
    });

    // First pass: read all TOML files and parse them to find referenced sources
    let mut toml_contents: Vec<(String, String)> = Vec::new();
    let mut source_paths: HashSet<String> = HashSet::new();

    for toml_path in &toml_paths {
        let mut file = zip
            .by_name(toml_path)
            .with_context(|| format!("Failed to read {} from zip {:?}", toml_path, zip_file))?;

        let mut content = String::new();
        file.read_to_string(&mut content).with_context(|| {
            format!("Failed to read contents of {} from zip {:?}", toml_path, zip_file)
        })?;

        // Parse TOML to find referenced module/copy sources
        let file_identifier = format!("{} from zip {:?}", toml_path, zip_file);
        if let Ok(patch_file) = parse_patch_file(&content, &file_identifier) {
            for patch in &patch_file.patches {
                match patch {
                    Patch::Module(x) => {
                        let source_path = format!("{}{}", mod_root, x.source.to_string_lossy());
                        source_paths.insert(source_path);
                    }
                    Patch::Copy(x) => {
                        if let Some(ref sources) = x.sources {
                            for source in sources {
                                let source_path = format!("{}{}", mod_root, source.to_string_lossy());
                                source_paths.insert(source_path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        toml_contents.push((toml_path.clone(), content));
    }

    // Second pass: read all referenced source files from zip
    let mut file_contents: HashMap<String, String> = HashMap::new();
    for source_path in &source_paths {
        if let Ok(mut file) = zip.by_name(source_path) {
            let mut content = String::new();
            file.read_to_string(&mut content).with_context(|| {
                format!("Failed to read source {} from zip {:?}", source_path, zip_file)
            })?;
            file_contents.insert(source_path.clone(), content);
        }
    }

    // Build IntermediatePatches with preloaded sources
    let intermediate_patches = toml_contents
        .into_iter()
        .map(|(toml_path, content)| {
            // Create intermediate path: zip_file/relative_path_from_mod_root
            let relative_path = &toml_path[mod_root.len()..];
            let intermediate_path = zip_file.join(relative_path);

            // Build sources map with paths relative to mod_root
            let mut sources: HashMap<PathBuf, String> = HashMap::new();
            for (full_path, content) in &file_contents {
                let relative = &full_path[mod_root.len()..];
                sources.insert(PathBuf::from(relative), content.clone());
            }

            IntermediatePatch {
                path: intermediate_path,
                content,
                sources,
            }
        })
        .collect();

    Ok((zip_file.to_path_buf(), intermediate_patches))
}

/// Load patches from the provided mod directory. This scans for lovely patch files
/// within each subdirectory that matches either:
/// - MOD_DIR/lovely.toml
/// - MOD_DIR/lovely/*.toml
/// 
/// Zip archives are supported and uniquely support directory nesting 
/// (i.e., mod.zip/dir/lovely.toml), but otherwise are treated the same as dir mods.
pub fn load_patches_new(mod_dir: &Path) -> Result<Vec<(Patch, Priority, PathBuf, HashMap<String, String>)>> {
    let blacklist_file = mod_dir.join("lovely").join("blacklist.txt");

    let mut blacklist: HashSet<String> = HashSet::new();
    if fs::exists(&blacklist_file)? {
        let text = fs::read_to_string(blacklist_file).context("Could not read blacklist")?;

        blacklist.extend(
            text.lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|line| line.to_string()),
        );
    } else {
        info!("No blacklist.txt in Mods/lovely.");
    }

    let mod_contents = fs::read_dir(mod_dir)
        .with_context(|| format!("Failed to read from mod directory within {mod_dir:?}"))?
        .filter_map(|x| x.ok())
        .map(|x| x.path())
        .filter(|x| {
            let cname = x.file_name();
            let name = cname.and_then(|x| x.to_str()).unwrap_or_default();
            let blacklisted = blacklist.contains(name);
            if blacklisted {
                info!("'{name}' was found in blacklist, skipping it.");
            }
            !blacklisted
        })
        .collect_vec();

    // Collect directory patches (read TOMLs into IntermediatePatch)
    let dir_results: Vec<(PathBuf, Vec<IntermediatePatch>)> = mod_contents
        .iter()
        .filter(|x| x.is_dir())
        .filter(|x| {
            let ignore_file = x.join(".lovelyignore");
            let dirname = x
                .file_name()
                .unwrap_or_else(|| panic!("Failed to read directory name of {x:?}"))
                .to_string_lossy();
            if ignore_file.is_file() {
                info!("Found .lovelyignore in '{dirname}', skipping it.");
            }
            !ignore_file.is_file()
        })
        .sorted_by(|a, b| filename_cmp(a, b))
        .map(|x| get_dir_patches(x))
        .collect::<Result<Vec<_>>>()?;

    // Collect zip patches (read TOMLs into IntermediatePatch)
    let zip_results: Vec<(PathBuf, Vec<IntermediatePatch>)> = mod_contents
        .iter()
        .filter(|x| x.is_file())
        .filter(|x| x.extension().is_some_and(|ext| ext == "zip"))
        .sorted_by(|a, b| filename_cmp(a, b))
        .map(|x| get_zip_patches(x))
        .collect::<Result<Vec<_>>>()?;

    // Parse TOML contents into PatchFile structures
    let mut patches: Vec<(Patch, Priority, PathBuf, HashMap<String, String>)> = Vec::new();

    // Handle all patch files using preloaded sources
    let all_results = dir_results.into_iter().chain(zip_results.into_iter());

    for (_base_path, ips) in all_results {
        for ip in ips {
            let file_identifier = format!("{:?}", ip.path);
            let mut patch_file: PatchFile = parse_patch_file(&ip.content, &file_identifier)?;

            // For module and copy patches, use preloaded sources
            for patch in &mut patch_file.patches {
                if let Patch::Module(ref mut x) = patch {
                    if x.load_now && x.before.is_none() {
                        bail!(
                            "Error at patch file {}:\nModule \"{}\" has \"load_now\" set to true, but does not have required parameter \"before\" set",
                            ip.path.display(),
                            x.name
                        );
                    }

                    x.display_source = x.source.to_string_lossy().to_string();
                    x.content = ip.sources.get(&x.source)
                        .with_context(|| format!(
                            "Module source {:?} not found in preloaded sources for patch from {}",
                            x.source,
                            ip.path.display()
                        ))?
                        .clone();
                }

                let Patch::Copy(ref mut x) = patch else { continue };
                let Some(ref sources) = x.sources else { continue };

                for source in sources {
                    let source_content = ip.sources.get(source)
                        .with_context(|| format!(
                            "Copy source {:?} not found in preloaded sources for patch from {}",
                            source,
                            ip.path.display()
                        ))?;
                    x.contents.push(source_content.clone());
                }
            }

            let priority = patch_file.manifest.priority;
            let vars = patch_file.vars;

            // mod_relative_path: path relative to top-level mod_dir
            let mod_relative_path = ip.path.strip_prefix(mod_dir).with_context(|| {
                format!(
                    "Base mod directory path {} expected to be a prefix of patch file path {}",
                    mod_dir.display(),
                    ip.path.display()
                )
            })?;

            let patches_vec = patch_file
                .patches
                .into_iter()
                .map(|patch| (patch, priority, mod_relative_path.to_path_buf(), vars.clone()));

            patches.extend(patches_vec);
        }
    }

    Ok(patches)
}

/// Process raw patches to extract targets and consolidate variables
pub fn process_patches(
    raw_patches: Vec<(Patch, Priority, PathBuf, HashMap<String, String>)>,
) -> (
    Vec<(Patch, Priority, PathBuf)>,
    HashSet<String>,
    HashMap<String, String>,
) {
    let mut targets: HashSet<String> = HashSet::new();
    let mut patches: Vec<(Patch, Priority, PathBuf)> = Vec::new();
    let mut var_table: HashMap<String, String> = HashMap::new();

    for (patch, priority, path, vars) in raw_patches {
        // Extract targets from patches
        match &patch {
            Patch::Copy(x) => {
                x.target.insert_into(&mut targets);
            }
            Patch::Module(x) => {
                targets.insert(x.before.clone().unwrap_or_default());
            }
            Patch::Pattern(x) => {
                x.target.insert_into(&mut targets);
            }
            Patch::Regex(x) => {
                x.target.insert_into(&mut targets);
            }
        }

        // Add to final patches
        patches.push((patch, priority, path));

        // Add variables (later ones override earlier ones)
        var_table.extend(vars);
    }

    (patches, targets, var_table)
}

/// Parse TOML content into a PatchFile
fn parse_patch_file(content: &str, file_identifier: &str) -> Result<PatchFile> {
    let ignored_key_callback = |key: serde_ignored::Path| {
        warn!("Unknown key `{key}` found in patch file {file_identifier}, ignoring it");
    };

    serde_ignored::deserialize(toml::Deserializer::new(content), ignored_key_callback)
        .with_context(|| format!("Failed to parse patch file {file_identifier}"))
}



/// Helper to extract parent directory path with trailing slash
fn get_parent(path: &str) -> String {
    path.rfind('/')
        .map(|i| &path[..=i])
        .unwrap_or("")
        .to_string()
}
