use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use convert_case::{Case, Casing};
use serde::Deserialize;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use toml::Value;

#[derive(StructOpt, Debug, PartialEq, Deserialize, Default)]
#[structopt(setting(AppSettings::DeriveDisplayOrder))]
pub struct RawOpts {
    /// Path of input Rust code
    #[structopt(short, long)]
    pub rust_input: String,
    /// Path of output generated Dart code
    #[structopt(short, long)]
    pub dart_output: String,
    /// If provided, generated Dart declaration code to this separate file
    #[structopt(long)]
    pub dart_decl_output: Option<String>,

    /// Path of output generated C header
    #[structopt(short, long)]
    pub c_output: Option<Vec<String>>,
    /// Crate directory for your Rust project
    #[structopt(long)]
    pub rust_crate_dir: Option<String>,
    /// Path of output generated Rust code
    #[structopt(long)]
    pub rust_output: Option<String>,
    /// Generated class name
    #[structopt(long)]
    pub class_name: Option<String>,
    /// Line length for dart formatting
    #[structopt(long)]
    pub dart_format_line_length: Option<i32>,
    /// Skip automatically adding `mod bridge_generated;` to `lib.rs`
    #[structopt(long)]
    pub skip_add_mod_to_lib: bool,
    /// Path to the installed LLVM
    #[structopt(long)]
    pub llvm_path: Option<Vec<String>>,
    /// LLVM compiler opts
    #[structopt(long)]
    pub llvm_compiler_opts: Option<String>,
    /// Path to root of Dart project, otherwise inferred from --dart-output
    #[structopt(long)]
    pub dart_root: Option<String>,
    /// Skip running build_runner even when codegen-capable code is detected
    #[structopt(long)]
    pub no_build_runner: bool,
    /// Show debug messages.
    #[structopt(short, long)]
    pub verbose: bool,
}

#[derive(Debug)]
pub struct Opts {
    pub rust_input_path: String,
    pub dart_output_path: String,
    pub dart_decl_output_path: Option<String>,
    pub c_output_path: Vec<String>,
    pub rust_crate_dir: String,
    pub rust_output_path: String,
    pub class_name: String,
    pub dart_format_line_length: i32,
    pub skip_add_mod_to_lib: bool,
    pub llvm_path: Vec<String>,
    pub llvm_compiler_opts: String,
    pub manifest_path: String,
    pub dart_root: Option<String>,
    pub build_runner: bool,
}

pub fn parse(raw: RawOpts) -> Opts {
    let rust_input_path = canon_path(&raw.rust_input);

    let rust_crate_dir = canon_path(&raw.rust_crate_dir.unwrap_or_else(|| {
        fallback_rust_crate_dir(&rust_input_path)
            .unwrap_or_else(|_| panic!("{}", format_fail_to_guess_error("rust_crate_dir")))
    }));
    let manifest_path = {
        let mut path = std::path::PathBuf::from_str(&rust_crate_dir).unwrap();
        path.push("Cargo.toml");
        path_to_string(path).unwrap()
    };
    let rust_output_path = canon_path(&raw.rust_output.unwrap_or_else(|| {
        fallback_rust_output_path(&rust_input_path)
            .unwrap_or_else(|_| panic!("{}", format_fail_to_guess_error("rust_output")))
    }));
    let class_name = raw.class_name.unwrap_or_else(|| {
        fallback_class_name(&*rust_crate_dir)
            .unwrap_or_else(|_| panic!("{}", format_fail_to_guess_error("class_name")))
    });
    let c_output_path = raw
        .c_output
        .map(|outputs| {
            outputs
                .iter()
                .map(|output| canon_path(output))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![fallback_c_output_path()
                .unwrap_or_else(|_| panic!("{}", format_fail_to_guess_error("c_output")))]
        });

    let dart_root = {
        let dart_output = &raw.dart_output;
        raw.dart_root
            .as_deref()
            .map(canon_path)
            .or_else(|| fallback_dart_root(dart_output).ok())
    };

    Opts {
        rust_input_path,
        dart_output_path: canon_path(&raw.dart_output),
        dart_decl_output_path: raw
            .dart_decl_output
            .as_ref()
            .map(|s| canon_path(s.as_str())),
        c_output_path,
        rust_crate_dir,
        rust_output_path,
        class_name,
        dart_format_line_length: raw.dart_format_line_length.unwrap_or(80),
        skip_add_mod_to_lib: raw.skip_add_mod_to_lib,
        llvm_path: raw.llvm_path.unwrap_or_else(|| {
            vec![
                "/opt/homebrew/opt/llvm".to_owned(), // Homebrew root
                "/usr/local/opt/llvm".to_owned(),    // Homebrew x86-64 root
                // Possible Linux LLVM roots
                "/usr/lib/llvm-9".to_owned(),
                "/usr/lib/llvm-10".to_owned(),
                "/usr/lib/llvm-11".to_owned(),
                "/usr/lib/llvm-12".to_owned(),
                "/usr/lib/llvm-13".to_owned(),
                "/usr/lib/llvm-14".to_owned(),
                "/usr/lib/".to_owned(),
                "/usr/lib64/".to_owned(),
                "C:/Program Files/llvm".to_owned(), // Default on Windows
                "C:/Program Files/LLVM".to_owned(),
                "C:/msys64/mingw64".to_owned(), // https://packages.msys2.org/package/mingw-w64-x86_64-clang
            ]
        }),
        llvm_compiler_opts: raw.llvm_compiler_opts.unwrap_or_else(|| "".to_string()),
        manifest_path,
        dart_root,
        build_runner: !raw.no_build_runner,
    }
}

fn format_fail_to_guess_error(name: &str) -> String {
    format!(
        "fail to guess {}, please specify it manually in command line arguments",
        name
    )
}

fn fallback_rust_crate_dir(rust_input_path: &str) -> Result<String> {
    let mut dir_curr = Path::new(rust_input_path)
        .parent()
        .ok_or_else(|| anyhow!(""))?;

    loop {
        let path_cargo_toml = dir_curr.join("Cargo.toml");

        if path_cargo_toml.exists() {
            return Ok(dir_curr
                .as_os_str()
                .to_str()
                .ok_or_else(|| anyhow!(""))?
                .to_string());
        }

        if let Some(next_parent) = dir_curr.parent() {
            dir_curr = next_parent;
        } else {
            break;
        }
    }
    Err(anyhow!(
        "look at parent directories but none contains Cargo.toml"
    ))
}

fn fallback_c_output_path() -> Result<String> {
    let named_temp_file = Box::leak(Box::new(tempfile::Builder::new().suffix(".h").tempfile()?));
    Ok(named_temp_file
        .path()
        .to_str()
        .ok_or_else(|| anyhow!(""))?
        .to_string())
}

fn fallback_rust_output_path(rust_input_path: &str) -> Result<String> {
    Ok(Path::new(rust_input_path)
        .parent()
        .ok_or_else(|| anyhow!(""))?
        .join("bridge_generated.rs")
        .to_str()
        .ok_or_else(|| anyhow!(""))?
        .to_string())
}

fn fallback_dart_root(dart_output_path: &str) -> Result<String> {
    let mut res = canon_pathbuf(dart_output_path);
    while res.pop() {
        if res.join("pubspec.yaml").is_file() {
            return res
                .to_str()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow!("Non-utf8 path"));
        }
    }
    Err(anyhow!(
        "Root of Dart library could not be inferred from Dart output"
    ))
}

fn fallback_class_name(rust_crate_dir: &str) -> Result<String> {
    let cargo_toml_path = Path::new(rust_crate_dir).join("Cargo.toml");
    let cargo_toml_content = fs::read_to_string(cargo_toml_path)?;

    let cargo_toml_value = cargo_toml_content.parse::<Value>()?;
    let package_name = cargo_toml_value
        .get("package")
        .ok_or_else(|| anyhow!("no `package` in Cargo.toml"))?
        .get("name")
        .ok_or_else(|| anyhow!("no `name` in Cargo.toml"))?
        .as_str()
        .ok_or_else(|| anyhow!(""))?;

    Ok(package_name.to_case(Case::Pascal))
}

fn canon_path(sub_path: &str) -> String {
    let path = canon_pathbuf(sub_path);
    path_to_string(path).unwrap_or_else(|_| panic!("fail to parse path: {}", sub_path))
}

fn canon_pathbuf(sub_path: &str) -> PathBuf {
    let mut path =
        env::current_dir().unwrap_or_else(|_| panic!("fail to parse path: {}", sub_path));
    path.push(sub_path);
    path
}

fn path_to_string(path: PathBuf) -> Result<String, OsString> {
    path.into_os_string().into_string()
}

impl Opts {
    pub fn dart_api_class_name(&self) -> String {
        self.class_name.clone()
    }

    pub fn dart_api_impl_class_name(&self) -> String {
        format!("{}Impl", self.class_name)
    }

    pub fn dart_wire_class_name(&self) -> String {
        format!("{}Wire", self.class_name)
    }

    /// Returns None if the path terminates in "..", or not utf8.
    pub fn dart_output_path_name(&self) -> Option<&str> {
        let name = Path::new(&self.dart_output_path);
        let root = name.file_name()?.to_str()?;
        if let Some((name, _)) = root.rsplit_once('.') {
            Some(name)
        } else {
            Some(root)
        }
    }

    pub fn dart_output_freezed_path(&self) -> Option<String> {
        Some(
            Path::new(&self.dart_output_path)
                .with_extension("freezed.dart")
                .to_str()?
                .to_owned(),
        )
    }
}
