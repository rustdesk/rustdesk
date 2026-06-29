use std::{
    env, fs,
    path::{Path, PathBuf},
    println,
};

#[cfg(all(target_os = "linux", feature = "linux-pkg-config"))]
fn link_pkg_config(name: &str) -> Vec<PathBuf> {
    let pc_name = match name {
        "libvpx" => "vpx",
        _ => name,
    };

    let lib = pkg_config::probe_library(pc_name).expect(
        format!(
            "unable to find '{pc_name}' development headers with pkg-config \
             (feature linux-pkg-config is enabled). try installing \
             '{pc_name}-dev' from your system package manager."
        )
        .as_str(),
    );

    lib.include_paths
}

#[cfg(not(all(target_os = "linux", feature = "linux-pkg-config")))]
fn link_pkg_config(_name: &str) -> Vec<PathBuf> {
    unimplemented!()
}

fn link_vcpkg(mut path: PathBuf, name: &str) -> PathBuf {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let mut target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    target_arch = match target_arch.as_str() {
        "x86_64" => "x64".to_owned(),
        "x86" => "x86".to_owned(),
        "loongarch64" => "loongarch64".to_owned(),
        "aarch64" => "arm64".to_owned(),
        _ => "arm".to_owned(),
    };

    let mut target = if target_os == "macos" {
        match target_arch.as_str() {
            "x64" => "x64-osx".to_owned(),
            "arm64" => "arm64-osx".to_owned(),
            _ => format!("{}-{}", target_arch, target_os),
        }
    } else if target_os == "windows" {
        format!("{}-windows-static", target_arch)
    } else {
        format!("{}-{}", target_arch, target_os)
    };

    if target_arch == "x86" {
        target = target.replace("x64", "x86");
    }

    println!("cargo:warning=vcpkg triplet: {}", target);

    if let Ok(vcpkg_root) = env::var("VCPKG_INSTALLED_ROOT") {
        path = vcpkg_root.into();
    } else {
        path.push("installed");
    }

    path.push(target);

    let lib_dir = path.join("lib");
    let include = path.join("include");

    println!(
        "cargo:rustc-link-lib=static={}",
        name.trim_start_matches("lib")
    );
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:include={}", include.display());

    include
}

fn link_homebrew_m1(name: &str) -> PathBuf {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    if target_os != "macos" || target_arch != "aarch64" {
        panic!(
            "Couldn't find VCPKG_ROOT, also can't fallback to homebrew \
             because it's only for macos aarch64."
        );
    }

    let mut path = PathBuf::from("/opt/homebrew/Cellar");
    path.push(name);

    let entries = fs::read_dir(&path).unwrap_or_else(|_| {
        panic!(
            "Could not find package in {}. Make sure homebrew package {} is installed.",
            path.display(),
            name
        )
    });

    let mut directories = entries
        .filter_map(Result::ok)
        .map(|x| x.path())
        .filter(|x| x.is_dir())
        .collect::<Vec<_>>();

    directories.sort_unstable();

    if directories.is_empty() {
        panic!("There's no installed version of {} in /opt/homebrew/Cellar", name);
    }

    path.push(directories.pop().unwrap());

    println!(
        "cargo:rustc-link-lib=static={}",
        name.trim_start_matches("lib")
    );
    println!(
        "cargo:rustc-link-search=native={}",
        path.join("lib").display()
    );

    let include = path.join("include");
    println!("cargo:include={}", include.display());

    include
}

fn find_package(name: &str) -> Vec<PathBuf> {
    let no_pkg_config_var_name = format!("NO_PKG_CONFIG_{name}");
    println!("cargo:rerun-if-env-changed={no_pkg_config_var_name}");

    if cfg!(all(target_os = "linux", feature = "linux-pkg-config"))
        && env::var(no_pkg_config_var_name).as_deref() != Ok("1")
    {
        link_pkg_config(name)
    } else if let Ok(vcpkg_root) = env::var("VCPKG_ROOT") {
        vec![link_vcpkg(vcpkg_root.into(), name)]
    } else {
        vec![link_homebrew_m1(name)]
    }
}

fn generate_bindings(
    ffi_header: &Path,
    include_paths: &[PathBuf],
    ffi_rs: &Path,
    exact_file: &Path,
    regex: &str,
) {
    let mut b = bindgen::builder()
        .header(ffi_header.to_str().unwrap())
        .allowlist_type(regex)
        .allowlist_var(regex)
        .allowlist_function(regex)
        .rustified_enum(regex)
        .trust_clang_mangling(false)
        .layout_tests(false)
        .generate_comments(false)
        .clang_arg("-DVPX_CODEC_USE_ENCODER=1")
        .clang_arg("-DVPX_CODEC_USE_DECODER=1")
        .clang_arg("-DAOM_CODEC_USE_ENCODER=1")
        .clang_arg("-DAOM_CODEC_USE_DECODER=1");

    for dir in include_paths {
        b = b.clang_arg(format!("-I{}", dir.display()));
    }

    if let Some(parent) = ffi_rs.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    if let Some(parent) = exact_file.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    let bindings = b.generate().unwrap_or_else(|err| {
        panic!(
            "bindgen failed for header: {}\ninclude paths: {:#?}\nregex: {}\nerror: {:?}",
            ffi_header.display(),
            include_paths,
            regex,
            err
        )
    });

    bindings.write_to_file(ffi_rs).unwrap_or_else(|err| {
        panic!("failed to write bindings to {}: {:?}", ffi_rs.display(), err)
    });

    fs::copy(ffi_rs, exact_file).unwrap_or_else(|err| {
        panic!(
            "failed to copy bindings from {} to {}: {:?}",
            ffi_rs.display(),
            exact_file.display(),
            err
        )
    });

    println!("cargo:warning=generated bindings: {}", ffi_rs.display());
    println!("cargo:warning=copied bindings: {}", exact_file.display());
}

fn gen_vcpkg_package(package: &str, ffi_header: &str, generated: &str, regex: &str) {
    let includes = find_package(package);

    let src_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = Path::new(&src_dir);

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    let ffi_header = src_dir.join("src").join("bindings").join(ffi_header);

    if !ffi_header.exists() {
        panic!("FFI header does not exist: {}", ffi_header.display());
    }

    println!("cargo:rerun-if-changed={}", ffi_header.display());

    for dir in &includes {
        println!("cargo:rerun-if-changed={}", dir.display());
    }

    let ffi_rs = out_dir.join(generated);
    let exact_file = src_dir.join("generated").join(generated);

    generate_bindings(&ffi_header, &includes, &ffi_rs, &exact_file, regex);
}

fn build_codec_cfg_shim() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    let mut include_paths = Vec::new();
    include_paths.extend(find_package("aom"));
    include_paths.extend(find_package("libvpx"));

    let mut build = cc::Build::new();
    build.file(
        manifest_dir
            .join("src")
            .join("bindings")
            .join("codec_cfg_shim.c"),
    );

    for include in include_paths {
        build.include(include);
    }

    build.compile("codec_cfg_shim");
}

fn main() {
    println!("cargo:rustc-check-cfg=cfg(dxgi,quartz,x11)");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    let target = target_build_utils::TargetInfo::new();
    if target.unwrap().target_pointer_width() != "64" {
        // panic!("Only support 64bit system");
    }

    env::remove_var("CARGO_CFG_TARGET_FEATURE");
    env::set_var("CARGO_CFG_TARGET_FEATURE", "crt-static");

    find_package("libyuv");

    gen_vcpkg_package("libvpx", "vpx_ffi.h", "vpx_ffi.rs", "^[vV].*");
    gen_vcpkg_package("aom", "aom_ffi.h", "aom_ffi.rs", "^(aom|AOM|OBU|AV1).*");
    gen_vcpkg_package("libyuv", "yuv_ffi.h", "yuv_ffi.rs", ".*");

    build_codec_cfg_shim();

    if target_os == "ios" {
        // nothing
    } else if target_os == "android" {
        println!("cargo:rustc-cfg=android");
    } else if cfg!(windows) {
        println!("cargo:rustc-cfg=dxgi");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-cfg=quartz");
    } else if cfg!(unix) {
        println!("cargo:rustc-cfg=x11");
    }
}