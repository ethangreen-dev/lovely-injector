use std::panic;
use std::slice;
use std::ffi::{c_void, CStr, CString};
use std::fs;
use std::path::PathBuf;

use lovely_core::log::*;
use lovely_core::PatchTable;
use lovely_core::sys::{self, LuaState};

use getargs::{Arg, Options};
use once_cell::sync::OnceCell;
use retour::static_detour;
use widestring::U16CString;
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MESSAGEBOX_STYLE};

static PATCH_TABLE: OnceCell<PatchTable> = OnceCell::new();
static MOD_DIR: OnceCell<PathBuf> = OnceCell::new();

static HAS_INIT: OnceCell<()> = OnceCell::new();

static_detour! {
    pub static LuaLoadbuffer_Detour: unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8) -> u32;
}

unsafe extern "C" fn lua_loadbuffer_detour(state: *mut LuaState, buf_ptr: *const u8, size: isize, name_ptr: *const u8) -> u32 {
    let patch_table = PATCH_TABLE.get().unwrap();

    // Install native function overrides *once*.
    if HAS_INIT.get().is_none() {
        let closure = override_print as *const c_void;
        sys::lua_pushcclosure(state, closure, 0);
        sys::lua_setfield(state, sys::LUA_GLOBALSINDEX, b"print\0".as_ptr() as _);

        patch_table.inject_metadata(state);

        HAS_INIT.set(()).unwrap();
    }

    let name = CStr::from_ptr(name_ptr as _).to_str().unwrap();    

    if !patch_table.needs_patching(name) {
        return LuaLoadbuffer_Detour.call(state, buf_ptr, size, name_ptr);
    }

    let buf = std::slice::from_raw_parts(buf_ptr, (size - 1) as _);
    let buf_str = CString::new(buf).unwrap();
    let buf_str = buf_str.to_str().unwrap();

    let patched = patch_table.apply_patches(name, buf_str, state);

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

    LuaLoadbuffer_Detour.call(state, raw_ptr as _, raw_size as _, name_ptr)
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_: HINSTANCE, reason: u32, _: *const c_void) -> u8 {
    if reason != 1 {
        return 1;
    }
 
    std::panic::set_hook(Box::new(|x| unsafe {
        let message = format!("lovely-injector has crashed: \n{x}");
        error!("{message}");

        let message = U16CString::from_str(message);
        MessageBoxW(
            HWND(0),
            PCWSTR(message.unwrap().as_ptr()),
            w!("lovely-injector"),
            MESSAGEBOX_STYLE(0),
        );
    }));

    let _ = AllocConsole();

    lovely_core::log::init().unwrap_or_else(|e| panic!("Failed to initialize logger: {e:?}"));

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut opts = Options::new(args.iter().map(String::as_str));

    let mut mod_dir = dirs::config_dir().unwrap().join("Balatro\\Mods");
    let mut vanilla = false;

    while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
        match opt {
            Arg::Long("mod-dir") => mod_dir = opts.value().map(PathBuf::from).unwrap_or(mod_dir),
            Arg::Long("vanilla") => vanilla = true,
            _ => (),
        }
    }

    // Stop here if we're runnning in vanilla mode. Don't install hooks, don't setup patches, etc.
    if vanilla {
        info!("Running in vanilla mode");
        return 1;
    }

    if !mod_dir.is_dir() {
        info!("Creating mods directory at {mod_dir:?}");
        fs::create_dir_all(&mod_dir).unwrap();
    }

    info!("Using mods directory at {mod_dir:?}");

    // Patch files are stored within the root of mod subdirectories within the mods dir.
    // - MOD_DIR/lovely.toml
    // - MOD_DIR/lovely/*.toml

    let patch_table = PatchTable::load(&mod_dir)
        .with_loadbuffer(|a, b, c, d| LuaLoadbuffer_Detour.call(a, b, c, d));

    PATCH_TABLE.set(patch_table).unwrap_or_else(|_| panic!("Failed to init PATCH_TABLE static"));
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

/// An override print function, copied piecemeal from the Lua 5.1 source, but in Rust.
/// # Safety
/// Native lua API access. It's unsafe, it's unchecked, it will probably eat your firstborn.
pub unsafe extern "C" fn override_print(state: *mut LuaState) -> isize {
    let argc = sys::lua_gettop(state);
    let mut out = String::new();

    for i in 0..argc {
        let mut str_len = 0_isize; 
        let arg_str = sys::lua_tolstring(state, -1, &mut str_len);
        
        let str_buf = slice::from_raw_parts(arg_str as *const u8, str_len as _);
        let arg_str = String::from_utf8(str_buf.to_vec()).unwrap();

        if i > 1 {
            out.push('\t');
        }

        out.push_str(&format!("[GAME] {arg_str}"));
        sys::lua_settop(state, -(1) - 1);
    }

    info!("{out}");

    0
}
