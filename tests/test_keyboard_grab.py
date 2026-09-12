"""Run: python -m unittest discover -s tests -p test_keyboard_grab.py

Compile the actual shortcut with session/OS doubles, without installing hooks.
Requires rustc on PATH; no RustDesk native dependencies are needed.
"""

from pathlib import Path
import subprocess
import tempfile
import unittest


STUBS = r'''
#![allow(dead_code)]
use std::sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, RwLock};
use rdev::{Event, EventType, Key};
static KEYBOARD_HOOKED: AtomicBool = AtomicBool::new(false);
static IS_RDEV_ENABLED: AtomicBool = AtomicBool::new(true);
static HAS_SESSION: AtomicBool = AtomicBool::new(true);
static DEFAULT_SESSION: AtomicBool = AtomicBool::new(true);
static KEYBOARD_PERMISSION: AtomicBool = AtomicBool::new(true);
static VIEW_ONLY: AtomicBool = AtomicBool::new(false);
static CTRL: AtomicBool = AtomicBool::new(true);
static ALT: AtomicBool = AtomicBool::new(true);
static TRANSITIONS: AtomicUsize = AtomicUsize::new(0);
mod winapi { pub mod um { pub mod winuser {
    pub const VK_PAUSE: i32 = 0x13;
    pub const VK_CANCEL: i32 = 0x03;
} } }
mod rdev {
    #[derive(Clone, Copy, PartialEq)]
    pub enum Key { Pause, Cancel, F15, ControlLeft, ControlRight, Alt, AltGr }
    #[derive(Clone, Copy)]
    pub enum EventType { KeyPress(Key), KeyRelease(Key), Other }
    pub struct Event { pub event_type: EventType, pub platform_code: u32 }
    pub fn get_modifier(key: Key) -> bool {
        match key {
            Key::ControlLeft | Key::ControlRight => crate::CTRL.load(crate::Ordering::SeqCst),
            Key::Alt | Key::AltGr => crate::ALT.load(crate::Ordering::SeqCst),
            _ => false,
        }
    }
}
mod enigo { pub enum Key { Control, RightControl, Alt, RightAlt } }
fn get_key_state(key: enigo::Key) -> bool {
    match key {
        enigo::Key::Control | enigo::Key::RightControl => CTRL.load(Ordering::SeqCst),
        enigo::Key::Alt | enigo::Key::RightAlt => ALT.load(Ordering::SeqCst),
    }
}
mod client {
    pub fn get_modifiers_state(_: bool, _: bool, _: bool, _: bool) -> (bool, bool, bool, bool) {
        (crate::ALT.load(crate::Ordering::SeqCst), crate::CTRL.load(crate::Ordering::SeqCst), false, false)
    }
}
struct ViewOnly { v: bool }
struct LocalConfig { view_only: ViewOnly }
struct Session { server_keyboard_enabled: RwLock<bool>, lc: RwLock<LocalConfig> }
impl Session { fn is_default(&self) -> bool { DEFAULT_SESSION.load(Ordering::SeqCst) } }
mod flutter {
    use super::*;
    pub fn get_cur_session_id() -> u128 { 42 }
    pub fn get_cur_session() -> Option<Session> {
        HAS_SESSION.load(Ordering::SeqCst).then(|| Session {
            server_keyboard_enabled: RwLock::new(KEYBOARD_PERMISSION.load(Ordering::SeqCst)),
            lc: RwLock::new(LocalConfig { view_only: ViewOnly { v: VIEW_ONLY.load(Ordering::SeqCst) } }),
        })
    }
}
mod flutter_ffi {
    pub fn session_enter_or_leave(id: u128, enter: bool) {
        assert_eq!(id, 42);
        crate::KEYBOARD_HOOKED.store(enter, crate::Ordering::SeqCst);
        crate::TRANSITIONS.fetch_add(1, crate::Ordering::SeqCst);
    }
}
'''

CHECKS = r'''
fn main() {
    let press = Event { event_type: EventType::KeyPress(Key::Pause), platform_code: 0x13 };
    let release = Event { event_type: EventType::KeyRelease(Key::Pause), platform_code: 0x13 };
    for (default_session, keyboard, view_only) in [
        (true, true, false), (false, true, false), (true, false, false), (true, true, true),
    ] {
        DEFAULT_SESSION.store(default_session, Ordering::SeqCst);
        KEYBOARD_PERMISSION.store(keyboard, Ordering::SeqCst);
        VIEW_ONLY.store(view_only, Ordering::SeqCst);
        for captured in [false, true] {
            KEYBOARD_HOOKED.store(captured, Ordering::SeqCst);
            TRANSITIONS.store(0, Ordering::SeqCst);
            CTRL.store(true, Ordering::SeqCst); ALT.store(true, Ordering::SeqCst);
            let can_acquire = !cfg!(keyboard_test_linux) && default_session && keyboard && !view_only;
            let handled = captured || can_acquire;
            assert_eq!(handle_keyboard_grab_shortcut(&press), handled,
                "default={default_session}, keyboard={keyboard}, view_only={view_only}, captured={captured}");
            assert_eq!(KEYBOARD_HOOKED.load(Ordering::SeqCst), !captured && can_acquire);
            if !cfg!(keyboard_test_linux) {
                assert_eq!(handle_keyboard_grab_shortcut(&press), handled, "Repeat must not toggle capture");
            }
            CTRL.store(false, Ordering::SeqCst); ALT.store(false, Ordering::SeqCst);
            assert_eq!(handle_keyboard_grab_shortcut(&release), handled && !cfg!(keyboard_test_linux));
            assert_eq!(TRANSITIONS.load(Ordering::SeqCst), handled as usize);
        }
    }
    DEFAULT_SESSION.store(true, Ordering::SeqCst);
    VIEW_ONLY.store(false, Ordering::SeqCst);
    for (enabled, session, ctrl, alt) in [
        (false, true, true, true), (true, false, true, true),
        (true, true, false, true), (true, true, true, false),
    ] {
        IS_RDEV_ENABLED.store(enabled, Ordering::SeqCst); HAS_SESSION.store(session, Ordering::SeqCst);
        CTRL.store(ctrl, Ordering::SeqCst); ALT.store(alt, Ordering::SeqCst);
        KEYBOARD_HOOKED.store(true, Ordering::SeqCst);
        assert!(!handle_keyboard_grab_shortcut(&press));
        assert!(KEYBOARD_HOOKED.load(Ordering::SeqCst), "Unsupported paths must not change capture");
    }
}
'''


class KeyboardGrabShortcutTest(unittest.TestCase):
    def test_capture_toggle_and_session_eligibility(self):
        source = (Path(__file__).resolve().parents[1] / 'src/keyboard.rs').read_text(encoding='utf-8')
        start = source.index('fn handle_keyboard_grab_shortcut(')
        shortcut = source[start:source.index('\nfn start_grab_loop()', start)]
        for platform in ('windows', 'macos', 'linux'):
            shortcut = shortcut.replace(f'target_os = "{platform}"', f'keyboard_test_{platform}')
        with tempfile.TemporaryDirectory(prefix='rustdesk-keyboard-test-') as directory:
            source_file = Path(directory) / 'keyboard_grab.rs'
            executable = Path(directory) / 'keyboard_grab.exe'
            source_file.write_text(STUBS + shortcut + CHECKS, encoding='utf-8')
            for platform in ('windows', 'macos', 'linux'):
                with self.subTest(platform=platform):
                    result = subprocess.run(
                        ['rustc', '--edition=2021', '--cfg', f'keyboard_test_{platform}',
                         str(source_file), '-o', str(executable)], capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    result = subprocess.run([str(executable)], capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == '__main__':
    unittest.main()
