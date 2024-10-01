#[cfg(windows)]
fn build_windows() {
    let files = ["src/platform/windows.cc", "src/platform/windows_delete_test_cert.cc"];
    let mut builder = cc::Build::new();
    
    for &file in &files {
        builder.file(file);
        println!("cargo:rerun-if-changed={}", file);
    }
    
    builder.compile("windows");
    println!("cargo:rustc-link-lib=WtsApi32");
}

#[cfg(target_os = "macos")]
fn build_mac() {
    let file = "src/platform/macos.mm";
    let mut builder = cc::Build::new();

    if let Ok(os_version::OsVersion::MacOS(v)) = os_version::detect() {
        if v.version.contains("10.14") {
            builder.flag("-DNO_InputMonitoringAuthStatus=1");
        }
    }
    
    builder.file(file).compile("macos");
    println!("cargo:rerun-if-changed={}", file);
}

#[cfg(all(windows, feature = "inline"))]
fn build_manifest() {
    use std::io::{self, Write};

    if std::env::var("PROFILE").unwrap_or_default() == "release" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("res/icon.ico")
            .set_language(winapi::um::winnt::MAKELANGID(
                winapi::um::winnt::LANG_ENGLISH,
                winapi::um::winnt::SUBLANG_ENGLISH_US,
            ))
            .set_manifest_file("res/manifest.xml");

        if let Err(e) = res.compile() {
            writeln!(io::stderr(), "{}", e).unwrap();
            std::process::exit(1);
        }
    }
}

fn install_android_deps() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "android" {
        println!("Not building for Android, skipping dependencies installation.");
        return;
    }

    let target_arch = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default().as_str() {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        _ => "arm",
    }.to_owned();

    let target = format!("{}-android", target_arch);
    let vcpkg_root = std::env::var("VCPKG_ROOT").expect("VCPKG_ROOT not set");
    let path = std::path::Path::new(&vcpkg_root).join("installed").join(&target).join("lib");

    println!("cargo:rustc-link-search={}", path.display());
    println!("cargo:rustc-link-lib=ndk_compat");
    println!("cargo:rustc-link-lib=oboe");
    println!("cargo:rustc-link-lib=oboe_wrapper");
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=OpenSLES");
}

fn main() {
    hbb_common::gen_version();
    install_android_deps();
    
    #[cfg(all(windows, feature = "inline"))]
    build_manifest();
    
    #[cfg(windows)]
    build_windows();
    
    #[cfg(target_os = "macos")]
    {
        build_mac();
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
