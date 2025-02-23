use std::ffi::c_void;

use windows::core::w;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::GetFileVersionInfoSizeW;
use windows::Win32::Storage::FileSystem::GetFileVersionInfoW;
use windows::Win32::Storage::FileSystem::VerQueryValueW;
use windows::Win32::Storage::FileSystem::VS_FIXEDFILEINFO;

pub fn get_version(dll_path: &str) -> Result<VS_FIXEDFILEINFO, Box<dyn std::error::Error>> {
    unsafe {
        // Encode `dll_path` as UTF-16, which is a subset of UCS2, which is what
        // win32 wants.
        let dll_path_wcs: Vec<_> = dll_path.encode_utf16().chain(std::iter::once(0)).collect();
        let dll_path_pwcs = PCWSTR::from_raw(dll_path_wcs.as_ptr());

        // Get file version info size.
        let data_len = GetFileVersionInfoSizeW(dll_path_pwcs, None);
        if data_len == 0 {
            // Don't forget to check for errors!
            return Err(windows::core::Error::from_win32())?;
        }

        // Win32 returns a `u32`, Rust wants a `usize`; do the conversion and
        // make sure it's valid.
        let data_len_usize: usize = data_len.try_into().unwrap();

        /*
        Allocate buffer.

        NOTE THE USE OF `mut`!  You must not ever, EVER mutate something you've
        told the compiler won't be mutated.

        Ever.

        No exceptions.
        */
        let mut data = vec![0u8; data_len_usize];

        /*
        Doing this makes it *slightly* harder to mess up: I can no longer
        resize `data`, which helps prevent making pointers into it invalid.
        */
        let data = &mut data[..];

        // Get the info.
        let ok = GetFileVersionInfoW(
            dll_path_pwcs,
            0,
            data_len,
            // NOTE: I used `as_mut_ptr` here because I want a mutable pointer.
            data.as_mut_ptr() as *mut c_void,
        );
        ok.unwrap_or_else(|err| panic!("Could not get info from dll"));

        // Again, DO NOT EVER MAKE A MUTABLE POINTER TO NON-MUTABLE DATA!
        let mut info_ptr: *mut VS_FIXEDFILEINFO = std::ptr::null_mut();
        let mut info_len: u32 = 0;

        let ok = VerQueryValueW(
            data.as_ptr() as *const c_void,
            w!(r"\"),
            (&mut info_ptr) as *mut _ as *mut *mut c_void,
            &mut info_len,
        );
        ok.ok()?;

        // Read the info from the buffer.
        assert!(!info_ptr.is_null());
        assert_eq!(info_len as usize, std::mem::size_of::<VS_FIXEDFILEINFO>());
        let info = info_ptr.read_unaligned();

        Ok(info)
    }
}