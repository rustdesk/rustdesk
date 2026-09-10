use hbb_common::{anyhow::anyhow, ResultType};
use std::{
    io::Error,
    mem::size_of,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        mpsc, OnceLock,
    },
    thread::{self, JoinHandle},
    time::Instant,
};
use winapi::{
    ctypes::c_int,
    shared::{
        minwindef::{FALSE, HMODULE, LPARAM, LRESULT, WPARAM},
        ntdef::NULL,
    },
    um::{
        libloaderapi::{
            GetModuleHandleExA, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        },
        winuser::{
            CallNextHookEx, GetMessageA, PostThreadMessageA, SendInput, SetWindowsHookExA,
            UnhookWindowsHookEx, HC_ACTION, INPUT, INPUT_MOUSE, LLMHF_INJECTED, MOUSEEVENTF_LEFTUP,
            MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_XUP, MOUSEINPUT, MSG,
            MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_QUIT, XBUTTON1, XBUTTON2,
        },
    },
};

use crate::input::{
    MOUSE_BUTTON_BACK, MOUSE_BUTTON_FORWARD, MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT,
    MOUSE_BUTTON_WHEEL, MOUSE_TYPE_DOWN, MOUSE_TYPE_UP,
};

const LOCAL_MOUSE_PRIORITY_TIMEOUT_MS: u64 = 600;

static LAST_LOCAL_MOUSE_ACTIVITY: AtomicU64 = AtomicU64::new(0);
static REMOTE_BUTTONS_DOWN: AtomicU8 = AtomicU8::new(0);
static OBSERVER_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RemoteMouseAction {
    Allow,
    Suppress,
}

pub(crate) struct LocalMouseObserver {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Drop for LocalMouseObserver {
    fn drop(&mut self) {
        let posted = unsafe { PostThreadMessageA(self.thread_id, WM_QUIT, 0, 0) };
        if posted == FALSE {
            log::error!(
                "Failed to stop local mouse observer, error: {}",
                Error::last_os_error()
            );
            return;
        }
        if let Some(thread) = self.thread.take() {
            if let Err(err) = thread.join() {
                log::error!("Failed to join local mouse observer: {:?}", err);
            }
        }
    }
}

pub(crate) fn start_local_mouse_observer() -> ResultType<LocalMouseObserver> {
    if OBSERVER_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(anyhow!("local mouse observer is already running"));
    }

    let (tx, rx) = mpsc::sync_channel(1);
    let thread = match thread::Builder::new()
        .name("local-mouse-observer".to_owned())
        .spawn(move || run_observer(tx))
    {
        Ok(thread) => thread,
        Err(err) => {
            OBSERVER_RUNNING.store(false, Ordering::Release);
            return Err(err.into());
        }
    };

    match rx.recv() {
        Ok(Ok(thread_id)) => Ok(LocalMouseObserver {
            thread_id,
            thread: Some(thread),
        }),
        Ok(Err(err)) => {
            thread.join().ok();
            Err(anyhow!(err))
        }
        Err(err) => {
            thread.join().ok();
            Err(err.into())
        }
    }
}

fn run_observer(tx: mpsc::SyncSender<Result<u32, String>>) {
    let mut module = std::ptr::null_mut() as HMODULE;
    let got_module = unsafe {
        GetModuleHandleExA(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            local_mouse_hook as _,
            &mut module,
        )
    };
    if got_module == FALSE {
        OBSERVER_RUNNING.store(false, Ordering::Release);
        tx.send(Err(format!(
            "GetModuleHandleExA failed for local mouse observer: {}",
            Error::last_os_error()
        )))
        .ok();
        return;
    }

    let hook = unsafe { SetWindowsHookExA(WH_MOUSE_LL, Some(local_mouse_hook), module, 0) };
    if hook.is_null() {
        OBSERVER_RUNNING.store(false, Ordering::Release);
        tx.send(Err(format!(
            "SetWindowsHookExA(WH_MOUSE_LL) failed: {}",
            Error::last_os_error()
        )))
        .ok();
        return;
    }

    let thread_id = unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() };
    if tx.send(Ok(thread_id)).is_err() {
        unsafe {
            UnhookWindowsHookEx(hook);
        }
        OBSERVER_RUNNING.store(false, Ordering::Release);
        return;
    }

    let mut msg = unsafe { std::mem::zeroed::<MSG>() };
    unsafe {
        while GetMessageA(&mut msg, NULL as _, 0, 0) > 0 {}
        if UnhookWindowsHookEx(hook) == FALSE {
            log::error!(
                "Failed to unhook local mouse observer, error: {}",
                Error::last_os_error()
            );
        }
    }
    OBSERVER_RUNNING.store(false, Ordering::Release);
}

extern "system" fn local_mouse_hook(code: c_int, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        let event = unsafe { &*(l_param as *const MSLLHOOKSTRUCT) };
        if is_local_physical_mouse_event(event.flags, event.dwExtraInfo) {
            note_local_mouse_activity();
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
}

#[inline]
fn monotonic_millis() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .saturating_add(1) as u64
}

#[inline]
fn is_local_physical_mouse_event(flags: u32, extra_info: usize) -> bool {
    // LLMHF_INJECTED also covers lower-integrity injected input. Keep RustDesk's marker as a
    // second guard so its own SendInput events can never refresh the priority window.
    flags & LLMHF_INJECTED == 0 && extra_info != enigo::ENIGO_INPUT_EXTRA_VALUE
}

#[inline]
fn local_mouse_has_priority_at(last_activity: u64, now: u64, timeout_ms: u64) -> bool {
    last_activity != 0 && now.wrapping_sub(last_activity) < timeout_ms
}

fn note_local_mouse_activity() {
    let buttons_to_release = note_local_mouse_activity_at(
        monotonic_millis(),
        &LAST_LOCAL_MOUSE_ACTIVITY,
        &REMOTE_BUTTONS_DOWN,
    );
    if buttons_to_release != 0 {
        // Release before the physical event leaves the hook so local movement cannot continue a
        // drag that a remote peer started.
        release_remote_buttons(buttons_to_release);
    }
}

fn note_local_mouse_activity_at(
    now: u64,
    last_activity: &AtomicU64,
    remote_buttons_down: &AtomicU8,
) -> u8 {
    last_activity.store(now, Ordering::Relaxed);
    remote_buttons_down.swap(0, Ordering::AcqRel)
}

fn release_remote_buttons(buttons: u8) {
    unsafe {
        if buttons & MOUSE_BUTTON_LEFT as u8 != 0 {
            send_mouse_button_up(MOUSEEVENTF_LEFTUP, 0);
        }
        if buttons & MOUSE_BUTTON_RIGHT as u8 != 0 {
            send_mouse_button_up(MOUSEEVENTF_RIGHTUP, 0);
        }
        if buttons & MOUSE_BUTTON_WHEEL as u8 != 0 {
            send_mouse_button_up(MOUSEEVENTF_MIDDLEUP, 0);
        }
        if buttons & MOUSE_BUTTON_BACK as u8 != 0 {
            send_mouse_button_up(MOUSEEVENTF_XUP, XBUTTON1 as u32);
        }
        if buttons & MOUSE_BUTTON_FORWARD as u8 != 0 {
            send_mouse_button_up(MOUSEEVENTF_XUP, XBUTTON2 as u32);
        }
    }
}

unsafe fn send_mouse_button_up(flags: u32, data: u32) {
    let mut input = std::mem::zeroed::<INPUT>();
    input.type_ = INPUT_MOUSE;
    *input.u.mi_mut() = MOUSEINPUT {
        dx: 0,
        dy: 0,
        mouseData: data,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: enigo::ENIGO_INPUT_EXTRA_VALUE,
    };
    if SendInput(1, &mut input, size_of::<INPUT>() as c_int) == 0 {
        log::error!(
            "Failed to release remote mouse button during local takeover, error: {}",
            Error::last_os_error()
        );
    }
}

#[inline]
pub(crate) fn arbitrate_remote_mouse() -> RemoteMouseAction {
    arbitrate_remote_mouse_at(local_mouse_has_priority())
}

fn local_mouse_has_priority() -> bool {
    local_mouse_has_priority_at(
        LAST_LOCAL_MOUSE_ACTIVITY.load(Ordering::Relaxed),
        monotonic_millis(),
        LOCAL_MOUSE_PRIORITY_TIMEOUT_MS,
    )
}

pub(crate) fn record_remote_mouse_injected(evt_type: i32, buttons: i32) {
    let buttons_to_release = record_remote_mouse_injected_with(
        evt_type,
        buttons,
        local_mouse_has_priority,
        &REMOTE_BUTTONS_DOWN,
    );
    if buttons_to_release != 0 {
        release_remote_buttons(buttons_to_release);
    }
}

fn record_remote_mouse_injected_with<F: FnOnce() -> bool>(
    evt_type: i32,
    buttons: i32,
    local_has_priority: F,
    remote_buttons_down: &AtomicU8,
) -> u8 {
    let buttons = buttons as u8;
    if evt_type == MOUSE_TYPE_DOWN {
        remote_buttons_down.fetch_or(buttons, Ordering::AcqRel);
        if local_has_priority() {
            return remote_buttons_down.swap(0, Ordering::AcqRel);
        }
    } else if evt_type == MOUSE_TYPE_UP {
        remote_buttons_down.fetch_and(!buttons, Ordering::AcqRel);
    }
    0
}

fn arbitrate_remote_mouse_at(local_has_priority: bool) -> RemoteMouseAction {
    if local_has_priority {
        RemoteMouseAction::Suppress
    } else {
        RemoteMouseAction::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT};

    #[test]
    fn local_priority_window_and_refresh_are_deterministic() {
        assert!(!local_mouse_has_priority_at(0, 100, 600));
        assert!(local_mouse_has_priority_at(100, 699, 600));
        assert!(!local_mouse_has_priority_at(100, 700, 600));
        assert!(local_mouse_has_priority_at(500, 1_099, 600));
        assert!(!local_mouse_has_priority_at(500, 1_100, 600));
    }

    #[test]
    fn only_hardware_events_count_as_local_activity() {
        assert!(is_local_physical_mouse_event(0, 0));
        assert!(!is_local_physical_mouse_event(
            LLMHF_INJECTED,
            enigo::ENIGO_INPUT_EXTRA_VALUE
        ));
        assert!(!is_local_physical_mouse_event(LLMHF_INJECTED, 0));
    }

    #[test]
    fn remote_input_is_allowed_without_local_activity() {
        assert_eq!(arbitrate_remote_mouse_at(false), RemoteMouseAction::Allow);
    }

    #[test]
    fn local_takeover_suppresses_motion_wheel_and_new_down() {
        assert_eq!(arbitrate_remote_mouse_at(true), RemoteMouseAction::Suppress);
    }

    #[test]
    fn local_takeover_releases_all_remote_buttons() {
        let buttons = AtomicU8::new(0);
        assert_eq!(
            record_remote_mouse_injected_with(
                MOUSE_TYPE_DOWN,
                MOUSE_BUTTON_LEFT,
                || false,
                &buttons,
            ),
            0
        );
        assert_eq!(
            record_remote_mouse_injected_with(
                MOUSE_TYPE_DOWN,
                MOUSE_BUTTON_RIGHT,
                || false,
                &buttons,
            ),
            0
        );
        let last_activity = AtomicU64::new(0);
        assert_eq!(
            note_local_mouse_activity_at(100, &last_activity, &buttons),
            (MOUSE_BUTTON_LEFT | MOUSE_BUTTON_RIGHT) as u8
        );
        assert_eq!(last_activity.load(Ordering::Relaxed), 100);
        assert_eq!(buttons.load(Ordering::Relaxed), 0);
        assert_eq!(
            record_remote_mouse_injected_with(MOUSE_TYPE_UP, MOUSE_BUTTON_LEFT, || true, &buttons),
            0
        );
    }

    #[test]
    fn down_racing_with_local_takeover_is_immediately_released() {
        let buttons = AtomicU8::new(0);
        assert_eq!(
            record_remote_mouse_injected_with(
                MOUSE_TYPE_DOWN,
                MOUSE_BUTTON_LEFT,
                || true,
                &buttons,
            ),
            MOUSE_BUTTON_LEFT as u8
        );
        assert_eq!(buttons.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn priority_is_global_across_remote_peers() {
        assert_eq!(arbitrate_remote_mouse_at(true), RemoteMouseAction::Suppress);
        assert_eq!(arbitrate_remote_mouse_at(true), RemoteMouseAction::Suppress);
    }
}
