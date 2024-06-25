use std::env;
use std::panic;
use std::ffi::c_void;

use lovely_core::log::*;
use lovely_core::Lovely;
use lovely_core::sys::LuaState;

use once_cell::sync::OnceCell;
use retour::static_detour;
use widestring::U16CString;
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MESSAGEBOX_STYLE};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static_detour! {
    pub static LuaLoadbuffer_Detour: unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8) -> u32;
}

unsafe extern "C" fn lua_loadbuffer_detour(state: *mut LuaState, buf_ptr: *const u8, size: isize, name_ptr: *const u8) -> u32 {
    let rt = RUNTIME.get_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr)
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
            w!("lovely-injector"),
            MESSAGEBOX_STYLE(0),
        );
    }));

    let args = env::args().collect_vec();
    if !args.contains(&"--disable-console".to_string()) { 
        let _ = AllocConsole();
    }

    // Initialize the lovely runtime.
    let rt = Lovely::init(&|a, b, c, d| LuaLoadbuffer_Detour.call(a, b, c, d));
    RUNTIME.set(rt).unwrap_or_else(|_| panic!("Failed to instantiate runtime."));
    
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
