use std::sync::LazyLock;

use libloading::Library;

use lovely_core::sys::LuaLib;

pub static LUA_LIBRARY: LazyLock<Library> = LazyLock::new(|| unsafe {
    return Library::new("liblove.so").unwrap();
});

pub unsafe fn get_lualib() -> LuaLib {
    LuaLib::from_library(&LUA_LIBRARY)
}
