#[allow(non_upper_case_globals)]

pub mod sys;
mod manifest;
mod patch;

use std::collections::HashMap;
use std::fs;
use std::ffi::{c_void, CStr, CString};
use std::path::PathBuf;

use getargs::{Arg, Options};
use manifest::ModulePatch;
use retour::static_detour;

use sha2::{Digest, Sha256};
use widestring::U16CString;
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows::Win32::System::Console::{AllocConsole, SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MESSAGEBOX_STYLE};

use once_cell::sync::OnceCell;

use crate::manifest::{Patch, PatchManifest};

static LOADER_DIR: OnceCell<PathBuf> = OnceCell::new();
static MOD_DIR: OnceCell<PathBuf> = OnceCell::new();

static FILE_PATCHES: OnceCell<Vec<ModulePatch>> = OnceCell::new();

static_detour! {
    pub static LuaLoadbuffer_Detour: unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8) -> u32;
}

unsafe extern "C" fn lua_loadbuffer_detour(lua_state: *mut c_void, buf_ptr: *const u8, size: isize, name_ptr: *const u8) -> u32 {
    let name = CStr::from_ptr(name_ptr as _).to_str().unwrap();

    // Search for a patch file to be loaded before this one.
    let load_target = FILE_PATCHES.get().unwrap().iter().find(|x| format!("@{}",x.before) == name);
    if let Some(patch) = load_target {
        patch::load_file(patch, lua_state);
    }

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
 
    // Compute a sha256 hash of the patched buffer.
    let mut hasher = Sha256::new();
    hasher.update(patched.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // Prefix the patched buffer with the hash. This is not efficient, but it works.
    let patched = format!("LOVELY_INTEGRITY = '{hash}'\n\n{patched}");

    fs::write(patch_dump, &patched).unwrap();

    let raw = CString::new(patched).unwrap();
    let raw_size = raw.as_bytes().len();
    let raw_ptr = raw.into_raw();

    LuaLoadbuffer_Detour.call(lua_state, raw_ptr as _, raw_size as _, name_ptr)
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_: *mut c_void, reason: u32, _: *const c_void) -> u8 {
    // Setup console redirection, replacing Love's own implementation.
    let _ = AllocConsole();
 
    std::panic::set_hook(Box::new(|x| unsafe {
        let message = format!("lovely-injector has crashed: \n{x}");

        let message = U16CString::from_str(message);
        MessageBoxW(
            HWND(0),
            PCWSTR(message.unwrap().as_ptr()),
            w!("lovely-injector"),
            MESSAGEBOX_STYLE(0),
        );
    }));

    let c_handle = CreateFileW(
        w!("CONOUT$"),
        0x40000000, // GENERIC_WRITE
        FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None
    ).expect("Failed to open CONOUT$");

    SetStdHandle(STD_INPUT_HANDLE, c_handle).unwrap();
    SetStdHandle(STD_ERROR_HANDLE, c_handle).unwrap();

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
    let mut var_table: HashMap<String, String> = HashMap::new();   

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

            if let Patch::Module(ref mut x) = patch {
                x.source = mod_dir.join(&x.source);
            }
        }

        let inner_patches = patch.patches.as_mut(); 
        patches.append(inner_patches);
        var_table.extend(patch.vars);
    }

    let mut file_patches: Vec<ModulePatch> = Vec::new();
    let mut patch_table: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, patch) in patches.iter().enumerate() {
        let target = match patch {
            Patch::Pattern(x) => &x.target,
            Patch::Copy(x) => &x.target,

            // File patches don't need to be registered in the table, so load them normally and skip iteration.
            Patch::Module(x) => {
                file_patches.push(x.clone());
                continue;
            }
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
    patch::VAR_TABLE.set(var_table).unwrap();
    
    FILE_PATCHES.set(file_patches).unwrap();
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
