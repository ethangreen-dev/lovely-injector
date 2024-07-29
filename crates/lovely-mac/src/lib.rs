use lovely_core::sys::LuaState;

use lovely_core::Lovely;
use once_cell::sync::{Lazy, OnceCell};

static RUNTIME: OnceCell<Lovely> = OnceCell::new();

static RECALL: Lazy<unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32> = Lazy::new(|| unsafe {
    let handle = libc::dlopen(b"../Frameworks/Lua.framework/Versions/A/Lua\0".as_ptr() as _, libc::RTLD_LAZY);
    
    if handle.is_null() {
        panic!("Failed to load lua");
    }
    let ptr = libc::dlsym(handle, b"luaL_loadbufferx\0".as_ptr() as _);
    
    if ptr.is_null() {
        panic!("Failed to load luaL_loadbufferx");
    }
    std::mem::transmute::<_, unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32>(ptr)
    
});

#[no_mangle]
unsafe extern "C" fn luaL_loadbufferx(state: *mut LuaState, buf_ptr: *const u8, size: isize, name_ptr: *const u8, mode_ptr: *const u8) -> u32 {
    let rt = RUNTIME.get_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, mode_ptr)
}

#[ctor::ctor]
unsafe fn construct() {
    let rt = Lovely::init(&|a, b, c, d,e| RECALL(a, b, c, d,e));
    RUNTIME.set(rt).unwrap_or_else(|_| panic!("Failed to instantiate runtime."));
}
