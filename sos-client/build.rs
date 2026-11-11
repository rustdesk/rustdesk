fn main() {
    // 编译前终止所有运行中的旧进程，防止二进制文件被锁定导致编译失败
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/f", "/im", "rustdesk-sos.exe"])
            .output();
    }

    // 嵌入 Windows 可执行文件图标
    #[cfg(windows)]
    {
        // winres 会读取本包的 Cargo.toml 里的 [package.metadata.winres]
        // 来填版本资源字符串，但它自己不声明这个依赖。不写这行的话，
        // 只改那段 metadata 时 build script 不会重跑，exe 里留的是旧值。
        println!("cargo:rerun-if-changed=Cargo.toml");
        let mut res = winres::WindowsResource::new();
        res.set_icon("../res/icon.ico");
        res.compile().unwrap();
    }
    // 编译 protobuf
    let out_dir = format!("{}/protos", std::env::var("OUT_DIR").unwrap());
    std::fs::create_dir_all(&out_dir).unwrap();

    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir(out_dir)
        .inputs(["protos/rendezvous.proto", "protos/message.proto"])
        .include("protos")
        .customize(protobuf_codegen::Customize::default().tokio_bytes(true))
        .run()
        .expect("Protobuf codegen failed.");

    // 编译 Windows 平台 C++ 辅助代码
    #[cfg(windows)]
    {
        let file = "platform/windows.cc";
        cc::Build::new().file(file).compile("sos_windows");
        println!("cargo:rustc-link-lib=WtsApi32");
        println!("cargo:rerun-if-changed={}", file);
    }

    // 生成 libvpx 绑定（仅 Windows）。
    // 目的：`libs/scrap` 里的 `codec_thread_num()` 被 `available_memory/2` 限制，
    // 无法从外部控制编码线程数。这里直接链接 vcpkg 提供的同一个 vpx.lib，
    // 自建编码器从而完全掌控线程数/CPU 等级/分辨率。
    #[cfg(windows)]
    generate_vpx_bindings();
}

#[cfg(windows)]
fn generate_vpx_bindings() {
    println!("cargo:rerun-if-changed=src/bindings/vpx_ffi.h");
    let Some(vcpkg_lib_dir) = find_vcpkg_lib_dir() else {
        println!(
            "cargo:warning=vcpkg x64-windows-static not found, SOS self-managed libvpx encoder disabled"
        );
        return;
    };
    let include = vcpkg_lib_dir
        .parent()
        .map(|p| p.join("include"))
        .filter(|p| p.join("vpx").join("vpx_codec.h").exists());
    let Some(include) = include else {
        println!(
            "cargo:warning=vcpkg vpx headers not found, SOS self-managed libvpx encoder disabled"
        );
        return;
    };

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let bindings = out_dir.join("vpx_ffi.rs");
    let header = std::path::Path::new("src/bindings/vpx_ffi.h");
    let generated = bindgen::builder()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", include.display()))
        .allowlist_type("^[vV].*")
        .allowlist_var("^[vV].*")
        .allowlist_function("^[vV].*")
        .rustified_enum("^[vV].*")
        .trust_clang_mangling(false)
        .layout_tests(false)
        .generate_comments(false)
        .generate();
    match generated {
        Ok(b) => {
            if let Err(e) = b.write_to_file(&bindings) {
                println!("cargo:warning=failed to write vpx bindings: {e}");
                return;
            }
            println!("cargo:rustc-link-lib=static=vpx");
            println!("cargo:rustc-link-search={}", vcpkg_lib_dir.display());
            println!("cargo::rustc-check-cfg=cfg(sos_vpx)");
            println!("cargo:rustc-cfg=sos_vpx");
            println!("cargo:warning=SOS self-managed libvpx encoder enabled");
        }
        Err(e) => {
            println!("cargo:warning=failed to generate vpx bindings: {e}");
        }
    }
}

/// 定位 vcpkg 静态库目录（含 vpx.lib）。
#[cfg(windows)]
fn find_vcpkg_lib_dir() -> Option<std::path::PathBuf> {
    let target = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x64-windows-static",
        Ok("x86") => "x86-windows-static",
        Ok("aarch64") => "arm64-windows-static",
        _ => return None,
    };
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(root) = std::env::var("VCPKG_INSTALLED_ROOT") {
        candidates.push(std::path::PathBuf::from(root).join(target).join("lib"));
    }
    if let Ok(root) = std::env::var("VCPKG_ROOT") {
        candidates.push(
            std::path::PathBuf::from(root)
                .join("installed")
                .join(target)
                .join("lib"),
        );
    }
    // cargo 的当前目录是本包目录（sos-client），仓库根在其上一级
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let repo = std::path::PathBuf::from(manifest_dir)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        candidates.push(repo.join("vcpkg_installed").join(target).join("lib"));
    }
    candidates.into_iter().find(|p| p.join("vpx.lib").exists())
}
