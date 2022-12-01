
use hbb_common::{
    log,
};

use std::{
    collections::HashMap,
    process::Child,
    sync::{Arc, Mutex}, error::Error,
};

use tauri::Manager;
use winapi::ctypes::c_void;


pub type Childs = Arc<Mutex<(bool, HashMap<(String, String), Child>)>>;

pub mod cm;
#[cfg(feature = "inline")]
pub mod inline;
#[cfg(target_os = "macos")]
mod macos;
pub mod remote;
#[cfg(target_os = "windows")]
pub mod win_privacy;


pub fn create_main_window(app: &tauri::AppHandle) -> tauri::Window {
    tauri::Window::builder(app, "main", tauri::WindowUrl::App("index.html".into()))
        .title("Rustdesk")
        .inner_size(700f64, 600f64)
        .center()
        .build()
        .unwrap()
}

fn create_remote_window(app: &tauri::AppHandle, id: &String) -> tauri::Window {
    tauri::Window::builder(app, "remote", tauri::WindowUrl::App("index.html".into()))
        .title(id)
        .inner_size(700f64, 600f64)
        .center()
        .build()
        .unwrap()
}

pub fn show_remote_window(app: &tauri::AppHandle) {
    if let Some(remote_window) = app.get_window("remote") {
        remote_window.show().unwrap();
        remote_window.unminimize().unwrap();
        remote_window.set_focus().unwrap();
    } else {
        create_remote_window(app, &"Undef".to_string());
    }
}

pub fn get_hwnd(window: impl raw_window_handle::HasRawWindowHandle) -> Result<*mut c_void, Box<dyn Error>> {
    match window.raw_window_handle() {
        #[cfg(target_os = "windows")]
        raw_window_handle::RawWindowHandle::Win32(handle) => {
            return Ok(handle.hwnd)
        }
        _ => Err("\"clear_acrylic()\" is only available on Windows 10 v1809 or newer and Windows 11.").map_err(Into::into),
    }
}

pub fn start(app: &tauri::AppHandle, args: &mut [String]) {
    #[cfg(all(windows, not(feature = "inline")))]
    unsafe {
        winapi::um::shellscalingapi::SetProcessDpiAwareness(2); // PROCESS_PER_MONITOR_DPI_AWARE
    }
    let page;
    if args.len() > 1 && args[0] == "--play" {
        args[0] = "--connect".to_owned();
        let path: std::path::PathBuf = (&args[1]).into();
        let id = path
            .file_stem()
            .map(|p| p.to_str().unwrap_or(""))
            .unwrap_or("")
            .to_owned();
        args[1] = id;
    } 
    
    if args.is_empty() {
        let child: Childs = Default::default();
        std::thread::spawn(move || check_zombie(child));
        // TODO:
        //  crate::common::check_software_update();
        page = "index.html";
        create_main_window(app);
        app.get_window("main").unwrap().open_devtools();
    } else if args[0] == "--install" {
        page = "install.html";
    } else if args[0] == "--cm" {
        // Implemetation "connection-manager" behavior using tauri state manager
        app.manage(Mutex::new(cm::TauriConnectionManager::new(app.clone()))); //TODO: Move app to static
        page = "cm.html";
    } else if (args[0] == "--connect"
        || args[0] == "--file-transfer"
        || args[0] == "--port-forward"
        || args[0] == "--rdp")
        && args.len() > 1
    {
        let mut iter = args.iter();
        let cmd = iter.next().unwrap().clone();
        let id = iter.next().unwrap().clone();
        let pass = iter.next().unwrap_or(&"".to_owned()).clone();
        let args: Vec<String> = iter.map(|x| x.clone()).collect();
        let remote = create_remote_window(&app, &id);
        #[cfg(windows)]
        {
            let hw = get_hwnd(remote).unwrap();
            // below copied from https://github.com/TigerVNC/tigervnc/blob/master/vncviewer/win32.c
            crate::platform::windows_lib::enable_lowlevel_keyboard(hw as _);
        }
        // Implemetation "native-remote" behavior using tauri state manager
        app.manage(Mutex::new(remote::TauriSession::new(
            cmd.clone(),
            id.clone(),
            pass.clone(),
            args.clone(),
        )));
        page = "remote.html";
    } else {
        log::error!("Wrong command: {:?}", args);
        return;
    }
    #[cfg(feature = "inline")]
    {
        let html = if page == "index.html" {
            inline::get_index()
        } else if page == "cm.html" {
            inline::get_cm()
        } else if page == "install.html" {
            inline::get_install()
        } else {
            inline::get_remote()
        };
        frame.load_html(html.as_bytes(), Some(page));
    }
    log::info!("page: {} args:{:?}", page, args);
    // #[cfg(not(feature = "inline"))]
    // window.load_file(&format!(
    //     "file://{}/src/ui/{}",
    //     std::env::current_dir()
    //         .map(|c| c.display().to_string())
    //         .unwrap_or("".to_owned()),
    //     page
    // ));
    // frame.run_app();
}

pub fn check_zombie(childs: Childs) {
    let mut deads = Vec::new();
    loop {
        let mut lock = childs.lock().unwrap();
        let mut n = 0;
        for (id, c) in lock.1.iter_mut() {
            if let Ok(Some(_)) = c.try_wait() {
                deads.push(id.clone());
                n += 1;
            }
        }
        for ref id in deads.drain(..) {
            lock.1.remove(id);
        }
        if n > 0 {
            lock.0 = true;
        }
        drop(lock);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}