pub mod ffi;

#[cfg(test)]
mod tests {
    #[test]
    fn test_build_version() {
        unsafe {
            let ptr = super::ffi::DobbyBuildVersion();
            let s = std::ffi::CStr::from_ptr(ptr);
            println!("{}", s.to_string_lossy());
        }
    }
}
