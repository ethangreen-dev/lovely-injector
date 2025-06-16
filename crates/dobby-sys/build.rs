
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let mut binding = cmake::Config::new("dobby");
    let build = binding
        .define("CMAKE_OSX_DEPLOYMENT_TARGET", "11.0");
        
    if target_os == "android" {
        build
            .define("CMAKE_ANDROID_NDK", std::env::var("ANDROID_NDK_HOME").unwrap())
            .define("ANDROID_USE_LEGACY_TOOLCHAIN_FILE", "OFF");
    }
        
    let dst = build
        .build_target("dobby_static")
        .build();
    let lib_path = dst.join("build");
    println!("cargo:warning=lib_path={}", lib_path.display());
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=dobby");
    match target_os.as_str() {
        "macos" | "ios" => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => println!("cargo:rustc-link-lib=dylib=stdc++")
    }
}
