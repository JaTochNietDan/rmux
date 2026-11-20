use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=GHOSTTY_VT_DIR");
    let target = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let dir = env::var_os("GHOSTTY_VT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if target == "macos" {
                root.join(".deps/ghostty-vt.xcframework/macos-arm64_x86_64")
            } else {
                root.join(".deps/ghostty-vt")
            }
        });
    let include = if dir.join("Headers").exists() {
        dir.join("Headers")
    } else {
        dir.join("include")
    };
    let lib = if dir.join("lib").exists() {
        dir.join("lib")
    } else {
        dir.clone()
    };
    let header = include.join("ghostty/vt.h");
    assert!(
        header.exists(),
        "Ghostty VT headers missing. Run mise run bootstrap (bootstrap-windows on Windows)"
    );
    let archive = lib.join(if target == "windows" {
        "ghostty-vt.lib"
    } else {
        "libghostty-vt.a"
    });
    assert!(
        archive.exists(),
        "Ghostty VT library missing: {}",
        archive.display()
    );
    println!("cargo:rerun-if-changed={}", include.display());
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rustc-link-search=native={}", lib.display());
    if target == "windows" {
        println!("cargo:rustc-link-lib=dylib=ghostty-vt");
    } else {
        println!("cargo:rustc-link-lib=static=ghostty-vt");
    }
    let mut builder = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", include.display()));
    if target != "windows" {
        builder = builder.clang_arg("-DGHOSTTY_STATIC");
    }
    builder
        .allowlist_function("ghostty_.*")
        .allowlist_type("Ghostty.*")
        .allowlist_var("GHOSTTY_.*")
        .prepend_enum_name(false)
        .derive_default(true)
        .layout_tests(false)
        .generate_comments(false)
        .generate()
        .expect("Ghostty VT bindings")
        .write_to_file(PathBuf::from(env::var("OUT_DIR").unwrap()).join("ghostty.rs"))
        .unwrap();
}
