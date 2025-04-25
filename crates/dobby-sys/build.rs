use std::path::PathBuf;
use std::process::Command;

fn build_android() {
    let workspace = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ndk = PathBuf::from(std::env::var("ANDROID_NDK_HOME").unwrap());
    let toolchain = ndk.join("toolchains/llvm/prebuilt");

    let dirs: Vec<_> = std::fs::read_dir(&toolchain)
        .unwrap().take(1).map(|r| r.unwrap().file_name()).collect();
    let llvm_dir = toolchain.join(dirs[0].to_string_lossy().as_ref());

    let _ = Command::new("python3")
        .arg(workspace.join("dobby/scripts/platform_builder.py"))
        .arg("--arch=arm64-v8a")
        .arg("--platform=android")
        .arg(format!("--android_ndk_dir={}", ndk.display()))
        .arg(format!("--llvm_dir={}", llvm_dir.display()))
        .spawn().unwrap().wait().unwrap();

    println!("cargo:rustc-link-search=native={}", workspace.join("dobby/build/android/arm64-v8a").display());
    println!("cargo:rustc-link-lib=dylib=dobby");
}

fn build_desktop() {
    let dst = cmake::Config::new("dobby")
        .define("CMAKE_OSX_DEPLOYMENT_TARGET", "11.0")
        .build_target("dobby_static")
        .build();
    let lib_path = dst.join("build");
    println!("cargo:warning=lib_path={}", lib_path.display());
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=dobby");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "macos" | "ios" => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => println!("cargo:rustc-link-lib=dylib=stdc++")
    }
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "android" {
        build_android();
    } else {
        build_desktop();
    }
}
