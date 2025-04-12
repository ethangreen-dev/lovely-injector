use libc::c_char;
use libloading::{Library, Symbol};
use std::ffi::CStr;
use std::str;
use std::num::ParseIntError;
use once_cell::sync::Lazy;

pub static LUA_LIB: Lazy<Library> = Lazy::new(|| unsafe { Library::new("love.dll").unwrap() });

pub static LOVE_VERSION: Lazy<Symbol<unsafe extern "C" fn() -> *const c_char>> =
    Lazy::new(|| unsafe { LUA_LIB.get(b"love_version").unwrap() });

// Calls the love_version function from the love.dll
pub unsafe fn load_version() -> Result<u32, ParseIntError> {
    let version_result = LOVE_VERSION();
    let version_str = CStr::from_ptr(version_result).to_str().unwrap_or_else(|err| panic!("Failed to convert love_version result into a str"));

    parse_version(version_str)
}

// Takes the version string "major.minor" and turns it into a u32 where the first 16 bits are the major version and the last 16 are the minor
fn parse_version(version: &str) -> Result<u32, ParseIntError> {
    let parts: Vec<&str> = version.split('.').collect();

    let major = parts[0].parse::<u32>()?;
    let minor = parts[1].parse::<u32>()?;

    Ok((major << 16) | (minor & 0xFFFF))
}
