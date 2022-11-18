
use hbb_common::{
    log,
};

use std::{
    collections::HashMap,
    process::Child,
    sync::{Arc, Mutex},
};

use tauri::Manager;


pub type Childs = Arc<Mutex<(bool, HashMap<(String, String), Child>)>>;

pub mod cm;
#[cfg(feature = "inline")]
pub mod inline;
#[cfg(target_os = "macos")]
mod macos;
pub mod remote;
#[cfg(target_os = "windows")]
pub mod win_privacy;


pub fn start(app: &tauri::AppHandle, args: &mut [String]) {
    #[cfg(all(windows, not(feature = "inline")))]
    unsafe {
        winapi::um::shellscalingapi::SetProcessDpiAwareness(2); // PROCESS_PER_MONITOR_DPI_AWARE
    }
    let page;
    let mut handler: Vec<String> = Vec::new();
    // let window = app.get_window("main").unwrap();
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
    } else if args[0] == "--install" {
        page = "install.html";
    } else if args[0] == "--cm" {
        app.manage(Mutex::new(cm::TauriConnectionManager::new(app.clone()))); //TODO: Move app to static
        handler.push("connection-manager".to_string());
        page = "cm.html";
    } else if (args[0] == "--connect"
        || args[0] == "--file-transfer"
        || args[0] == "--port-forward"
        || args[0] == "--rdp")
        && args.len() > 1
    {
        // #[cfg(windows)]
        // {
        //     let hw = crate::processing::get_hwnd(&window).unwrap();
        //     // below copied from https://github.com/TigerVNC/tigervnc/blob/master/vncviewer/win32.c
        //     crate::platform::windows_lib::enable_lowlevel_keyboard(hw as _);
        // }
        let mut iter = args.iter();
        let cmd = iter.next().unwrap().clone();
        let id = iter.next().unwrap().clone();
        let pass = iter.next().unwrap_or(&"".to_owned()).clone();
        let args: Vec<String> = iter.map(|x| x.clone()).collect();
        // window.set_title(&id).unwrap();
        // TODO: TauriSession handler implementation
        app.manage(Mutex::new(remote::TauriSession::new(
            cmd.clone(),
            id.clone(),
            pass.clone(),
            args.clone(),
        )));
        handler.push(format!("native-remote {} {} {} {:?}", cmd, id, pass, args)); 
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