use crate::{
    ipc::{connect, Data, PrivacyModeState},
    platform::windows::get_user_token,
};
use hbb_common::{allow_err, bail, lazy_static, log, tokio, ResultType};
use std::{
    ffi::CString,
    os::windows::process::CommandExt,
    sync::Mutex,
    time::{Duration, Instant},
};
use winapi::{
    ctypes::c_int,
    shared::{
        minwindef::{DWORD, FALSE, HMODULE, LOBYTE, LPARAM, LRESULT, UINT, WPARAM},
        ntdef::{HANDLE, NULL},
        windef::{HHOOK, HWND, POINT},
    },
    um::{
        errhandlingapi::GetLastError,
        handleapi::CloseHandle,
        libloaderapi::{GetModuleHandleA, GetModuleHandleExA, GetProcAddress},
        memoryapi::{VirtualAllocEx, WriteProcessMemory},
        processthreadsapi::{
            CreateProcessAsUserW, GetCurrentThreadId, QueueUserAPC, ResumeThread,
            PROCESS_INFORMATION, STARTUPINFOW,
        },
        winbase::{
            WTSGetActiveConsoleSessionId, CREATE_NO_WINDOW, CREATE_SUSPENDED, DETACHED_PROCESS,
        },
        winnt::{MEM_COMMIT, PAGE_READWRITE},
        winuser::*,
    },
};

pub const ORIGIN_PROCESS_EXE: &'static str = "C:\\Windows\\System32\\RuntimeBroker.exe";
pub const INJECTED_PROCESS_EXE: &'static str = "RuntimeBroker_rustdesk.exe";
pub const PRIVACY_WINDOW_NAME: &'static str = "RustDeskPrivacyWindow";

pub const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 2;
pub const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 4;

const WM_USER_EXIT_HOOK: u32 = WM_USER + 1;

lazy_static::lazy_static! {
    static ref DLL_FOUND: Mutex<bool> = Mutex::new(false);
    static ref CONN_ID: Mutex<i32> = Mutex::new(0);
    static ref CUR_HOOK_THREAD_ID: Mutex<DWORD> = Mutex::new(0);
    static ref WND_HANDLERS: Mutex<WindowHandlers> = Mutex::new(WindowHandlers{hthread: 0, hprocess: 0});
}

struct WindowHandlers {
    hthread: u64,
    hprocess: u64,
}

impl Drop for WindowHandlers {
    fn drop(&mut self) {
        unsafe {
            if self.hthread != 0 {
                CloseHandle(self.hthread as _);
            }
            self.hthread = 0;
            if self.hprocess != 0 {
                CloseHandle(self.hprocess as _);
            }
            self.hprocess = 0;
        }
    }
}

pub fn turn_on_privacy(conn_id: i32) -> ResultType<bool> {
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

    if !*DLL_FOUND.lock().unwrap() {
        log::info!("turn_on_privacy, dll not found when started, try start");
        start()?;
        std::thread::sleep(std::time::Duration::from_millis(1_000));
    }

    let pre_conn_id = *CONN_ID.lock().unwrap();
    if pre_conn_id == conn_id {
        return Ok(true);
    }
    if pre_conn_id != 0 {
        bail!("Privacy occupied by another one");
    }

    let hwnd = wait_find_privacy_hwnd(0)?;
    if hwnd.is_null() {
        bail!("No privacy window created");
    }
    privacy_hook::hook()?;
    unsafe {
        ShowWindow(hwnd as _, SW_SHOW);
    }
    *CONN_ID.lock().unwrap() = conn_id;
    Ok(true)
}

pub fn turn_off_privacy(conn_id: i32, state: Option<PrivacyModeState>) -> ResultType<()> {
    let pre_conn_id = *CONN_ID.lock().unwrap();
    if pre_conn_id != 0 && conn_id != 0 && pre_conn_id != conn_id {
        bail!("Failed to turn off privacy mode that belongs to someone else")
    }

    privacy_hook::unhook()?;

    unsafe {
        let hwnd = wait_find_privacy_hwnd(0)?;
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    if pre_conn_id != 0 {
        if let Some(state) = state {
            allow_err!(set_privacy_mode_state(pre_conn_id, state, 1_000));
        }
        *CONN_ID.lock().unwrap() = 0;
    }

    Ok(())
}

pub fn start() -> ResultType<()> {
    let mut wnd_handlers = WND_HANDLERS.lock().unwrap();
    if wnd_handlers.hprocess != 0 {
        return Ok(());
    }

    let exe_file = std::env::current_exe()?;
    if exe_file.parent().is_none() {
        bail!("Cannot get parent of current exe file");
    }
    let cur_dir = exe_file.parent().unwrap();

    let dll_file = cur_dir.join("WindowInjection.dll");
    if !dll_file.exists() {
        bail!(
            "Failed to find required file {}",
            dll_file.to_string_lossy().as_ref()
        );
    }

    *DLL_FOUND.lock().unwrap() = true;

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
                "Failed to create privacy window process {}, code {}",
                cmdline,
                GetLastError()
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
                "Failed to create privacy window process, {}",
                GetLastError()
            );
        }

        wnd_handlers.hthread = proc_info.hThread as _;
        wnd_handlers.hprocess = proc_info.hProcess as _;

        let hwnd = wait_find_privacy_hwnd(30_000)?;
        if hwnd.is_null() {
            bail!("Failed to get hwnd after started");
        }
    }

    Ok(())
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

fn wait_find_privacy_hwnd(msecs: u128) -> ResultType<HWND> {
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

pub fn is_process_consent_running() -> ResultType<bool> {
    let output = std::process::Command::new("cmd")
        .args(&["/C", "tasklist | findstr consent.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    Ok(output.status.success() && !output.stdout.is_empty())
}

#[tokio::main(flavor = "current_thread")]
async fn set_privacy_mode_state(
    conn_id: i32,
    state: PrivacyModeState,
    ms_timeout: u64,
) -> ResultType<()> {
    let mut c = connect(ms_timeout, "_cm").await?;
    c.send(&Data::PrivacyModeState((conn_id, state))).await
}

pub(super) mod privacy_hook {
    use super::*;
    use std::sync::mpsc::{channel, Sender};

    fn do_hook(tx: Sender<String>) -> ResultType<(HHOOK, HHOOK)> {
        let invalid_ret = (0 as HHOOK, 0 as HHOOK);

        let mut cur_hook_thread_id = CUR_HOOK_THREAD_ID.lock().unwrap();
        if *cur_hook_thread_id != 0 {
            // unreachable!
            tx.send("Already hooked".to_owned())?;
            return Ok(invalid_ret);
        }

        unsafe {
            let mut hm_keyboard = 0 as HMODULE;
            if 0 == GetModuleHandleExA(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                DefWindowProcA as _,
                &mut hm_keyboard as _,
            ) {
                tx.send(format!(
                    "Failed to GetModuleHandleExA, error: {}",
                    GetLastError()
                ))?;
                return Ok(invalid_ret);
            }
            let mut hm_mouse = 0 as HMODULE;
            if 0 == GetModuleHandleExA(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                DefWindowProcA as _,
                &mut hm_mouse as _,
            ) {
                tx.send(format!(
                    "Failed to GetModuleHandleExA, error: {}",
                    GetLastError()
                ))?;
                return Ok(invalid_ret);
            }

            let hook_keyboard = SetWindowsHookExA(
                WH_KEYBOARD_LL,
                Some(privacy_mode_hook_keyboard),
                hm_keyboard,
                0,
            );
            if hook_keyboard.is_null() {
                tx.send(format!(" SetWindowsHookExA keyboard {}", GetLastError()))?;
                return Ok(invalid_ret);
            }

            let hook_mouse =
                SetWindowsHookExA(WH_MOUSE_LL, Some(privacy_mode_hook_mouse), hm_mouse, 0);
            if hook_mouse.is_null() {
                if FALSE == UnhookWindowsHookEx(hook_keyboard) {
                    // Fatal error
                    log::error!(" UnhookWindowsHookEx keyboard {}", GetLastError());
                }
                tx.send(format!(" SetWindowsHookExA mouse {}", GetLastError()))?;
                return Ok(invalid_ret);
            }

            *cur_hook_thread_id = GetCurrentThreadId();
            tx.send("".to_owned())?;
            return Ok((hook_keyboard, hook_mouse));
        }
    }

    pub fn hook() -> ResultType<()> {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let hook_keyboard;
            let hook_mouse;
            unsafe {
                match do_hook(tx.clone()) {
                    Ok(hooks) => {
                        hook_keyboard = hooks.0;
                        hook_mouse = hooks.1;
                    }
                    Err(e) => {
                        // Fatal error
                        tx.send(format!("Unexpected err when hook {}", e)).unwrap();
                        return;
                    }
                }
                if hook_keyboard.is_null() {
                    return;
                }

                let mut msg = MSG {
                    hwnd: NULL as _,
                    message: 0 as _,
                    wParam: 0 as _,
                    lParam: 0 as _,
                    time: 0 as _,
                    pt: POINT {
                        x: 0 as _,
                        y: 0 as _,
                    },
                };
                while FALSE != GetMessageA(&mut msg, NULL as _, 0, 0) {
                    if msg.message == WM_USER_EXIT_HOOK {
                        break;
                    }

                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                }

                if FALSE == UnhookWindowsHookEx(hook_keyboard as _) {
                    // Fatal error
                    log::error!("Failed UnhookWindowsHookEx keyboard {}", GetLastError());
                }

                if FALSE == UnhookWindowsHookEx(hook_mouse as _) {
                    // Fatal error
                    log::error!("Failed UnhookWindowsHookEx mouse {}", GetLastError());
                }

                *CUR_HOOK_THREAD_ID.lock().unwrap() = 0;
            }
        });

        match rx.recv() {
            Ok(msg) => {
                if msg == "" {
                    Ok(())
                } else {
                    bail!(msg)
                }
            }
            Err(e) => {
                bail!("Failed to wait hook result {}", e)
            }
        }
    }

    pub fn unhook() -> ResultType<()> {
        unsafe {
            let cur_hook_thread_id = CUR_HOOK_THREAD_ID.lock().unwrap();
            if *cur_hook_thread_id != 0 {
                if FALSE == PostThreadMessageA(*cur_hook_thread_id, WM_USER_EXIT_HOOK, 0, 0) {
                    bail!("Failed to post message to exit hook, {}", GetLastError());
                }
            }
        }
        Ok(())
    }

    #[no_mangle]
    pub extern "system" fn privacy_mode_hook_keyboard(
        code: c_int,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code < 0 {
            unsafe {
                return CallNextHookEx(NULL as _, code, w_param, l_param);
            }
        }

        let ks = l_param as PKBDLLHOOKSTRUCT;
        let w_param2 = w_param as UINT;

        unsafe {
            if (*ks).dwExtraInfo != enigo::ENIGO_INPUT_EXTRA_VALUE {
                // Disable alt key. Alt + Tab will switch windows.
                if (*ks).flags & LLKHF_ALTDOWN == LLKHF_ALTDOWN {
                    return 1;
                }

                match w_param2 {
                    WM_KEYDOWN => {
                        // Disable all keys other than P and Ctrl.
                        if ![80, 162, 163].contains(&(*ks).vkCode) {
                            return 1;
                        }

                        // NOTE: GetKeyboardState may not work well...

                        // Check if Ctrl + P is pressed
                        let cltr_down = (GetKeyState(VK_CONTROL) as u16) & (0x8000 as u16) > 0;
                        let key = LOBYTE((*ks).vkCode as _);
                        if cltr_down && (key == 'p' as u8 || key == 'P' as u8) {
                            // Ctrl + P is pressed, turn off privacy mode
                            if let Err(e) =
                                turn_off_privacy(0, Some(crate::ipc::PrivacyModeState::OffByPeer))
                            {
                                log::error!("Failed to off_privacy {}", e);
                            }
                        }
                    }
                    WM_KEYUP => {
                        log::trace!("WM_KEYUP {}", (*ks).vkCode);
                    }
                    _ => {
                        log::trace!("KEYBOARD OTHER {} {}", w_param2, (*ks).vkCode);
                    }
                }
            }
        }
        unsafe { CallNextHookEx(NULL as _, code, w_param, l_param) }
    }

    #[no_mangle]
    pub extern "system" fn privacy_mode_hook_mouse(
        code: c_int,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code < 0 {
            unsafe {
                return CallNextHookEx(NULL as _, code, w_param, l_param);
            }
        }

        let ms = l_param as PMOUSEHOOKSTRUCT;
        unsafe {
            if (*ms).dwExtraInfo != enigo::ENIGO_INPUT_EXTRA_VALUE {
                return 1;
            }
        }
        unsafe { CallNextHookEx(NULL as _, code, w_param, l_param) }
    }
}

mod test {
    #[test]
    fn privacy_hook() {
        //use super::*;

        // privacy_hook::hook().unwrap();
        // std::thread::sleep(std::time::Duration::from_millis(50));
        // privacy_hook::unhook().unwrap();
    }
}
