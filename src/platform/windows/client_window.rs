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

pub(crate) fn is_open() -> bool {
    struct Context {
        executable: PathBuf,
        open: bool,
    }

    unsafe extern "system" fn check_window(hwnd: HWND, data: LPARAM) -> BOOL {
        let context = &mut *(data as *mut Context);
        if IsWindowVisible(hwnd) == FALSE
            || GetPropW(hwnd, super::wide_string("RustDeskMainWindow").as_ptr()).is_null()
        {
            return TRUE;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if let Ok(path) = super::get_process_executable_path(pid) {
            if path
                .to_string_lossy()
                .eq_ignore_ascii_case(&context.executable.to_string_lossy())
            {
                context.open = true;
                return FALSE;
            }
        }
        TRUE
    }

    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let mut context = Context {
        executable,
        open: false,
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
            return false;
        }
        EnumDesktopWindows(
            desktop,
            Some(check_window),
            &mut context as *mut Context as LPARAM,
        );
        CloseDesktop(desktop);
    }
    context.open
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
                super::super::wide_string("RD-4 visibility test").as_ptr(),
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
            assert!(!is_open());
            ShowWindow(window, SW_SHOWNOACTIVATE);
            assert!(
                !is_open(),
                "a connection-manager or unrelated window must not count"
            );
            SetPropW(
                window,
                super::super::wide_string("RustDeskMainWindow").as_ptr(),
                1usize as _,
            );
            assert!(is_open(), "visible main window allows connections");
            ShowWindow(window, SW_MINIMIZE);
            assert!(is_open(), "minimized main window remains open");
            ShowWindow(window, SW_HIDE);
            assert!(
                !is_open(),
                "X and hidden startup must reject new connections"
            );
            DestroyWindow(window);
            assert!(!is_open(), "exited main window must reject new connections");
        }
    }
}
