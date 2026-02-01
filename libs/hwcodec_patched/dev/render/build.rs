use cc::Build;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let externals_dir = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("externals");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed={}", externals_dir.display());
    let ffi_header = "src/render_ffi.h";
    bindgen::builder()
        .header(ffi_header)
        .rustified_enum("*")
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("render_ffi.rs"))
        .unwrap();

    let mut builder = Build::new();

    // system
    #[cfg(windows)]
    ["d3d11", "dxgi", "User32"].map(|lib| println!("cargo:rustc-link-lib={}", lib));
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=stdc++");

    #[cfg(windows)]
    {
        let sdl_dir = externals_dir.join("SDL");
        builder.include(sdl_dir.join("include"));
        let sdl_lib_path = sdl_dir.join("lib").join("x64");
        builder.file(manifest_dir.join("src").join("dxgi_sdl.cpp"));
        println!("cargo:rustc-link-search=native={}", sdl_lib_path.display());
        println!("cargo:rustc-link-lib=SDL2");
    }

    // crate
    builder
        .cpp(false)
        .static_crt(true)
        .warnings(false)
        .compile("render");
}
