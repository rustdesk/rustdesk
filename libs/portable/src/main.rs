#![windows_subsystem = "windows"]

use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use bin_reader::BinaryReader;

pub mod bin_reader;

const APP_PREFIX: &str = "rustdesk";
const APPNAME_RUNTIME_ENV_KEY: &str = "RUSTDESK_APPNAME";

fn setup(reader: BinaryReader, dir: Option<PathBuf>, clear: bool) -> Option<PathBuf> {
    let dir = if let Some(dir) = dir {
        dir
    } else {
        // home dir
        if let Some(dir) = dirs::data_local_dir() {
            dir.join(APP_PREFIX)
        } else {
            eprintln!("not found data local dir");
            return None;
        }
    };
    if clear {
        std::fs::remove_dir_all(&dir).ok();
    }
    for file in reader.files.iter() {
        file.write_to_file(&dir);
    }
    #[cfg(unix)]
    reader.configure_permission(&dir);
    Some(dir.join(&reader.exe))
}

fn execute(path: PathBuf, args: Vec<String>) {
    println!("executing {}", path.display());
    // setup env
    let exe = std::env::current_exe().unwrap();
    let exe_name = exe.file_name().unwrap();
    // run executable
    Command::new(path)
        .args(args)
        .env(APPNAME_RUNTIME_ENV_KEY, exe_name)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect(&format!("failed to execute {:?}", exe_name));
}

fn main() {
    let mut args = Vec::new();
    let mut arg_exe = Default::default();
    let mut i = 0;
    for arg in std::env::args() {
        if i == 0 {
            arg_exe = arg.clone();
        } else {
            args.push(arg);
        }
        i += 1;
    }
    let click_setup = args.is_empty() && arg_exe.to_lowercase().ends_with("install.exe");
    let quick_support = args.is_empty() && arg_exe.to_lowercase().ends_with("qs.exe");

    let reader = BinaryReader::default();
    if let Some(exe) = setup(
        reader,
        None,
        click_setup || args.contains(&"--silent-install".to_owned()),
    ) {
        if click_setup {
            args = vec!["--install".to_owned()];
        } else if quick_support {
            args = vec!["--quick_support".to_owned()];
        }
        execute(exe, args);
    }
}
