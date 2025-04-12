use std::env;
use std::ffi::c_void;
use std::panic;
use std::ptr::null;

use itertools::Itertools;
use lovely_core::log::*;
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

pub mod util;

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static_detour! {
    pub static LuaLoadbufferx_Detour: unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8,*const u8) -> u32;
    pub static LuaLoadbuffer_Detour: unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8 ) -> u32;
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

unsafe extern "C" fn lua_loadbuffer_detour(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: isize,
    name_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, null())
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

    let args = env::args().collect_vec();

    if args.contains(&"--disable-mods".to_string()) || args.contains(&"-d".to_string()) {
        return 1;
    }

    if !args.contains(&"--disable-console".to_string()) {
        let _ = AllocConsole();
        SetConsoleTitleW(PCWSTR(WIN_TITLE.as_ptr())).expect("Failed to set console title.");
    }

    let dump_all = args.contains(&"--dump-all".to_string());

    let love_version_value = util::load_version().unwrap_or_else(|err| panic!("{err}"));

    // Quick and easy hook injection. Load the lua51.dll module at runtime, determine the address of the luaL_loadbuffer fn, hook it.
    let handle =
        LoadLibraryW(w!("lua51.dll")).unwrap_or_else(|err| panic!("Could not find love.dll"));

    const LOVE_11_4: u32 = (11 << 16) + 4;
    if love_version_value > LOVE_11_4 {
        // Detour with loadbuferx

        // Initialize the lovely runtime.
        let rt = Lovely::init(
            &|a, b, c, d, e| LuaLoadbufferx_Detour.call(a, b, c, d, e),
            dump_all,
        );
        RUNTIME
            .set(rt)
            .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));

        let proc = GetProcAddress(handle, s!("luaL_loadbufferx"));

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
    } else {
        // Detour with loadbufer

        // Initialize the lovely runtime.
        let rt = Lovely::init(
            &|a, b, c, d, e| LuaLoadbuffer_Detour.call(a, b, c, d),
            dump_all,
        );
        RUNTIME
            .set(rt)
            .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));

        let proc = GetProcAddress(handle, s!("luaL_loadbuffer"))
            .unwrap_or_else(|| panic!("func not found"));

        let fn_target = std::mem::transmute::<
            _,
            unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8) -> u32,
        >(proc);

        LuaLoadbuffer_Detour
            .initialize(fn_target, |a, b, c, d| lua_loadbuffer_detour(a, b, c, d))
            .unwrap()
            .enable()
            .unwrap();
    }

    1
}
