mod manifest;
mod patch;

use std::collections::HashMap;
use std::fs;
use std::ffi::{c_void, CStr, CString};
use std::path::PathBuf;

use getargs::{Arg, Options};
use retour::static_detour;

use windows::core::{s, w};
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::ProcessStatus::GetProcessImageFileNameA;

use once_cell::sync::OnceCell;

use crate::manifest::{Patch, PatchManifest};

static LOADER_DIR: OnceCell<PathBuf> = OnceCell::new();
static MOD_DIR: OnceCell<PathBuf> = OnceCell::new();

static_detour! {
    pub static LuaLoadbuffer_Detour: unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8) -> u32;
}

unsafe extern "C" fn lua_loadbuffer_detour(lua_state: *mut c_void, buf_ptr: *const u8, size: isize, name_ptr: *const u8) -> u32 {
    let name = CStr::from_ptr(name_ptr as _).to_str().unwrap();
    if !patch::is_patch_target(name) {
        return LuaLoadbuffer_Detour.call(lua_state, buf_ptr, size, name_ptr);
    }

    let buf = std::slice::from_raw_parts(buf_ptr, size as _);
    let buf_str = CString::new(buf).unwrap();
    let buf_str = buf_str.to_str().unwrap();

    let patched = patch::apply(buf_str, name);
    if patched.is_none() {
        return LuaLoadbuffer_Detour.call(lua_state, buf_ptr, size, name_ptr);
    }

    let patched = patched.unwrap();
    let patch_dump = MOD_DIR.get_unchecked()
        .join("lovely")
        .join("dump")
        .join(name);
    let dump_parent = patch_dump.parent().unwrap();

    if !dump_parent.is_dir() {
        fs::create_dir_all(dump_parent).unwrap();
    }
    fs::write(patch_dump, &patched).unwrap();

    let raw = CString::new(patched).unwrap();
    let raw_size = raw.as_bytes().len();
    let raw_ptr = raw.into_raw();

    LuaLoadbuffer_Detour.call(lua_state, raw_ptr as _, raw_size as _, name_ptr)
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_: *mut c_void, reason: u32, _: *const c_void) -> u8 {
    let _ = AllocConsole();

    if reason != 1 {
        return 1;
    }
    
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut opts = Options::new(args.iter().map(String::as_str));

    let mut mod_dir = dirs::config_dir().unwrap().join("Balatro\\Mods");

    while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
        match opt {
            Arg::Long("mod-dir") if opts.value().is_ok() => mod_dir = PathBuf::from(opts.value().unwrap()),
            _ => (),
        }
    }

    if !mod_dir.is_dir() {
        println!("[LOVELY] Creating mods directory at {mod_dir:?}");
        fs::create_dir_all(&mod_dir).unwrap();
    }

    println!("[LOVELY] Using mods directory at {mod_dir:?}");

    // Patch files are stored within the root of mod subdirectories within the mods dir.
    // - MOD_DIR/lovely.toml
    // - MOD_DIR/lovely/*.toml

    let mod_dirs = fs::read_dir(&mod_dir)
        .unwrap_or_else(|_| panic!("Failed to read from mod directory at '{mod_dir:?}'."))
        .filter_map(|x| x.ok())
        .filter(|x| x.path().is_dir());

    let patch_files = mod_dirs
        .flat_map(|x| {
            let lovely_toml = x.path().join("lovely.toml");
            let lovely_dir = x.path().join("lovely");
            let mut files = Vec::new();

            if lovely_toml.is_file() {
                files.push(lovely_toml)
            }
            
            if lovely_dir.is_dir() {
                let mut subfiles = fs::read_dir(&lovely_dir)
                    .unwrap_or_else(|_| panic!("Failed to read from lovely directory at '{lovely_dir:?}'."))
                    .filter_map(|x| x.ok())
                    .map(|x| x.path())
                    .filter(|x| x.is_file())
                    .filter(|x| x.extension().unwrap() == "toml")
                    .collect::<Vec<_>>();
                files.append(&mut subfiles);
            }

            files
        })
        .collect::<Vec<_>>();

    let mut patches: Vec<Patch> = Vec::new();

    // Load n > 0 patch files from the patch directory, collecting them for later processing.
    for patch_file in patch_files {
        let patch_dir = patch_file.parent().unwrap();
        let mod_dir = if patch_dir.file_name().unwrap() == "lovely" {
            patch_dir.parent().unwrap()
        } else {
            patch_dir
        };

        let mut patch: PatchManifest = {
            let str = fs::read_to_string(&patch_file).unwrap_or_else(|_| panic!("Failed to read patch file at '{patch_file:?}'"));
            toml::from_str(&str).unwrap_or_else(|e| panic!("Failed to parse patch file at '{patch_file:?}'. Error: {e:?}"))
        };
        for patch in &mut patch.patches[..] {
            if let Patch::Copy(ref mut x) = patch {
                x.sources = x.sources.iter_mut().map(|x| mod_dir.join(x)).collect();
            }
        }

        let inner_patches = patch.patches.as_mut(); 
        patches.append(inner_patches);
    }

    let mut patch_table: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, patch) in patches.iter().enumerate() {
        let target = match patch {
            Patch::Pattern(x) => &x.target,
            Patch::Copy(x) => &x.target,
        };

        // Initialize a patch table entry with the new target.
        if !patch_table.contains_key(target) {
            patch_table.insert(target.clone(), vec![i]);
            continue;
        }

        // Extend the patch table entry with an additional patch reference. 
        let vals = patch_table.get_mut(target).unwrap();
        vals.push(i);
    }

    if patches.len() == 1 {
        println!("[LOVELY] Registered 1 patch");
    } else {
        println!("[LOVELY] Registered {} patches", patches.len());
    }

    patch::PATCHES.set(patches).unwrap();
    patch::PATCH_TABLE.set(patch_table).unwrap();
    MOD_DIR.set(mod_dir).unwrap();
    
    // Quick and easy hook injection. Load the lua51.dll module at runtime, determine the address of the luaL_loadbuffer fn, hook it.
    let handle = LoadLibraryW(w!("lua51.dll")).unwrap();
    let proc = GetProcAddress(handle, s!("luaL_loadbuffer")).unwrap();
    let fn_target = std::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8) -> u32>(proc);

    LuaLoadbuffer_Detour.initialize(
        fn_target, 
        |a, b, c, d| lua_loadbuffer_detour(a, b, c, d)
    )
    .unwrap()
    .enable()
    .unwrap();

    1
}
