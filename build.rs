#[cfg(windows)]
fn build_windows() {
    let file = "src/platform/windows.cc";
    cc::Build::new().file(file).compile("windows");
    println!("cargo:rustc-link-lib=WtsApi32");
    println!("cargo:rerun-if-changed={}", file);
}

#[cfg(target_os = "macos")]
fn build_mac() {
    let file = "src/platform/macos.mm";
    let mut b = cc::Build::new();
    if let Ok(os_version::OsVersion::MacOS(v)) = os_version::detect() {
        let v = v.version;
        if v.contains("10.14") {
            b.flag("-DNO_InputMonitoringAuthStatus=1");
        }
    }
    b.file(file).compile("macos");
    println!("cargo:rerun-if-changed={}", file);
}

#[cfg(all(windows, feature = "inline"))]
fn build_manifest() {
    use std::io::Write;
    if std::env::var("PROFILE").unwrap() == "release" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("res/icon.ico")
            .set_language(winapi::um::winnt::MAKELANGID(
                winapi::um::winnt::LANG_ENGLISH,
                winapi::um::winnt::SUBLANG_ENGLISH_US,
            ))
            .set_manifest_file("res/manifest.xml");
        match res.compile() {
            Err(e) => {
                write!(std::io::stderr(), "{}", e).unwrap();
                std::process::exit(1);
            }
            Ok(_) => {}
        }
    }
}

fn install_oboe() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os != "android" {
        return;
    }
    let mut target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if target_arch == "x86_64" {
        target_arch = "x64".to_owned();
    } else if target_arch == "aarch64" {
        target_arch = "arm64".to_owned();
    } else {
        target_arch = "arm".to_owned();
    }
    let target = format!("{}-android", target_arch);
    let vcpkg_root = std::env::var("VCPKG_ROOT").unwrap();
    let mut path: std::path::PathBuf = vcpkg_root.into();
    path.push("installed");
    path.push(target);
    println!(
        "{}",
        format!(
            "cargo:rustc-link-search={}",
            path.join("lib").to_str().unwrap()
        )
    );
    println!("cargo:rustc-link-lib=oboe");
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=OpenSLES");
    // I always got some strange link error with oboe, so as workaround, put oboe.cc into oboe src: src/common/AudioStreamBuilder.cpp
    // also to avoid libc++_shared not found issue, cp ndk's libc++_shared.so to jniLibs, e.g.
    // ./flutter_hbb/android/app/src/main/jniLibs/arm64-v8a/libc++_shared.so
    // let include = path.join("include");
    //cc::Build::new().file("oboe.cc").include(include).compile("oboe_wrapper");
}

#[cfg(feature = "flutter")]
fn gen_flutter_rust_bridge() {
    if !std::env::var("RUN_FFIGEN").is_ok() {
        return;
    }
    use lib_flutter_rust_bridge_codegen::{
        config_parse, frb_codegen, get_symbols_if_no_duplicates, RawOpts,
    };
    let llvm_path = match std::env::var("LLVM_HOME") {
        Ok(path) => Some(vec![path]),
        Err(_) => None,
    };
    // Tell Cargo that if the given file changes, to rerun this build script.
    println!("cargo:rerun-if-changed=src/flutter_ffi.rs");
    // Options for frb_codegen
    let raw_opts = RawOpts {
        // Path of input Rust code
        rust_input: vec!["src/flutter_ffi.rs".to_string()],
        // Path of output generated Dart code
        dart_output: vec!["flutter/lib/generated_bridge.dart".to_string()],
        // Path of output generated C header
        c_output: Some(vec!["flutter/macos/Runner/bridge_generated.h".to_string()]),
        /// Path to the installed LLVM
        llvm_path,
        // for other options use defaults
        ..Default::default()
    };
    // get opts from raw opts
    let configs = config_parse(raw_opts);
    // generation of rust api for ffi
    let all_symbols = get_symbols_if_no_duplicates(&configs).unwrap();
    for config in configs.iter() {
        frb_codegen(config, &all_symbols).unwrap();
    }
}

fn main() {
    hbb_common::gen_version();
    install_oboe();
    // there is problem with cfg(target_os) in build.rs, so use our workaround
    // let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    // if target_os == "android" || target_os == "ios" {
    #[cfg(feature = "flutter")]
    gen_flutter_rust_bridge();
    //     return;
    // }
    #[cfg(all(windows, feature = "inline"))]
    build_manifest();
    #[cfg(windows)]
    build_windows();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "macos" {
        #[cfg(target_os = "macos")]
        build_mac();
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
