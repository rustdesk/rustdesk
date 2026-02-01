use cc::Build;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let externals_dir = manifest_dir.join("externals");
    let cpp_dir = manifest_dir.join("cpp");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=deps");
    println!("cargo:rerun-if-changed={}", externals_dir.display());
    println!("cargo:rerun-if-changed={}", cpp_dir.display());
    let mut builder = Build::new();

    build_common(&mut builder);
    ffmpeg::build_ffmpeg(&mut builder);
    #[cfg(all(windows, feature = "vram"))]
    sdk::build_sdk(&mut builder);
    builder.static_crt(true).compile("hwcodec");
}

fn build_common(builder: &mut Build) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let common_dir = manifest_dir.join("cpp").join("common");
    bindgen::builder()
        .header(common_dir.join("common.h").to_string_lossy().to_string())
        .header(common_dir.join("callback.h").to_string_lossy().to_string())
        .rustified_enum("*")
        .parse_callbacks(Box::new(CommonCallbacks))
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("common_ffi.rs"))
        .unwrap();

    // system
    #[cfg(windows)]
    {
        ["d3d11", "dxgi"].map(|lib| println!("cargo:rustc-link-lib={}", lib));
    }

    builder.include(&common_dir);

    // platform
    let _platform_path = common_dir.join("platform");
    #[cfg(windows)]
    {
        let win_path = _platform_path.join("win");
        builder.include(&win_path);
        builder.file(win_path.join("win.cpp"));
    }
    #[cfg(target_os = "linux")]
    {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let externals_dir = manifest_dir.join("externals");
        // ffnvcodec
        let ffnvcodec_path = externals_dir
            .join("nv-codec-headers_n12.1.14.0")
            .join("include")
            .join("ffnvcodec");
        builder.include(ffnvcodec_path);

        let linux_path = _platform_path.join("linux");
        builder.include(&linux_path);
        builder.file(linux_path.join("linux.cpp"));
    }
    if target_os == "macos" {
        let macos_path = _platform_path.join("mac");
        builder.include(&macos_path);
        builder.file(macos_path.join("mac.mm"));
    }

    // tool
    builder.files(["log.cpp", "util.cpp"].map(|f| common_dir.join(f)));
}

#[derive(Debug)]
struct CommonCallbacks;
impl bindgen::callbacks::ParseCallbacks for CommonCallbacks {
    fn add_derives(&self, name: &str) -> Vec<String> {
        let names = vec!["DataFormat", "SurfaceFormat", "API"];
        if names.contains(&name) {
            vec!["Serialize", "Deserialize"]
                .drain(..)
                .map(|s| s.to_string())
                .collect()
        } else {
            vec![]
        }
    }
}

mod ffmpeg {
    #[allow(unused_imports)]
    use core::panic;

    use super::*;

    pub fn build_ffmpeg(builder: &mut Build) {
        ffmpeg_ffi();
        link_vcpkg(builder, std::env::var("VCPKG_ROOT").unwrap().into());
        link_os();
        build_ffmpeg_ram(builder);
        #[cfg(feature = "vram")]
        build_ffmpeg_vram(builder);
        build_mux(builder);
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        if target_os == "macos" || target_os == "ios" {
            builder.flag("-std=c++11");
        }
    }

    fn link_vcpkg(builder: &mut Build, mut path: PathBuf) -> PathBuf {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        let mut target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        if target_arch == "x86_64" {
            target_arch = "x64".to_owned();
        } else if target_arch == "x86" {
            target_arch = "x86".to_owned();
        } else if target_arch == "loongarch64" {
            target_arch = "loongarch64".to_owned();
        } else if target_arch == "aarch64" {
            target_arch = "arm64".to_owned();
        } else {
            target_arch = "arm".to_owned();
        }
        let mut target = if target_os == "macos" {
            if target_arch == "x64" {
                "x64-osx".to_owned()
            } else if target_arch == "arm64" {
                "arm64-osx".to_owned()
            } else {
                format!("{}-{}", target_arch, target_os)
            }
        } else if target_os == "windows" {
            "x64-windows-static".to_owned()
        } else {
            format!("{}-{}", target_arch, target_os)
        };
        if target_arch == "x86" {
            target = target.replace("x64", "x86");
        }
        println!("cargo:info={}", target);
        path.push("installed");
        path.push(target);

        println!(
            "{}",
            format!(
                "cargo:rustc-link-search=native={}",
                path.join("lib").to_str().unwrap()
            )
        );
        {
            let mut static_libs = vec!["avcodec", "avutil", "avformat"];
            if target_os == "windows" {
                static_libs.push("libmfx");
            }
            static_libs
                .iter()
                .map(|lib| println!("cargo:rustc-link-lib=static={}", lib))
                .count();
        }

        let include = path.join("include");
        println!("{}", format!("cargo:include={}", include.to_str().unwrap()));
        builder.include(&include);
        include
    }

    fn link_os() {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let dyn_libs: Vec<&str> = if target_os == "windows" {
            ["User32", "bcrypt", "ole32", "advapi32"].to_vec()
        } else if target_os == "linux" {
            let mut v = ["drm", "X11", "stdc++"].to_vec();
            if target_arch == "x86_64" {
                v.push("z");
            }
            v
        } else if target_os == "macos" || target_os == "ios" {
            ["c++", "m"].to_vec()
        } else if target_os == "android" {
            // https://github.com/FFmpeg/FFmpeg/commit/98b5e80fd6980e641199e9ce3bc27100e2df17a4
            // link to mediandk directly since n7.1
            ["z", "m", "android", "atomic", "mediandk"].to_vec()
        } else {
            panic!("unsupported os");
        };
        dyn_libs
            .iter()
            .map(|lib| println!("cargo:rustc-link-lib={}", lib))
            .count();

        if target_os == "macos" || target_os == "ios" {
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=CoreVideo");
            println!("cargo:rustc-link-lib=framework=CoreMedia");
            println!("cargo:rustc-link-lib=framework=VideoToolbox");
            println!("cargo:rustc-link-lib=framework=AVFoundation");
        }
    }

    fn ffmpeg_ffi() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("common");
        let ffi_header = ffmpeg_ram_dir
            .join("ffmpeg_ffi.h")
            .to_string_lossy()
            .to_string();
        bindgen::builder()
            .header(ffi_header)
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_ffi.rs"))
            .unwrap();
    }

    fn build_ffmpeg_ram(builder: &mut Build) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("ffmpeg_ram");
        let ffi_header = ffmpeg_ram_dir
            .join("ffmpeg_ram_ffi.h")
            .to_string_lossy()
            .to_string();
        bindgen::builder()
            .header(ffi_header)
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_ram_ffi.rs"))
            .unwrap();

        builder.files(
            ["ffmpeg_ram_encode.cpp", "ffmpeg_ram_decode.cpp"].map(|f| ffmpeg_ram_dir.join(f)),
        );
    }

    #[cfg(feature = "vram")]
    fn build_ffmpeg_vram(builder: &mut Build) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("ffmpeg_vram");
        let ffi_header = ffmpeg_ram_dir
            .join("ffmpeg_vram_ffi.h")
            .to_string_lossy()
            .to_string();
        bindgen::builder()
            .header(ffi_header)
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_vram_ffi.rs"))
            .unwrap();

        builder.files(
            ["ffmpeg_vram_decode.cpp", "ffmpeg_vram_encode.cpp"].map(|f| ffmpeg_ram_dir.join(f)),
        );
    }

    fn build_mux(builder: &mut Build) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mux_dir = manifest_dir.join("cpp").join("mux");
        let mux_header = mux_dir.join("mux_ffi.h").to_string_lossy().to_string();
        bindgen::builder()
            .header(mux_header)
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("mux_ffi.rs"))
            .unwrap();

        builder.files(["mux.cpp"].map(|f| mux_dir.join(f)));
    }
}

#[cfg(all(windows, feature = "vram"))]
mod sdk {
    use super::*;

    pub(crate) fn build_sdk(builder: &mut Build) {
        build_amf(builder);
        build_nv(builder);
        build_mfx(builder);
    }

    fn build_nv(builder: &mut Build) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let externals_dir = manifest_dir.join("externals");
        let common_dir = manifest_dir.join("common");
        let nv_dir = manifest_dir.join("cpp").join("nv");
        println!("cargo:rerun-if-changed=src");
        println!("cargo:rerun-if-changed={}", common_dir.display());
        println!("cargo:rerun-if-changed={}", externals_dir.display());
        bindgen::builder()
            .header(&nv_dir.join("nv_ffi.h").to_string_lossy().to_string())
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("nv_ffi.rs"))
            .unwrap();

        // system
        #[cfg(target_os = "windows")]
        [
            "kernel32", "user32", "gdi32", "winspool", "shell32", "ole32", "oleaut32", "uuid",
            "comdlg32", "advapi32", "d3d11", "dxgi",
        ]
        .map(|lib| println!("cargo:rustc-link-lib={}", lib));
        #[cfg(target_os = "linux")]
        println!("cargo:rustc-link-lib=stdc++");

        // ffnvcodec
        let ffnvcodec_path = externals_dir
            .join("nv-codec-headers_n12.1.14.0")
            .join("include")
            .join("ffnvcodec");
        builder.include(ffnvcodec_path);

        // video codc sdk
        let sdk_path = externals_dir.join("Video_Codec_SDK_12.1.14");
        builder.includes([
            sdk_path.clone(),
            sdk_path.join("Interface"),
            sdk_path.join("Samples").join("Utils"),
            sdk_path.join("Samples").join("NvCodec"),
            sdk_path.join("Samples").join("NvCodec").join("NVEncoder"),
            sdk_path.join("Samples").join("NvCodec").join("NVDecoder"),
        ]);

        for file in vec!["NvEncoder.cpp", "NvEncoderD3D11.cpp"] {
            builder.file(
                sdk_path
                    .join("Samples")
                    .join("NvCodec")
                    .join("NvEncoder")
                    .join(file),
            );
        }
        for file in vec!["NvDecoder.cpp"] {
            builder.file(
                sdk_path
                    .join("Samples")
                    .join("NvCodec")
                    .join("NvDecoder")
                    .join(file),
            );
        }

        // crate
        builder.files(["nv_encode.cpp", "nv_decode.cpp"].map(|f| nv_dir.join(f)));
    }

    fn build_amf(builder: &mut Build) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let externals_dir = manifest_dir.join("externals");
        let amf_dir = manifest_dir.join("cpp").join("amf");
        println!("cargo:rerun-if-changed=src");
        println!("cargo:rerun-if-changed={}", externals_dir.display());
        bindgen::builder()
            .header(amf_dir.join("amf_ffi.h").to_string_lossy().to_string())
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("amf_ffi.rs"))
            .unwrap();

        // system
        #[cfg(windows)]
        println!("cargo:rustc-link-lib=ole32");
        #[cfg(target_os = "linux")]
        println!("cargo:rustc-link-lib=stdc++");

        // amf
        let amf_path = externals_dir.join("AMF_v1.4.35");
        builder.include(format!("{}/amf/public/common", amf_path.display()));
        builder.include(amf_path.join("amf"));

        for f in vec![
            "AMFFactory.cpp",
            "AMFSTL.cpp",
            "Thread.cpp",
            #[cfg(windows)]
            "Windows/ThreadWindows.cpp",
            #[cfg(target_os = "linux")]
            "Linux/ThreadLinux.cpp",
            "TraceAdapter.cpp",
        ] {
            builder.file(format!("{}/amf/public/common/{}", amf_path.display(), f));
        }

        // crate
        builder.files(["amf_encode.cpp", "amf_decode.cpp"].map(|f| amf_dir.join(f)));
    }

    fn build_mfx(builder: &mut Build) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let externals_dir = manifest_dir.join("externals");
        let mfx_dir = manifest_dir.join("cpp").join("mfx");
        println!("cargo:rerun-if-changed=src");
        println!("cargo:rerun-if-changed={}", externals_dir.display());
        bindgen::builder()
            .header(&mfx_dir.join("mfx_ffi.h").to_string_lossy().to_string())
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("mfx_ffi.rs"))
            .unwrap();

        // MediaSDK
        let sdk_path = externals_dir.join("MediaSDK_22.5.4");

        // mfx_dispatch
        let mfx_path = sdk_path.join("api").join("mfx_dispatch");
        // include headers and reuse static lib
        builder.include(mfx_path.join("windows").join("include"));

        let sample_path = sdk_path.join("samples").join("sample_common");
        builder
            .includes([
                sdk_path.join("api").join("include"),
                sample_path.join("include"),
            ])
            .files(
                [
                    "sample_utils.cpp",
                    "base_allocator.cpp",
                    "d3d11_allocator.cpp",
                    "avc_bitstream.cpp",
                    "avc_spl.cpp",
                    "avc_nal_spl.cpp",
                ]
                .map(|f| sample_path.join("src").join(f)),
            )
            .files(
                [
                    "time.cpp",
                    "atomic.cpp",
                    "shared_object.cpp",
                    "thread_windows.cpp",
                ]
                .map(|f| sample_path.join("src").join("vm").join(f)),
            );

        // link
        [
            "kernel32", "user32", "gdi32", "winspool", "shell32", "ole32", "oleaut32", "uuid",
            "comdlg32", "advapi32", "d3d11", "dxgi",
        ]
        .map(|lib| println!("cargo:rustc-link-lib={}", lib));

        builder
            .files(["mfx_encode.cpp", "mfx_decode.cpp"].map(|f| mfx_dir.join(f)))
            .define("NOMINMAX", None)
            .define("MFX_DEPRECATED_OFF", None)
            .define("MFX_D3D11_SUPPORT", None);
    }
}
