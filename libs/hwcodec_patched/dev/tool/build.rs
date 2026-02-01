use cc::Build;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    println!("cargo:rerun-if-changed=src");
    let ffi_header = "src/tool_ffi.h";
    bindgen::builder()
        .header(ffi_header)
        .rustified_enum("*")
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("tool_ffi.rs"))
        .unwrap();

    let mut builder = Build::new();

    builder.include(
        manifest_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("cpp")
            .join("common"),
    );

    builder.file("src/tool.cpp");

    // crate
    builder
        .cpp(false)
        .static_crt(true)
        .warnings(false)
        .compile("tool");
}
