use std::ffi::CStr;
use std::num::ParseIntError;
use std::str;

use windows::core::s;
use windows::core::w;
use windows::Win32::System::LibraryLoader::GetProcAddress;
use windows::Win32::System::LibraryLoader::LoadLibraryW;

// Calls the love_version function from the love.dll
pub unsafe fn load_version() -> Result<u32, ParseIntError> {
    let handle = LoadLibraryW(w!("love.dll")).unwrap_or_else(|err| panic!("Could not find dll"));

    let proc = GetProcAddress(handle, s!("love_version"));
    let fn_target = std::mem::transmute::<
        _,
        unsafe extern "C" fn() -> *const i8
    >(proc);

    let version = fn_target();
    let version_str = CStr::from_ptr(version).to_str().unwrap_or_else(|err| panic!("Failed to convert love_version result into a str"));

    parse_version(version_str)
}

// Takes the version string "major.minor" and turns it into a u32 where the first 16 bits are the major version and the last 16 are the minor
fn parse_version(version: &str) -> Result<u32, ParseIntError>{
    let parts: Vec<&str> = version.split('.').collect();

    let major = parts[0].parse::<u32>()?;
    let minor = parts[1].parse::<u32>()?;
    
    Ok((major << 16) | (minor & 0xFFFF))
}