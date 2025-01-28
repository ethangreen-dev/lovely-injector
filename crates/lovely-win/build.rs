fn main() {
    let windows_path = "%SystemDrive%\\Windows\\System32\\version.dll";
    if let Some(path) = std::option_env!("LOVELY_VERSION_PATH") {
        forward_dll::forward_dll_with_dev_path(windows_path, path).unwrap();
    } else {
        #[cfg(target_os = "windows")]
        forward_dll::forward_dll(windows_path).unwrap();
        #[cfg(not(target_os = "windows"))]
        forward_dll::forward_dll_with_dev_path(windows_path, "../../version.dll").unwrap();
    }
}
