use std::path::Path;

/*
fn generate_binding() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let bindings = bindgen::builder().header("bind.h").generate().unwrap();

    bindings
        .write_to_file(Path::new(&out_dir).join("ffi.rs"))
        .unwrap();
}
*/

/*
fn link_dobby() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    let os_dir = match target_os.as_str() {
        "macos" => "darwin",
        _ => &target_os,
    };
    let arch_dir = match target_arch.as_str() {
        "arm" => "armv7",
        "aarch64" => "arm64",
        _ => &target_arch,
    };

    let lib_path = Path::new("crates/dobby-sys/dobby_static").join(os_dir).join(arch_dir);
    println!("cargo:warning=lib_path={}", lib_path.display());
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=dobby");
    match os_dir {
        "darwin" | "ios" => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => println!("cargo:rustc-link-lib=dylib=stdc++")
    }
}
*/

fn main() {
    let dst = cmake::Config::new("dobby")
        .define("CMAKE_OSX_DEPLOYMENT_TARGET", "11.0")
        .build_target("dobby_static")
        .build();
    let lib_path = dst.join("build");
    // generate_binding();
    println!("cargo:warning=lib_path={}", lib_path.display());
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=dobby");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "macos" | "ios" => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => println!("cargo:rustc-link-lib=dylib=stdc++")
    }
    // link_dobby();
}
