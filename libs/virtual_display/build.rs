fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let file = "src/macos/virtual_display.mm";
    cc::Build::new()
        .cpp(true)
        .flag("-std=c++17")
        .flag("-fobjc-arc")
        .file(file)
        .compile("macos_virtual_display");
    println!("cargo:rerun-if-changed={file}");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
}
