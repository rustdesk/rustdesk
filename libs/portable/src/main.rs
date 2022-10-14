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

fn is_setup(name: &str) -> bool {
    name.to_lowercase().ends_with("install.exe") || name.to_lowercase().ends_with("安装.exe")
}

fn main() {
    let is_setup = is_setup(
        &std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .to_string(),
    );
    let reader = BinaryReader::default();
    if let Some(exe) = setup(reader, None, is_setup) {
        let args = if is_setup {
            vec!["--install".to_owned()]
        } else {
            vec![]
        };
        execute(exe, args);
    }
}
