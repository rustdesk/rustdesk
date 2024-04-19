use super::{PrivacyMode, INVALID_PRIVACY_MODE_CONN_ID};
use crate::{platform::windows::get_user_token, privacy_mode::PrivacyModeState};
use hbb_common::{allow_err, bail, log, ResultType};
use std::{
    ffi::CString,
    io::Error,
    time::{Duration, Instant},
};
use winapi::{
    shared::{
        minwindef::FALSE,
        ntdef::{HANDLE, NULL},
        windef::HWND,
    },
    um::{
        handleapi::CloseHandle,
        libloaderapi::{GetModuleHandleA, GetProcAddress},
        memoryapi::{VirtualAllocEx, WriteProcessMemory},
        processthreadsapi::{
            CreateProcessAsUserW, QueueUserAPC, ResumeThread, TerminateProcess,
            PROCESS_INFORMATION, STARTUPINFOW,
        },
        winbase::{WTSGetActiveConsoleSessionId, CREATE_SUSPENDED, DETACHED_PROCESS},
        winnt::{MEM_COMMIT, PAGE_READWRITE},
        winuser::*,
    },
};

pub(super) const PRIVACY_MODE_IMPL: &str = "privacy_mode_impl_mag";

pub const ORIGIN_PROCESS_EXE: &'static str = "C:\\Windows\\System32\\RuntimeBroker.exe";
pub const WIN_TOPMOST_INJECTED_PROCESS_EXE: &'static str = "RuntimeBroker_rustdesk.exe";
pub const INJECTED_PROCESS_EXE: &'static str = WIN_TOPMOST_INJECTED_PROCESS_EXE;
pub(super) const PRIVACY_WINDOW_NAME: &'static str = "RustDeskPrivacyWindow";

struct WindowHandlers {
    hthread: u64,
    hprocess: u64,
}

impl Drop for WindowHandlers {
    fn drop(&mut self) {
        self.reset();
    }
}

impl WindowHandlers {
    fn reset(&mut self) {
        unsafe {
            if self.hprocess != 0 {
                let _res = TerminateProcess(self.hprocess as _, 0);
                CloseHandle(self.hprocess as _);
            }
            self.hprocess = 0;
            if self.hthread != 0 {
                CloseHandle(self.hthread as _);
            }
            self.hthread = 0;
        }
    }

    fn is_default(&self) -> bool {
        self.hthread == 0 && self.hprocess == 0
    }
}

pub struct PrivacyModeImpl {
    impl_key: String,
    conn_id: i32,
    handlers: WindowHandlers,
    hwnd: u64,
}

impl PrivacyMode for PrivacyModeImpl {
    fn is_async_privacy_mode(&self) -> bool {
        false
    }

    fn init(&self) -> ResultType<()> {
        Ok(())
    }

    fn clear(&mut self) {
        allow_err!(self.turn_off_privacy(self.conn_id, None));
    }

    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if self.check_on_conn_id(conn_id)? {
            log::debug!("Privacy mode of conn {} is already on", conn_id);
            return Ok(true);
        }

        let exe_file = std::env::current_exe()?;
        if let Some(cur_dir) = exe_file.parent() {
            if !cur_dir.join("WindowInjection.dll").exists() {
                return Ok(false);
            }
        } else {
            bail!(
                "Invalid exe parent for {}",
                exe_file.to_string_lossy().as_ref()
            );
        }

        if self.handlers.is_default() {
            log::info!("turn_on_privacy, dll not found when started, try start");
            self.start()?;
            std::thread::sleep(std::time::Duration::from_millis(1_000));
        }

        let hwnd = wait_find_privacy_hwnd(0)?;
        if hwnd.is_null() {
            bail!("No privacy window created");
        }
        super::win_input::hook()?;
        unsafe {
            ShowWindow(hwnd as _, SW_SHOW);
        }
        self.conn_id = conn_id;
        self.hwnd = hwnd as _;
        Ok(true)
    }

    fn turn_off_privacy(
        &mut self,
        conn_id: i32,
        state: Option<PrivacyModeState>,
    ) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        super::win_input::unhook()?;

        unsafe {
            let hwnd = wait_find_privacy_hwnd(0)?;
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_HIDE);
            }
        }

        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            if let Some(state) = state {
                allow_err!(super::set_privacy_mode_state(
                    conn_id,
                    state,
                    PRIVACY_MODE_IMPL.to_string(),
                    1_000
                ));
            }
            self.conn_id = INVALID_PRIVACY_MODE_CONN_ID.to_owned();
        }

        Ok(())
    }

    #[inline]
    fn pre_conn_id(&self) -> i32 {
        self.conn_id
    }

    #[inline]
    fn get_impl_key(&self) -> &str {
        &self.impl_key
    }
}

impl PrivacyModeImpl {
    pub fn new(impl_key: &str) -> Self {
        Self {
            impl_key: impl_key.to_owned(),
            conn_id: INVALID_PRIVACY_MODE_CONN_ID,
            handlers: WindowHandlers {
                hthread: 0,
                hprocess: 0,
            },
            hwnd: 0,
        }
    }

    #[inline]
    pub fn get_hwnd(&self) -> u64 {
        self.hwnd
    }

    pub fn start(&mut self) -> ResultType<()> {
        if self.handlers.hprocess != 0 {
            return Ok(());
        }

        log::info!("Start privacy mode window broker, check_update_broker_process");
        if let Err(e) = crate::platform::windows::check_update_broker_process() {
            log::warn!(
                "Failed to check update broker process. Privacy mode may not work properly. {}",
                e
            );
        }

        let exe_file = std::env::current_exe()?;
        let Some(cur_dir) = exe_file.parent() else {
            bail!("Cannot get parent of current exe file");
        };

        let dll_file = cur_dir.join("WindowInjection.dll");
        if !dll_file.exists() {
            bail!(
                "Failed to find required file {}",
                dll_file.to_string_lossy().as_ref()
            );
        }

        let hwnd = wait_find_privacy_hwnd(1_000)?;
        if !hwnd.is_null() {
            log::info!("Privacy window is ready");
            return Ok(());
        }

        // let cmdline = cur_dir.join("MiniBroker.exe").to_string_lossy().to_string();
        let cmdline = cur_dir
            .join(INJECTED_PROCESS_EXE)
            .to_string_lossy()
            .to_string();

        unsafe {
            let cmd_utf16: Vec<u16> = cmdline.encode_utf16().chain(Some(0).into_iter()).collect();

            let mut start_info = STARTUPINFOW {
                cb: 0,
                lpReserved: NULL as _,
                lpDesktop: NULL as _,
                lpTitle: NULL as _,
                dwX: 0,
                dwY: 0,
                dwXSize: 0,
                dwYSize: 0,
                dwXCountChars: 0,
                dwYCountChars: 0,
                dwFillAttribute: 0,
                dwFlags: 0,
                wShowWindow: 0,
                cbReserved2: 0,
                lpReserved2: NULL as _,
                hStdInput: NULL as _,
                hStdOutput: NULL as _,
                hStdError: NULL as _,
            };
            let mut proc_info = PROCESS_INFORMATION {
                hProcess: NULL as _,
                hThread: NULL as _,
                dwProcessId: 0,
                dwThreadId: 0,
            };

            let session_id = WTSGetActiveConsoleSessionId();
            let token = get_user_token(session_id, true);
            if token.is_null() {
                bail!("Failed to get token of current user");
            }

            let create_res = CreateProcessAsUserW(
                token,
                NULL as _,
                cmd_utf16.as_ptr() as _,
                NULL as _,
                NULL as _,
                FALSE,
                CREATE_SUSPENDED | DETACHED_PROCESS,
                NULL,
                NULL as _,
                &mut start_info,
                &mut proc_info,
            );
            CloseHandle(token);
            if 0 == create_res {
                bail!(
                    "Failed to create privacy window process {}, error {}",
                    cmdline,
                    Error::last_os_error()
                );
            };

            inject_dll(
                proc_info.hProcess,
                proc_info.hThread,
                dll_file.to_string_lossy().as_ref(),
            )?;

            if 0xffffffff == ResumeThread(proc_info.hThread) {
                // CloseHandle
                CloseHandle(proc_info.hThread);
                CloseHandle(proc_info.hProcess);

                bail!(
                    "Failed to create privacy window process, error {}",
                    Error::last_os_error()
                );
            }

            self.handlers.hthread = proc_info.hThread as _;
            self.handlers.hprocess = proc_info.hProcess as _;

            let hwnd = wait_find_privacy_hwnd(1_000)?;
            if hwnd.is_null() {
                bail!("Failed to get hwnd after started");
            }
        }

        Ok(())
    }

    #[inline]
    pub fn stop(&mut self) {
        self.handlers.reset();
    }
}

impl Drop for PrivacyModeImpl {
    fn drop(&mut self) {
        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            allow_err!(self.turn_off_privacy(self.conn_id, None));
        }
    }
}

unsafe fn inject_dll<'a>(hproc: HANDLE, hthread: HANDLE, dll_file: &'a str) -> ResultType<()> {
    let dll_file_utf16: Vec<u16> = dll_file.encode_utf16().chain(Some(0).into_iter()).collect();

    let buf = VirtualAllocEx(
        hproc,
        NULL as _,
        dll_file_utf16.len() * 2,
        MEM_COMMIT,
        PAGE_READWRITE,
    );
    if buf.is_null() {
        bail!("Failed VirtualAllocEx");
    }

    let mut written: usize = 0;
    if 0 == WriteProcessMemory(
        hproc,
        buf,
        dll_file_utf16.as_ptr() as _,
        dll_file_utf16.len() * 2,
        &mut written,
    ) {
        bail!("Failed WriteProcessMemory");
    }

    let kernel32_modulename = CString::new("kernel32")?;
    let hmodule = GetModuleHandleA(kernel32_modulename.as_ptr() as _);
    if hmodule.is_null() {
        bail!("Failed GetModuleHandleA");
    }

    let load_librarya_name = CString::new("LoadLibraryW")?;
    let load_librarya = GetProcAddress(hmodule, load_librarya_name.as_ptr() as _);
    if load_librarya.is_null() {
        bail!("Failed GetProcAddress of LoadLibraryW");
    }

    if 0 == QueueUserAPC(Some(std::mem::transmute(load_librarya)), hthread, buf as _) {
        bail!("Failed QueueUserAPC");
    }

    Ok(())
}

pub(super) fn wait_find_privacy_hwnd(msecs: u128) -> ResultType<HWND> {
    let tm_begin = Instant::now();
    let wndname = CString::new(PRIVACY_WINDOW_NAME)?;
    loop {
        unsafe {
            let hwnd = FindWindowA(NULL as _, wndname.as_ptr() as _);
            if !hwnd.is_null() {
                return Ok(hwnd);
            }
        }

        if msecs == 0 || tm_begin.elapsed().as_millis() > msecs {
            return Ok(NULL as _);
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
