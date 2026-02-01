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
    let ffi_header = "src/dxgi_ffi.h";
    bindgen::builder()
        .header(ffi_header)
        .rustified_enum("*")
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("capture_ffi.rs"))
        .unwrap();

    let mut builder = Build::new();

    // system
    #[cfg(windows)]
    ["d3d11", "dxgi"].map(|lib| println!("cargo:rustc-link-lib={}", lib));
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=stdc++");

    #[cfg(windows)]
    {
        // dxgi
        let dxgi_path = externals_dir.join("nvEncDXGIOutputDuplicationSample");
        builder.include(&dxgi_path);
        for f in vec!["DDAImpl.cpp"] {
            builder.file(format!("{}/{}", dxgi_path.display(), f));
        }
        builder.file("src/dxgi.cpp");
    }

    // crate
    builder
        .cpp(false)
        .static_crt(true)
        .warnings(false)
        .compile("capture");
}
