use std::env;
use std::ffi::c_void;
use std::panic;

use itertools::Itertools;
use libloading::Library;
use lovely_core::log::*;
use lovely_core::sys::set_lua_lib;
use lovely_core::sys::LuaState;
use lovely_core::Lovely;
use lovely_core::LOVELY_VERSION;

use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use retour::static_detour;
use widestring::U16CString;
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::System::Console::{AllocConsole, SetConsoleTitleW};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MESSAGEBOX_STYLE};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static_detour! {
    pub static LuaLoadbufferx_Detour: unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8,*const u8) -> u32;
}

static WIN_TITLE: Lazy<U16CString> =
    Lazy::new(|| U16CString::from_str(format!("Lovely {LOVELY_VERSION}")).unwrap());

unsafe extern "C" fn lua_loadbufferx_detour(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: isize,
    name_ptr: *const u8,
    mode_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, mode_ptr)
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_: HINSTANCE, reason: u32, _: *const c_void) -> u8 {
    if reason != 1 {
        return 1;
    }

    panic::set_hook(Box::new(|x| unsafe {
        let message = format!("lovely-injector has crashed: \n{x}");
        error!("{message}");

        let message = U16CString::from_str(message);
        MessageBoxW(
            HWND(0),
            PCWSTR(message.unwrap().as_ptr()),
            PCWSTR(WIN_TITLE.as_ptr()),
            MESSAGEBOX_STYLE(0),
        );
    }));

    set_lua_lib(Library::new("lua51.dll").unwrap());

    let args = env::args().collect_vec();

    if args.contains(&"--disable-mods".to_string()) || args.contains(&"-d".to_string()) {
        return 1;
    }

    if !args.contains(&"--disable-console".to_string()) {
        let _ = AllocConsole();
        SetConsoleTitleW(PCWSTR(WIN_TITLE.as_ptr())).expect("Failed to set console title.");
    }

    let dump_all = args.contains(&"--dump-all".to_string());

    // Validate that an older Lovely install doesn't already exist within the game directory.
    let exe_path = env::current_exe().unwrap();
    let game_dir = exe_path.parent().unwrap();
    let dwmapi = game_dir.join("dwmapi.dll");

    if dwmapi.is_file() {
        panic!(
            "An old Lovely installation was detected within the game directory. \
            This problem MUST BE FIXED before you can start the game.\n\nTO FIX: Delete the file at {dwmapi:?}"
        );
    }

    // Initialize the lovely runtime.
    let rt = Lovely::init(
        &|a: *mut c_void, b, c, d, e| LuaLoadbufferx_Detour.call(a, b, c, d, e),
        dump_all,
    );
    RUNTIME
        .set(rt)
        .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));

    // Quick and easy hook injection. Load the lua51.dll module at runtime, determine the address of the luaL_loadbuffer fn, hook it.
    let handle = LoadLibraryW(w!("lua51.dll")).unwrap();
    let proc = GetProcAddress(handle, s!("luaL_loadbufferx")).unwrap();
    let fn_target = std::mem::transmute::<
        _,
        unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8, *const u8) -> u32,
    >(proc);

    LuaLoadbufferx_Detour
        .initialize(fn_target, |a, b, c, d, e| {
            lua_loadbufferx_detour(a, b, c, d, e)
        })
        .unwrap()
        .enable()
        .unwrap();

    1
}
