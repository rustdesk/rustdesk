#![windows_subsystem = "windows"]

use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use bin_reader::BinaryReader;

pub mod bin_reader;

const APP_PREFIX: &str = "rustdesk";
const APPNAME_RUNTIME_ENV_KEY: &str = "RUSTDESK_APPNAME";

fn setup(reader: BinaryReader) -> Option<PathBuf> {
    // home dir
    if let Some(dir) = dirs::data_local_dir() {
        let dir = dir.join(APP_PREFIX);
        for file in reader.files.iter() {
            file.write_to_file(&dir);
        }
        #[cfg(unix)]
        reader.configure_permission(&dir);
        Some(dir.join(&reader.exe))
    } else {
        eprintln!("not found data local dir");
        None
    }
}

fn execute(path: PathBuf) {
    println!("executing {}", path.display());
    // setup env
    let exe = std::env::current_exe().unwrap();
    let exe_name = exe.file_name().unwrap();
    // run executable
    Command::new(path)
        .env(APPNAME_RUNTIME_ENV_KEY, exe_name)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect(&format!("failed to execute {:?}", exe_name));
}

fn main() {
    let reader = BinaryReader::default();
    if let Some(exe) = setup(reader) {
        execute(exe);
    }
}
