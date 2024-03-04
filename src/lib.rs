use std::fs;
use std::ffi::{c_void, CStr, CString};
use std::path::PathBuf;

use getargs::{Arg, Options};
use retour::static_detour;

use windows::core::{s, w};
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use once_cell::sync::OnceCell;

static LOADER_DIR: OnceCell<PathBuf> = OnceCell::new();
static MOD_DIR: OnceCell<PathBuf> = OnceCell::new();

static_detour! {
    pub static LuaLoadbuffer_Detour: unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8);
}

unsafe extern "C" fn lua_loadbuffer_detour(lua_state: *mut c_void, buff: *const u8, size: isize, name_buf: *const u8) {
    let name = CStr::from_ptr(name_buf as _).to_str().unwrap();

    if !["@main.lua", "@game.lua"].contains(&name) {
        return LuaLoadbuffer_Detour.call(lua_state, buff, size, name_buf);
    }

    let input_buf = std::slice::from_raw_parts(buff, size as _);
    let input = CString::from_vec_unchecked(input_buf.to_vec());
    let input = input.to_str().unwrap();

    if name == "@main.lua" {
        let main = load_main(input.to_string());
        let raw = CString::new(main).unwrap();
        let raw_nul = raw.as_bytes();

        return LuaLoadbuffer_Detour.call(lua_state, raw_nul.as_ptr(), raw_nul.len() as _, name_buf);
    }

    let mut input_lines = input.split('\n').collect::<Vec<_>>();
    let line_match = input_lines.iter().position(|x| x.trim() == "self.SPEEDFACTOR = 1").unwrap();

    input_lines.insert(line_match + 1, "    initSteamodded()");
    let input_lines = input_lines.join("\n");

    fs::write("game.lua", &input_lines).unwrap();
    let raw = CString::new(input_lines).unwrap();
    let raw_nul = raw.as_bytes();

    LuaLoadbuffer_Detour.call(lua_state, raw_nul.as_ptr(), raw_nul.len() as _, name_buf)
}

unsafe fn load_main(input: String) -> String {
    let dirs = ["core", "debug", "loader"]
        .into_iter()
        .map(|x| LOADER_DIR.get_unchecked().join(x))
        .collect::<Vec<_>>();

    dbg!(&dirs);

    let dir_contents = dirs.into_iter().flat_map(|x| fs::read_dir(x)
        .expect("Failed to read directory")
        .filter_map(|x| x.ok())
        .filter(|x| x.path().extension().unwrap() == "lua")
        .map(|x| fs::read_to_string(x.path()).expect("Failed to read to string")))
        .collect::<Vec<_>>()
        .join("\n");

    let out = format!("{input}\n{dir_contents}");
    fs::write("main.lua", &out).unwrap();

    out
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_: *mut c_void, reason: u32, _: *const c_void) -> u8 {
    AllocConsole();
 
    if reason != 1 {
        return 1;
    }

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut opts = Options::new(args.iter().map(String::as_str));

    while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
        match opt {
            Arg::Long("loader-dir") => LOADER_DIR.set(PathBuf::from(opts.value().expect("`--loader-dir` argument has no value."))).unwrap(),
            Arg::Long("mod-dir") => MOD_DIR.set(PathBuf::from(opts.value().expect("`--mod-dir` argument has no value."))).unwrap(),
            _ => (),
        }
    }

    let handle = LoadLibraryW(w!("lua51.dll")).unwrap();
    let proc = GetProcAddress(handle, s!("luaL_loadbuffer")).unwrap();
    let fn_target = std::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8)>(proc);

    LuaLoadbuffer_Detour.initialize(
        fn_target, 
        |a, b, c, d| lua_loadbuffer_detour(a, b, c, d)
    )
    .unwrap()
    .enable()
    .unwrap();

    1
}
