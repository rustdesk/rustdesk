use std::path::PathBuf;
use winapi::{
    shared::{
        minwindef::{BOOL, FALSE, LPARAM, TRUE},
        windef::HWND,
    },
    um::winuser::{
        CloseDesktop, EnumDesktopWindows, GetPropW, GetWindowThreadProcessId, IsWindowVisible,
        OpenDesktopW, DESKTOP_READOBJECTS,
    },
};

pub(crate) fn enabled() -> bool {
    super::is_cur_exe_the_installed()
        && hbb_common::config::Config::get_option(base::config::keys::OPTION_HIDE_CLIENT_WINDOW)
            == "Y"
}

pub(crate) fn rejects_new_connection_when_window_closed() -> bool {
    if hbb_common::config::Config::get_option("allow-only-conn-window-open") != "Y" {
        return false;
    }
    let (main_open, hidden_mode) = window_state();
    if hidden_mode {
        !main_open
    } else {
        crate::is_server() && !crate::check_process("", !crate::platform::is_root())
    }
}

pub(crate) fn add_startup_args(normal_launch: bool, flutter_args: &mut Vec<String>) {
    if normal_launch && !crate::platform::is_self_service_running() {
        flutter_args.push("--show-ui".to_owned());
    }
    if enabled() {
        flutter_args.push("--hidden-client-window-mode".to_owned());
    }
}

pub(crate) fn open_from_tray() {
    if enabled() {
        crate::run_me::<&str>(vec!["--show-ui"]).ok();
    } else {
        crate::run_me::<&str>(vec![]).ok();
    }
}

fn window_state() -> (bool, bool) {
    struct Context {
        executable: PathBuf,
        main_open: bool,
        hidden_mode: bool,
    }

    unsafe extern "system" fn check_window(hwnd: HWND, data: LPARAM) -> BOOL {
        let context = &mut *(data as *mut Context);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if let Ok(path) = super::get_process_executable_path(pid) {
            if path
                .to_string_lossy()
                .eq_ignore_ascii_case(&context.executable.to_string_lossy())
            {
                if !GetPropW(hwnd, super::wide_string("RustDeskClientWindow").as_ptr()).is_null() {
                    context.hidden_mode = true;
                }
                if IsWindowVisible(hwnd) != FALSE
                    && !GetPropW(hwnd, super::wide_string("RustDeskMainWindow").as_ptr()).is_null()
                {
                    context.main_open = true;
                }
                if context.main_open && context.hidden_mode {
                    return FALSE;
                }
            }
        }
        TRUE
    }

    let Ok(executable) = std::env::current_exe() else {
        return (false, false);
    };
    let mut context = Context {
        executable,
        main_open: false,
        hidden_mode: false,
    };
    unsafe {
        // The server can be on the lock-screen desktop; the main UI is on Default.
        let desktop = OpenDesktopW(
            super::wide_string("Default").as_ptr(),
            0,
            FALSE,
            DESKTOP_READOBJECTS,
        );
        if desktop.is_null() {
            return (false, false);
        }
        EnumDesktopWindows(
            desktop,
            Some(check_window),
            &mut context as *mut Context as LPARAM,
        );
        CloseDesktop(desktop);
    }
    (context.main_open, context.hidden_mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::null_mut;
    use winapi::um::winuser::*;

    #[test]
    fn only_an_open_main_window_allows_connections() {
        unsafe {
            let window = CreateWindowExW(
                0,
                super::super::wide_string("STATIC").as_ptr(),
                super::super::wide_string("Client window visibility test").as_ptr(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                100,
                100,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            assert!(!window.is_null());
            assert!(!window_state().0);
            ShowWindow(window, SW_SHOWNOACTIVATE);
            assert!(
                !window_state().0,
                "a connection-manager or unrelated window must not count"
            );
            SetPropW(
                window,
                super::super::wide_string("RustDeskMainWindow").as_ptr(),
                1usize as _,
            );
            assert!(window_state().0, "visible main window allows connections");
            ShowWindow(window, SW_MINIMIZE);
            assert!(window_state().0, "minimized main window remains open");
            ShowWindow(window, SW_HIDE);
            assert!(
                !window_state().0,
                "X and hidden startup must reject new connections"
            );
            SetPropW(
                window,
                super::super::wide_string("RustDeskClientWindow").as_ptr(),
                1usize as _,
            );
            assert!(window_state().1, "hidden-mode window remains identifiable");
            DestroyWindow(window);
            assert!(!window_state().0, "exited main window must reject new connections");
        }
    }
}
