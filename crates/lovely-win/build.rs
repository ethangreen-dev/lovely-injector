use std::env;

fn main() {
    let system_drive = env::var("SYSTEMDRIVE").unwrap_or("C:".to_string());
    let windows_path = format!("{}\\Windows\\System32\\version.dll", system_drive);

    if let Ok(dev_path) = env::var("LOVELY_VERSION_PATH") {
        forward_dll::forward_dll_with_dev_path(&windows_path, &dev_path).unwrap();
    } else {
        #[cfg(target_os = "windows")]
        forward_dll::forward_dll(&windows_path).unwrap();
        #[cfg(not(target_os = "windows"))]
        forward_dll::forward_dll_with_dev_path(&windows_path, "../../version.dll").unwrap();
    }
}
