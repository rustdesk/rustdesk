use hbb_common::{allow_err, bail, lazy_static, log, ResultType};
use std::{
    io::Error,
    sync::{
        mpsc::{channel, Sender},
        Mutex,
    },
};
use winapi::{
    ctypes::c_int,
    shared::{
        minwindef::{DWORD, FALSE, HMODULE, LOBYTE, LPARAM, LRESULT, UINT, WPARAM},
        ntdef::NULL,
        windef::{HHOOK, POINT},
    },
    um::{libloaderapi::GetModuleHandleExA, processthreadsapi::GetCurrentThreadId, winuser::*},
};

const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 2;
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 4;

const WM_USER_EXIT_HOOK: u32 = WM_USER + 1;

lazy_static::lazy_static! {
    static ref CUR_HOOK_THREAD_ID: Mutex<DWORD> = Mutex::new(0);
}

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
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            DefWindowProcA as _,
            &mut hm_keyboard as _,
        ) {
            tx.send(format!(
                "Failed to GetModuleHandleExA, error: {}",
                Error::last_os_error()
            ))?;
            return Ok(invalid_ret);
        }
        let mut hm_mouse = 0 as HMODULE;
        if 0 == GetModuleHandleExA(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            DefWindowProcA as _,
            &mut hm_mouse as _,
        ) {
            tx.send(format!(
                "Failed to GetModuleHandleExA, error: {}",
                Error::last_os_error()
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
            tx.send(format!(
                " SetWindowsHookExA keyboard, error {}",
                Error::last_os_error()
            ))?;
            return Ok(invalid_ret);
        }

        let hook_mouse = SetWindowsHookExA(WH_MOUSE_LL, Some(privacy_mode_hook_mouse), hm_mouse, 0);
        if hook_mouse.is_null() {
            if FALSE == UnhookWindowsHookEx(hook_keyboard) {
                // Fatal error
                log::error!(
                    " UnhookWindowsHookEx keyboard, error {}",
                    Error::last_os_error()
                );
            }
            tx.send(format!(
                " SetWindowsHookExA mouse, error {}",
                Error::last_os_error()
            ))?;
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
                    allow_err!(tx.send(format!("Unexpected err when hook {}", e)));
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
                log::error!(
                    "Failed UnhookWindowsHookEx keyboard, error {}",
                    Error::last_os_error()
                );
            }

            if FALSE == UnhookWindowsHookEx(hook_mouse as _) {
                // Fatal error
                log::error!(
                    "Failed UnhookWindowsHookEx mouse, error {}",
                    Error::last_os_error()
                );
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
                bail!(
                    "Failed to post message to exit hook, error {}",
                    Error::last_os_error()
                );
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
                        if let Some(Err(e)) = super::turn_off_privacy(
                            super::INVALID_PRIVACY_MODE_CONN_ID,
                            Some(super::PrivacyModeState::OffByPeer),
                        ) {
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

mod test {
    #[test]
    fn privacy_hook() {
        //use super::*;

        // privacy_hook::hook().unwrap();
        // std::thread::sleep(std::time::Duration::from_millis(50));
        // privacy_hook::unhook().unwrap();
    }
}
