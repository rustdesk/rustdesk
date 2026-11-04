#[cfg(windows)]
fn build_windows() {
    let file = "src/platform/windows.cc";
    let file2 = "src/platform/windows_delete_test_cert.cc";
    let mut build = cc::Build::new();
    if !cfg!(feature = "flutter") {
        build.cpp(true).std("c++17").define("RUSTDESK_SCITER", None);
        println!("cargo:rerun-if-changed=flutter/windows/runner/service_control.h");
    }
    build.file(file).file(file2).compile("windows");
    println!("cargo:rustc-link-lib=WtsApi32");
    println!("cargo:rerun-if-changed={}", file);
    println!("cargo:rerun-if-changed={}", file2);
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
    b.flag("-std=c++17").file(file).compile("macos");
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

// bionic only exports getifaddrs()/freeifaddrs() from API 24, while the jniLibs
// are built against the API 21 sysroot (flutter/ndk_*.sh). webrtc-util calls
// them, so without this the android link fails on undefined symbols.
fn build_android_ifaddrs() {
    let file = "src/platform/android_ifaddrs.c";
    cc::Build::new().file(file).compile("android_ifaddrs");
    println!("cargo:rerun-if-changed={}", file);
}

fn install_android_deps() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os != "android" {
        return;
    }
    let mut target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if target_arch == "x86_64" {
        target_arch = "x64".to_owned();
    } else if target_arch == "x86" {
        target_arch = "x86".to_owned();
    } else if target_arch == "aarch64" {
        target_arch = "arm64".to_owned();
    } else {
        target_arch = "arm".to_owned();
    }
    let target = format!("{}-android", target_arch);
    let vcpkg_root = std::env::var("VCPKG_ROOT").unwrap();
    let mut path: std::path::PathBuf = vcpkg_root.into();
    if let Ok(vcpkg_root) = std::env::var("VCPKG_INSTALLED_ROOT") {
        path = vcpkg_root.into();
    } else {
        path.push("installed");
    }
    path.push(target);
    println!(
        "cargo:rustc-link-search={}",
        path.join("lib").to_str().unwrap()
    );
    println!("cargo:rustc-link-lib=ndk_compat");
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=OpenSLES");
}

/// Known Tefway distribution profiles. Anything else is a configuration error:
/// a typo must never silently ship a half-configured or mislabeled product.
const TEFWAY_PROFILES: &[&str] = &["master", "client"];

/// Bakes the Tefway product into the binary. An unset profile keeps stock
/// RustDesk behaviour for ordinary development builds; a set but unknown
/// profile aborts the build instead of falling back to stock. When a Tefway
/// profile is selected, a verifiable build manifest is written next to the
/// artifact so the packager can confirm which profile was really embedded.
fn build_profile() {
    let requested = std::env::var("TEFWAY_PROFILE")
        .unwrap_or_default()
        .to_lowercase();
    let profile = match requested.as_str() {
        "master" => "master",
        "client" => "client",
        other => {
            if !other.is_empty() {
                panic!(
                    "unknown TEFWAY_PROFILE={:?}; expected one of {:?} for a Tefway distribution build",
                    other, TEFWAY_PROFILES
                );
            }
            ""
        }
    };
    println!("cargo:rustc-env=TEFWAY_PROFILE={}", profile);
    println!("cargo:rerun-if-env-changed=TEFWAY_PROFILE");
    if !profile.is_empty() {
        write_build_manifest(profile);
    }
}

/// Commit the source tree was built from, for the build manifest. Best effort:
/// a distribution build should be reproducible from a known commit, but the
/// build must not fail because git is unavailable in an odd checkout.
fn git_commit() -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_owned(),
        _ => "unknown".to_owned(),
    }
}

/// Writes `target/release/tefway-build-manifest.json` describing what this
/// build embeds: profile, version, source commit, OS, architecture, compiled
/// resources (cargo features) and the server identity the build is pinned to.
/// No secrets are included: the public key is handed to every client.
fn write_build_manifest(profile: &str) {
    use std::path::PathBuf;
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target"));
    let manifest_path = target_dir.join("release").join("tefway-build-manifest.json");
    if let Some(parent) = manifest_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut features: Vec<String> = std::env::vars()
        .filter_map(|(key, value)| {
            if value == "1" {
                key.strip_prefix("CARGO_FEATURE_").map(|s| s.to_owned())
            } else {
                None
            }
        })
        .collect();
    features.sort_unstable();
    let features_list = features
        .iter()
        .map(|f| format!("\"{}\"", f))
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!(
        "{{\n  \"profile\": \"{}\",\n  \"version\": \"{}\",\n  \"commit\": \"{}\",\n  \"os\": \"{}\",\n  \"arch\": \"{}\",\n  \"resources\": [{}]\n,\n  \"server\": {{\n    \"rendezvous\": \"remote.tefway.com.br\",\n    \"relay\": \"remote.tefway.com.br\",\n    \"public_key\": \"3KBIfqldtiCmJkAl2O6rammBCLK4iRiTH3vQdRXoEEs=\"\n  }}\n}}\n",
        profile,
        std::env::var("CARGO_PKG_VERSION").unwrap_or_default(),
        git_commit(),
        std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
        features_list,
    );
    match std::fs::write(&manifest_path, manifest) {
        Ok(()) => println!("cargo:info=tefway build manifest written to {}", manifest_path.display()),
        Err(err) => println!("cargo:warning=failed to write tefway build manifest: {err}"),
    }
}

fn main() {
    hbb_common::gen_version();
    build_profile();
    install_android_deps();
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
    if target_os == "android" {
        build_android_ifaddrs();
    }
    println!("cargo:rerun-if-changed=build.rs");
}
