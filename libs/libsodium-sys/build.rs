#[cfg(not(windows))]
extern crate cc;

#[cfg(target_env = "msvc")]
extern crate libc;

extern crate pkg_config;
extern crate walkdir;

use std::{
    env,
    path::{Path, PathBuf},
};

static VERSION: &str = "1.0.18";

fn main() {
    println!("cargo:rerun-if-env-changed=SODIUM_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SODIUM_SHARED");
    println!("cargo:rerun-if-env-changed=SODIUM_USE_PKG_CONFIG");

    if cfg!(not(windows)) {
        println!("cargo:rerun-if-env-changed=SODIUM_DISABLE_PIE");
    }

    if env::var("SODIUM_STATIC").is_ok() {
        panic!("SODIUM_STATIC is deprecated. Use SODIUM_SHARED instead.");
    }

    // Cross builds (e.g. a windows-msvc host targeting aarch64-unknown-linux-ohos)
    // scope the prebuilt lib dir per target so the host build keeps using its own
    // (vendored msvc) library.
    let scoped_lib_dir = format!(
        "SODIUM_LIB_DIR_{}",
        env::var("TARGET").unwrap().replace('-', "_")
    );
    if let Ok(lib_dir) = env::var(&scoped_lib_dir) {
        println!("cargo:rustc-link-search=native={}", lib_dir);
        println!("cargo:rustc-link-lib=static=sodium");
        return;
    }

    let lib_dir_isset = env::var("SODIUM_LIB_DIR").is_ok();
    let use_pkg_isset = if cfg!(feature = "use-pkg-config") {
        true
    } else {
        env::var("SODIUM_USE_PKG_CONFIG").is_ok()
    };
    let shared_isset = env::var("SODIUM_SHARED").is_ok();

    if lib_dir_isset && use_pkg_isset {
        panic!("SODIUM_LIB_DIR is incompatible with SODIUM_USE_PKG_CONFIG. Set the only one env variable");
    }

    if lib_dir_isset {
        find_libsodium_env();
    } else if use_pkg_isset {
        if shared_isset {
            println!("cargo:warning=SODIUM_SHARED has no effect with SODIUM_USE_PKG_CONFIG");
        }

        find_libsodium_pkg();
    } else {
        if shared_isset {
            println!(
                "cargo:warning=SODIUM_SHARED has no effect for building libsodium from source"
            );
        }

        build_libsodium();
    }
}

/* Must be called when SODIUM_LIB_DIR is set to any value
This function will set `cargo` flags.
*/
fn find_libsodium_env() {
    let lib_dir = env::var("SODIUM_LIB_DIR").unwrap(); // cannot fail

    println!("cargo:rustc-link-search=native={}", lib_dir);
    let mode = if env::var("SODIUM_SHARED").is_ok() {
        "dylib"
    } else {
        "static"
    };
    let name = if cfg!(target_env = "msvc") {
        "libsodium"
    } else {
        "sodium"
    };
    println!("cargo:rustc-link-lib={}={}", mode, name);
    println!(
        "cargo:warning=Using unknown libsodium version.  This crate is tested against \
         {} and may not be fully compatible with other versions.",
        VERSION
    );
}

/* Must be called when no SODIUM_USE_PKG_CONFIG env var is set
This function will set `cargo` flags.
*/
#[cfg(target_env = "msvc")]
fn find_libsodium_pkg() {
    panic!("SODIUM_USE_PKG_CONFIG is not supported on msvc");
}

/* Must be called when SODIUM_USE_PKG_CONFIG env var is set
This function will set `cargo` flags.
*/
#[cfg(not(target_env = "msvc"))]
fn find_libsodium_pkg() {
    match pkg_config::Config::new().probe("libsodium") {
        Ok(lib) => {
            if lib.version != VERSION {
                println!(
                    "cargo:warning=Using libsodium version {}.  This crate is tested against {} \
                     and may not be fully compatible with {}.",
                    lib.version, VERSION, lib.version
                );
            }
            for lib_dir in &lib.link_paths {
                println!("cargo:lib={}", lib_dir.to_str().unwrap());
            }
            for include_dir in &lib.include_paths {
                println!("cargo:include={}", include_dir.to_str().unwrap());
            }
        }
        Err(e) => {
            panic!(
                "
Failed to run pkg-config:
{:?}

You can try fixing this by installing pkg-config:

    # On Ubuntu
    sudo apt install pkg-config
    # On Arch Linux
    sudo pacman -S pkgconf
    # On Fedora
    sudo dnf install pkgconf-pkg-config

",
                e
            );
        }
    }
}

#[cfg(windows)]
fn make_libsodium(_: &str, _: &Path, _: &Path) -> PathBuf {
    // We don't build anything on windows, we simply linked to precompiled
    // libs.
    get_lib_dir()
}

#[cfg(not(windows))]
fn make_libsodium(target: &str, source_dir: &Path, install_dir: &Path) -> PathBuf {
    use std::{fs, process::Command, str};

    // Decide on CC, CFLAGS and the --host configure argument
    let build_compiler = cc::Build::new().get_compiler();
    let mut compiler = build_compiler.path().to_str().unwrap().to_string();
    let mut cflags = build_compiler.cflags_env().into_string().unwrap();
    let mut host_arg = format!("--host={}", target);
    let mut cross_compiling = target != env::var("HOST").unwrap();
    if target.contains("-ios") {
        // Determine Xcode directory path
        let xcode_select_output = Command::new("xcode-select").arg("-p").output().unwrap();
        if !xcode_select_output.status.success() {
            panic!("Failed to run xcode-select -p");
        }
        let xcode_dir = str::from_utf8(&xcode_select_output.stdout)
            .unwrap()
            .trim()
            .to_string();

        // Determine SDK directory paths
        let sdk_dir_simulator = Path::new(&xcode_dir)
            .join("Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk")
            .to_str()
            .unwrap()
            .to_string();
        let sdk_dir_ios = Path::new(&xcode_dir)
            .join("Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk")
            .to_str()
            .unwrap()
            .to_string();

        // Min versions
        let ios_simulator_version_min = "6.0.0";
        let ios_version_min = "6.0.0";

        // Roughly based on `dist-build/ios.sh` in the libsodium sources
        match &*target {
            "aarch64-apple-ios" => {
                cflags += " -arch arm64";
                cflags += &format!(" -isysroot {}", sdk_dir_ios);
                cflags += &format!(" -mios-version-min={}", ios_version_min);
                cflags += " -fembed-bitcode";
                host_arg = "--host=arm-apple-darwin10".to_string();
            }
            "armv7-apple-ios" => {
                cflags += " -arch armv7";
                cflags += &format!(" -isysroot {}", sdk_dir_ios);
                cflags += &format!(" -mios-version-min={}", ios_version_min);
                cflags += " -mthumb";
                host_arg = "--host=arm-apple-darwin10".to_string();
            }
            "armv7s-apple-ios" => {
                cflags += " -arch armv7s";
                cflags += &format!(" -isysroot {}", sdk_dir_ios);
                cflags += &format!(" -mios-version-min={}", ios_version_min);
                cflags += " -mthumb";
                host_arg = "--host=arm-apple-darwin10".to_string();
            }
            "i386-apple-ios" => {
                cflags += " -arch i386";
                cflags += &format!(" -isysroot {}", sdk_dir_simulator);
                cflags += &format!(" -mios-simulator-version-min={}", ios_simulator_version_min);
                host_arg = "--host=i686-apple-darwin10".to_string();
            }
            "x86_64-apple-ios" => {
                cflags += " -arch x86_64";
                cflags += &format!(" -isysroot {}", sdk_dir_simulator);
                cflags += &format!(" -mios-simulator-version-min={}", ios_simulator_version_min);
                host_arg = "--host=x86_64-apple-darwin10".to_string();
            }
            _ => panic!("Unknown iOS build target: {}", target),
        }
        cross_compiling = true;
    } else if target.contains("i686") {
        compiler += " -m32 -maes";
        cflags += " -march=i686";
    }

    let help = if cross_compiling {
        "***********************************************************\n\
         Possible missing dependencies.\n\
         See https://github.com/sodiumoxide/sodiumoxide#cross-compiling\n\
         ***********************************************************\n\n"
    } else {
        ""
    };

    // Run `./configure`
    let prefix_arg = format!("--prefix={}", install_dir.to_str().unwrap());
    let libdir_arg = format!("--libdir={}/lib", install_dir.to_str().unwrap());
    let mut configure_cmd = Command::new(fs::canonicalize(source_dir.join("configure")).expect("Failed to find configure script! Did you clone the submodule at `libsodium-sys/libsodium`?"));
    if !compiler.is_empty() {
        configure_cmd.env("CC", &compiler);
    }
    if !cflags.is_empty() {
        configure_cmd.env("CFLAGS", &cflags);
    }
    if env::var("SODIUM_DISABLE_PIE").is_ok() {
        configure_cmd.arg("--disable-pie");
    }
    let configure_status = configure_cmd
        .current_dir(&source_dir)
        .arg(&prefix_arg)
        .arg(&libdir_arg)
        .arg(&host_arg)
        .arg("--enable-shared=no")
        .status()
        .unwrap_or_else(|error| {
            panic!("Failed to run './configure': {}\n{}", error, help);
        });
    if !configure_status.success() {
        panic!(
            "\nFailed to configure libsodium using {:?}\nCFLAGS={}\nCC={}\n{}\n",
            configure_cmd, cflags, compiler, help
        );
    }

    // Run `make check`, or `make all` if we're cross-compiling
    let j_arg = format!("-j{}", env::var("NUM_JOBS").unwrap());
    let make_arg = if cross_compiling { "all" } else { "check" };
    let mut make_cmd = Command::new("make");
    let make_status = make_cmd
        .current_dir(&source_dir)
        .env("V", "1")
        .arg(make_arg)
        .arg(&j_arg)
        .status()
        .unwrap_or_else(|error| {
            panic!("Failed to run 'make {}': {}\n{}", make_arg, error, help);
        });
    if !make_status.success() {
        panic!("\nFailed to build libsodium using {:?}\n{}", make_cmd, help);
    }

    // Run `make install`
    let mut install_cmd = Command::new("make");
    let install_status = install_cmd
        .current_dir(&source_dir)
        .arg("install")
        .status()
        .unwrap_or_else(|error| {
            panic!("Failed to run 'make install': {}", error);
        });
    if !install_status.success() {
        panic!("\nFailed to install libsodium using {:?}", install_cmd);
    }

    install_dir.join("lib")
}

#[cfg(any(windows, target_env = "msvc"))]
fn get_crate_dir() -> PathBuf {
    env::var("CARGO_MANIFEST_DIR").unwrap().into()
}

#[cfg(target_env = "msvc")]
fn is_release_profile() -> bool {
    env::var("PROFILE").unwrap() == "release"
}

#[cfg(all(target_env = "msvc", target_pointer_width = "32"))]
fn get_lib_dir() -> PathBuf {
    if is_release_profile() {
        get_crate_dir().join("msvc/Win32/Release/v142/")
    } else {
        get_crate_dir().join("msvc/Win32/Debug/v142/")
    }
}

#[cfg(all(target_env = "msvc", target_pointer_width = "64"))]
fn get_lib_dir() -> PathBuf {
    if is_release_profile() {
        get_crate_dir().join("msvc/x64/Release/v142/")
    } else {
        get_crate_dir().join("msvc/x64/Debug/v142/")
    }
}

#[cfg(all(windows, not(target_env = "msvc"), target_pointer_width = "32"))]
fn get_lib_dir() -> PathBuf {
    get_crate_dir().join("mingw/win32/")
}

#[cfg(all(windows, not(target_env = "msvc"), target_pointer_width = "64"))]
fn get_lib_dir() -> PathBuf {
    get_crate_dir().join("mingw/win64/")
}

fn build_libsodium() {
    use std::{ffi::OsStr, fs};

    // Determine build target triple
    let mut out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let profile = env::var("PROFILE").unwrap();

    // Avoid issues with paths containing spaces by falling back to using a tempfile.
    // See https://github.com/jedisct1/libsodium/issues/207
    if out_dir.to_str().unwrap().contains(' ') {
        out_dir = env::temp_dir()
            .join("libsodium-sys")
            .join(&target)
            .join(&profile);
        println!(
            "cargo:warning=The path to the usual build directory contains spaces and hence \
             can't be used to build libsodium.  Falling back to use {}.  If running `cargo \
             clean`, ensure you also delete this fallback directory",
            out_dir.display()
        );
    }

    // Determine source and install dir
    let install_dir = out_dir.join("installed");
    let source_dir = out_dir.join("source").join("libsodium");

    // Create directories
    fs::create_dir_all(&install_dir).unwrap();
    fs::create_dir_all(&source_dir).unwrap();

    // Copy sources into build directory
    // Skip .git because it's marked read-only and that causes problems re-building
    for entry in walkdir::WalkDir::new("libsodium")
        .into_iter()
        .filter_entry(|e| e.file_name() != OsStr::new(".git"))
        .filter_map(Result::ok)
    {
        let outpath = out_dir.join("source").join(entry.path());
        if let Err(e) = if entry.file_type().is_dir() {
            fs::create_dir_all(outpath)
        } else {
            fs::copy(entry.path(), outpath).map(|_| ())
        } {
            panic!("Failed to copy sources into build directory: {}", e);
        }
    }

    let lib_dir = make_libsodium(&target, &source_dir, &install_dir);

    if target.contains("msvc") {
        println!("cargo:rustc-link-lib=static=libsodium");
    } else {
        println!("cargo:rustc-link-lib=static=sodium");
    }

    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir.to_str().unwrap()
    );

    let include_dir = source_dir.join("src/libsodium/include");

    println!("cargo:include={}", include_dir.to_str().unwrap());
    println!("cargo:lib={}", lib_dir.to_str().unwrap());
}
