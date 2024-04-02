use std::ffi::{CStr, CString};
use std::fs;
use std::path::PathBuf;

use getargs::{Arg, Options};
use lovely_core::sys::LuaState;

use lovely_core::PatchTable;
use once_cell::sync::{Lazy, OnceCell};

static PATCH_TABLE: OnceCell<PatchTable> = OnceCell::new();
static MOD_DIR: OnceCell<PathBuf> = OnceCell::new();

static RECALL: Lazy<unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8) -> u32> = Lazy::new(|| unsafe {
    let handle = libc::dlopen(b"../Frameworks/Lua.framework/Versions/A/Lua\0".as_ptr() as _, libc::RTLD_LAZY);
    
    if handle.is_null() {
        panic!("Failed to load lua");
    }
    let ptr = libc::dlsym(handle, b"luaL_loadbuffer\0".as_ptr() as _);
    
    if ptr.is_null() {
        panic!("Failed to load luaL_loadbuffer");
    }
    std::mem::transmute::<_, unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8) -> u32>(ptr)
    
});

#[no_mangle]
unsafe extern "C" fn luaL_loadbuffer(state: *mut LuaState, buf_ptr: *const u8, size: isize, name_ptr: *const u8) -> u32 {
    let name = CStr::from_ptr(name_ptr as _).to_str().unwrap();
    let patch_table = PATCH_TABLE.get().unwrap();
    
    if !patch_table.needs_patching(name) {
        println!("Skipping patching for {name}");
        return RECALL(state, buf_ptr, size, name_ptr);
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

    RECALL(state, raw_ptr as _, raw_size as _, name_ptr)
}

// Mark this function as a global constructor (like C++).
#[ctor::ctor]
fn construct() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut opts = Options::new(args.iter().map(String::as_str));

    let mut mod_dir = dirs::config_dir().unwrap().join("Balatro/Mods");

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

    let patch_table = PatchTable::load(&mod_dir)
        .with_loadbuffer(|a, b, c, d| unsafe { luaL_loadbuffer(a, b, c, d) });

    PATCH_TABLE.set(patch_table).unwrap_or_else(|_| panic!("Failed to init PATCH_TABLE static"));
    MOD_DIR.set(mod_dir).unwrap();
}
