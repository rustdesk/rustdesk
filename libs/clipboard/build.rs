use cc;

fn build_c_impl() {
    let mut build = cc::Build::new();

    #[cfg(target_os = "windows")]
    build.file("src/windows/wf_cliprdr.c");
    #[cfg(target_os = "linux")]
    build.file("src/X11/xf_cliprdr.c");
    #[cfg(target_os = "macos")]
    build.file("src/OSX/Clipboard.m");

    build.flag_if_supported("-Wno-c++0x-extensions");
    build.flag_if_supported("-Wno-return-type-c-linkage");
    build.flag_if_supported("-Wno-invalid-offsetof");
    build.flag_if_supported("-Wno-unused-parameter");

    if build.get_compiler().is_like_msvc() {
        build.define("WIN32", "");
        // build.define("_AMD64_", "");
        build.flag("-Z7");
        build.flag("-GR-");
        // build.flag("-std:c++11");
    } else {
        build.flag("-fPIC");
        // build.flag("-std=c++11");
        // build.flag("-include");
        // build.flag(&confdefs_path.to_string_lossy());
    }

    build.compile("mycliprdr");

    #[cfg(target_os = "windows")]
    println!("cargo:rerun-if-changed=src/windows/wf_cliprdr.c");
    #[cfg(target_os = "linux")]
    println!("cargo:rerun-if-changed=src/X11/xf_cliprdr.c");
    #[cfg(target_os = "macos")]
    println!("cargo:rerun-if-changed=src/OSX/Clipboard.m");
}

fn main() {
    build_c_impl();
}
