use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Re-run build script if Zig files change
    println!("cargo:rerun-if-changed=../parser/src/parser.zig");
    println!("cargo:rerun-if-changed=../parser/build.zig");
    println!("cargo:rerun-if-changed=../helper-windows/src/main.zig");
    println!("cargo:rerun-if-changed=../helper-windows/build.zig");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_dir = manifest_dir.parent().unwrap();

    // Locate Zig compiler
    let home = env::var("HOME").unwrap_or_default();
    let local_zig = format!("{}/.local/bin/zig", home);
    let zig_cmd = if std::path::Path::new(&local_zig).exists() {
        local_zig
    } else {
        "zig".to_string()
    };

    println!("cargo:warning=Using Zig compiler: {}", zig_cmd);

    // 1. Build parser (native static library)
    let parser_dir = workspace_dir.join("parser");
    let status_parser = Command::new(&zig_cmd)
        .args(&["build", "-Doptimize=ReleaseSafe"])
        .current_dir(&parser_dir)
        .status()
        .expect("Failed to execute zig build for parser");

    if !status_parser.success() {
        panic!("Zig build for parser failed");
    }

    // Tell cargo where to find the static library
    let lib_dir = parser_dir.join("zig-out").join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=grapevine_parser");

    // 2. Build helper-windows (cross-compiled guest helper exe)
    let helper_dir = workspace_dir.join("helper-windows");
    let status_helper = Command::new(&zig_cmd)
        .args(&["build", "-Dtarget=x86_64-windows", "-Doptimize=ReleaseSafe"])
        .current_dir(&helper_dir)
        .status()
        .expect("Failed to execute zig build for helper-windows");

    if !status_helper.success() {
        panic!("Zig build for helper-windows failed");
    }

    // Copy grapevine-helper.exe to OUT_DIR so Rust code can include_bytes! it
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let helper_exe_src = helper_dir.join("zig-out").join("bin").join("grapevine-helper.exe");
    let helper_exe_dst = out_dir.join("grapevine-helper.exe");

    std::fs::copy(&helper_exe_src, &helper_exe_dst)
        .expect("Failed to copy grapevine-helper.exe to OUT_DIR");
}
