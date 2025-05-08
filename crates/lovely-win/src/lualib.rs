use libloading::Library;
use lovely_core::sys::LuaLib;


pub unsafe fn get_lualib() -> LuaLib {
    let library = Library::new("lua51.dll").unwrap();
    LuaLib::from_library(&library)
}
