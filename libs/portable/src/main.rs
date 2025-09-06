#![windows_subsystem = "windows"]

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use bin_reader::BinaryReader;

pub mod bin_reader;
#[cfg(windows)]
mod ui;

#[cfg(windows)]
const APP_METADATA: &[u8] = include_bytes!("../app_metadata.toml");
#[cfg(not(windows))]
const APP_METADATA: &[u8] = &[];
const APP_METADATA_CONFIG: &str = "meta.toml";
const META_LINE_PREFIX_TIMESTAMP: &str = "timestamp = ";
const APP_PREFIX: &str = "rustdesk";
const APPNAME_RUNTIME_ENV_KEY: &str = "RUSTDESK_APPNAME";
#[cfg(windows)]
const SET_FOREGROUND_WINDOW_ENV_KEY: &str = "SET_FOREGROUND_WINDOW";

fn is_timestamp_matches(dir: &Path, ts: &mut u64) -> bool {
    let Ok(app_metadata) = std::str::from_utf8(APP_METADATA) else {
        return true;
    };
    for line in app_metadata.lines() {
        if line.starts_with(META_LINE_PREFIX_TIMESTAMP) {
            if let Ok(stored_ts) = line.replace(META_LINE_PREFIX_TIMESTAMP, "").parse::<u64>() {
                *ts = stored_ts;
                break;
            }
        }
    }
    if *ts == 0 {
        return true;
    }

    if let Ok(content) = std::fs::read_to_string(dir.join(APP_METADATA_CONFIG)) {
        for line in content.lines() {
            if line.starts_with(META_LINE_PREFIX_TIMESTAMP) {
                if let Ok(stored_ts) = line.replace(META_LINE_PREFIX_TIMESTAMP, "").parse::<u64>() {
                    return *ts == stored_ts;
                }
            }
        }
    }
    false
}

fn write_meta(dir: &Path, ts: u64) {
    let meta_file = dir.join(APP_METADATA_CONFIG);
    if ts != 0 {
        let content = format!("{}{}", META_LINE_PREFIX_TIMESTAMP, ts);
        // Ignore is ok here
        let _ = std::fs::write(meta_file, content);
    }
}

fn setup(
    reader: BinaryReader,
    dir: Option<PathBuf>,
    clear: bool,
    _args: &Vec<String>,
    _ui: &mut bool,
) -> Option<PathBuf> {
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

    let mut ts = 0;
    if clear || !is_timestamp_matches(&dir, &mut ts) {
        #[cfg(windows)]
        if _args.is_empty() {
            *_ui = true;
            ui::setup();
        }
        std::fs::remove_dir_all(&dir).ok();
    }
    for file in reader.files.iter() {
        file.write_to_file(&dir);
    }
    write_meta(&dir, ts);
    #[cfg(windows)]
    windows::copy_runtime_broker(&dir);
    #[cfg(linux)]
    reader.configure_permission(&dir);
    Some(dir.join(&reader.exe))
}

fn execute(path: PathBuf, args: Vec<String>, _ui: bool) {
    println!("executing {}", path.display());
    // setup env
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_name = exe.file_name().unwrap_or_default();
    // run executable
    let mut cmd = Command::new(path);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);
        if _ui {
            cmd.env(SET_FOREGROUND_WINDOW_ENV_KEY, "1");
        }
    }
    let _child = cmd
        .env(APPNAME_RUNTIME_ENV_KEY, exe_name)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    #[cfg(windows)]
    if _ui {
        match _child {
            Ok(child) => unsafe {
                winapi::um::winuser::AllowSetForegroundWindow(child.id() as u32);
            },
            Err(e) => {
                eprintln!("{:?}", e);
            }
        }
    }
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

    let mut ui = false;
    let reader = BinaryReader::default();
    if let Some(exe) = setup(
        reader,
        None,
        click_setup || args.contains(&"--silent-install".to_owned()),
        &args,
        &mut ui,
    ) {
        if click_setup {
            args = vec!["--install".to_owned()];
        } else if quick_support {
            args = vec!["--quick_support".to_owned()];
        }
        execute(exe, args, ui);
    }
}

#[cfg(windows)]
mod windows {
    use std::{fs, os::windows::process::CommandExt, path::Path, process::Command};

    // Used for privacy mode(magnifier impl).
    pub const RUNTIME_BROKER_EXE: &'static str = "C:\\Windows\\System32\\RuntimeBroker.exe";
    pub const WIN_TOPMOST_INJECTED_PROCESS_EXE: &'static str = "RuntimeBroker_rustdesk.exe";

    pub(super) fn copy_runtime_broker(dir: &Path) {
        let src = RUNTIME_BROKER_EXE;
        let tgt = WIN_TOPMOST_INJECTED_PROCESS_EXE;
        let target_file = dir.join(tgt);
        if target_file.exists() {
            if let (Ok(src_file), Ok(tgt_file)) = (fs::read(src), fs::read(&target_file)) {
                let src_md5 = format!("{:x}", md5::compute(&src_file));
                let tgt_md5 = format!("{:x}", md5::compute(&tgt_file));
                if src_md5 == tgt_md5 {
                    return;
                }
            }
        }
        let _allow_err = Command::new("taskkill")
            .args(&["/F", "/IM", "RuntimeBroker_rustdesk.exe"])
            .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
            .output();
        let _allow_err = std::fs::copy(src, &format!("{}\\{}", dir.to_string_lossy(), tgt));
    }
}
