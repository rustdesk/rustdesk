use super::input_service::set_clipboard_for_paste_sync;
use crate::uinput::service::{can_input_via_keysym, char_to_keysym, map_key};
use dbus::{blocking::SyncConnection, Path};
use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::{log, ResultType};
use scrap::wayland::pipewire::{get_portal, PwStreamInfo, RdpInputBackend};
use scrap::wayland::remote_desktop_portal::OrgFreedesktopPortalRemoteDesktop as remote_desktop_portal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub mod client {
    use hbb_common::platform::linux::{DISPLAY_DESKTOP_KDE, XDG_CURRENT_DESKTOP};

    use super::*;

    const EVDEV_MOUSE_LEFT: i32 = 272;
    const EVDEV_MOUSE_RIGHT: i32 = 273;
    const EVDEV_MOUSE_MIDDLE: i32 = 274;

    const PRESSED_DOWN_STATE: u32 = 1;
    const PRESSED_UP_STATE: u32 = 0;

    const MUTTER_REMOTE_DESKTOP_NAME: &str = "org.gnome.Mutter.RemoteDesktop";
    const MUTTER_REMOTE_DESKTOP_SESSION_IFACE: &str = "org.gnome.Mutter.RemoteDesktop.Session";
    const MUTTER_POINTER_MOTION_RELATIVE_METHOD: &str = "NotifyPointerMotionRelative";
    const DBUS_TIMEOUT: Duration = Duration::from_secs(3);

    /// Modifier key state tracking for RDP input.
    /// Portal API doesn't provide a way to query key state, so we track it ourselves.
    #[derive(Default)]
    struct ModifierState {
        shift_left: bool,
        shift_right: bool,
        ctrl_left: bool,
        ctrl_right: bool,
        alt_left: bool,
        alt_right: bool,
        meta_left: bool,
        meta_right: bool,
    }

    impl ModifierState {
        fn update(&mut self, key: &Key, down: bool) {
            match key {
                Key::Shift => self.shift_left = down,
                Key::RightShift => self.shift_right = down,
                Key::Control => self.ctrl_left = down,
                Key::RightControl => self.ctrl_right = down,
                Key::Alt => self.alt_left = down,
                Key::RightAlt => self.alt_right = down,
                Key::Meta | Key::Super | Key::Windows | Key::Command => self.meta_left = down,
                Key::RWin => self.meta_right = down,
                // Handle raw keycodes for modifier keys (Linux evdev codes + 8)
                // In translate mode, modifier keys may be sent as Chr events with raw keycodes.
                // The +8 offset converts evdev codes to X11/XKB keycodes.
                Key::Raw(code) => {
                    const EVDEV_OFFSET: u16 = 8;
                    const KEY_LEFTSHIFT: u16 = evdev::Key::KEY_LEFTSHIFT.code() + EVDEV_OFFSET;
                    const KEY_RIGHTSHIFT: u16 = evdev::Key::KEY_RIGHTSHIFT.code() + EVDEV_OFFSET;
                    const KEY_LEFTCTRL: u16 = evdev::Key::KEY_LEFTCTRL.code() + EVDEV_OFFSET;
                    const KEY_RIGHTCTRL: u16 = evdev::Key::KEY_RIGHTCTRL.code() + EVDEV_OFFSET;
                    const KEY_LEFTALT: u16 = evdev::Key::KEY_LEFTALT.code() + EVDEV_OFFSET;
                    const KEY_RIGHTALT: u16 = evdev::Key::KEY_RIGHTALT.code() + EVDEV_OFFSET;
                    const KEY_LEFTMETA: u16 = evdev::Key::KEY_LEFTMETA.code() + EVDEV_OFFSET;
                    const KEY_RIGHTMETA: u16 = evdev::Key::KEY_RIGHTMETA.code() + EVDEV_OFFSET;
                    match *code {
                        KEY_LEFTSHIFT => self.shift_left = down,
                        KEY_RIGHTSHIFT => self.shift_right = down,
                        KEY_LEFTCTRL => self.ctrl_left = down,
                        KEY_RIGHTCTRL => self.ctrl_right = down,
                        KEY_LEFTALT => self.alt_left = down,
                        KEY_RIGHTALT => self.alt_right = down,
                        KEY_LEFTMETA => self.meta_left = down,
                        KEY_RIGHTMETA => self.meta_right = down,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub struct RdpInputKeyboard {
        conn: Arc<SyncConnection>,
        session: Path<'static>,
        backend: RdpInputBackend,
        modifier_state: ModifierState,
    }

    impl RdpInputKeyboard {
        pub fn new(
            conn: Arc<SyncConnection>,
            session: Path<'static>,
            backend: RdpInputBackend,
        ) -> ResultType<Self> {
            Ok(Self {
                conn,
                session,
                backend,
                modifier_state: ModifierState::default(),
            })
        }
    }

    impl KeyboardControllable for RdpInputKeyboard {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn get_key_state(&mut self, key: Key) -> bool {
            // Use tracked modifier state for supported keys
            match key {
                Key::Shift => self.modifier_state.shift_left,
                Key::RightShift => self.modifier_state.shift_right,
                Key::Control => self.modifier_state.ctrl_left,
                Key::RightControl => self.modifier_state.ctrl_right,
                Key::Alt => self.modifier_state.alt_left,
                Key::RightAlt => self.modifier_state.alt_right,
                Key::Meta | Key::Super | Key::Windows | Key::Command => {
                    self.modifier_state.meta_left
                }
                Key::RWin => self.modifier_state.meta_right,
                _ => false,
            }
        }

        fn key_sequence(&mut self, s: &str) {
            if s.is_empty() {
                return;
            }

            // Keep ordering deterministic:
            // - pure ASCII printable: send via Portal keysym
            // - any non-ASCII present (including mixed ASCII/non-ASCII): send whole
            //   sequence via clipboard as one atomic paste
            let ascii_only = s.chars().all(|c| {
                let keysym = char_to_keysym(c);
                can_input_via_keysym(c, keysym)
            });
            if !ascii_only {
                input_text_via_clipboard(s, self.conn.clone(), &self.session, self.backend);
                return;
            }

            for c in s.chars() {
                let keysym = char_to_keysym(c);
                // ASCII characters: use keysym
                if can_input_via_keysym(c, keysym) {
                    if let Err(e) =
                        send_keysym(keysym, true, self.conn.clone(), &self.session, self.backend)
                    {
                        log::error!("Failed to send keysym down: {:?}", e);
                    }
                    if let Err(e) = send_keysym(
                        keysym,
                        false,
                        self.conn.clone(),
                        &self.session,
                        self.backend,
                    ) {
                        log::error!("Failed to send keysym up: {:?}", e);
                    }
                }
            }
        }

        fn key_down(&mut self, key: Key) -> enigo::ResultType {
            if let Key::Layout(chr) = key {
                let keysym = char_to_keysym(chr);
                // ASCII characters: use keysym
                if can_input_via_keysym(chr, keysym) {
                    send_keysym(keysym, true, self.conn.clone(), &self.session, self.backend)?;
                } else {
                    // Non-ASCII: use clipboard (complete key press in key_down)
                    input_text_via_clipboard(
                        &chr.to_string(),
                        self.conn.clone(),
                        &self.session,
                        self.backend,
                    );
                }
            } else {
                handle_key(
                    true,
                    key.clone(),
                    self.conn.clone(),
                    &self.session,
                    self.backend,
                )?;
                // Update modifier state only after successful send —
                // if handle_key fails, we don't want stale "pressed" state
                // affecting subsequent key event decisions.
                self.modifier_state.update(&key, true);
            }
            Ok(())
        }

        fn key_up(&mut self, key: Key) {
            // Intentionally asymmetric with key_down: update state BEFORE sending.
            // On release, we always mark as released even if the send fails below,
            // to avoid permanently stuck-modifier state in our tracker. The trade-off
            // (tracker says "released" while OS may still have it pressed) is acceptable
            // because such failures are rare and subsequent events will resynchronize.
            self.modifier_state.update(&key, false);

            if let Key::Layout(chr) = key {
                // ASCII characters: send keysym up if we also sent it on key_down
                let keysym = char_to_keysym(chr);
                if can_input_via_keysym(chr, keysym) {
                    if let Err(e) = send_keysym(
                        keysym,
                        false,
                        self.conn.clone(),
                        &self.session,
                        self.backend,
                    ) {
                        log::error!("Failed to send keysym up: {:?}", e);
                    }
                }
                // Non-ASCII: already handled completely in key_down via clipboard paste,
                // no corresponding release needed (clipboard paste is an atomic operation)
            } else {
                if let Err(e) =
                    handle_key(false, key, self.conn.clone(), &self.session, self.backend)
                {
                    log::error!("Failed to handle key up: {:?}", e);
                }
            }
        }

        fn key_click(&mut self, key: Key) {
            if let Key::Layout(chr) = key {
                let keysym = char_to_keysym(chr);
                // ASCII characters: use keysym
                if can_input_via_keysym(chr, keysym) {
                    if let Err(e) =
                        send_keysym(keysym, true, self.conn.clone(), &self.session, self.backend)
                    {
                        log::error!("Failed to send keysym down: {:?}", e);
                    }
                    if let Err(e) = send_keysym(
                        keysym,
                        false,
                        self.conn.clone(),
                        &self.session,
                        self.backend,
                    ) {
                        log::error!("Failed to send keysym up: {:?}", e);
                    }
                } else {
                    // Non-ASCII: use clipboard
                    input_text_via_clipboard(
                        &chr.to_string(),
                        self.conn.clone(),
                        &self.session,
                        self.backend,
                    );
                }
            } else {
                if let Err(e) = handle_key(
                    true,
                    key.clone(),
                    self.conn.clone(),
                    &self.session,
                    self.backend,
                ) {
                    log::error!("Failed to handle key down: {:?}", e);
                } else {
                    // Only mark modifier as pressed if key-down was actually delivered
                    self.modifier_state.update(&key, true);
                }
                // Always mark as released to avoid stuck-modifier state
                self.modifier_state.update(&key, false);
                if let Err(e) =
                    handle_key(false, key, self.conn.clone(), &self.session, self.backend)
                {
                    log::error!("Failed to handle key up: {:?}", e);
                }
            }
        }
    }

    /// Input text via clipboard + Shift+Insert.
    /// Shift+Insert is more universal than Ctrl+V, works in both GUI apps and terminals.
    ///
    /// Note: Clipboard content is NOT restored after paste - see `set_clipboard_for_paste_sync` for rationale.
    fn input_text_via_clipboard(
        text: &str,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
        backend: RdpInputBackend,
    ) {
        if text.is_empty() {
            return;
        }
        if !set_clipboard_for_paste_sync(text) {
            return;
        }

        let shift_keycode = evdev::Key::KEY_LEFTSHIFT.code() as i32;
        let insert_keycode = evdev::Key::KEY_INSERT.code() as i32;

        // Send Shift+Insert (universal paste shortcut)
        if let Err(e) = notify_keyboard_keycode(backend, &conn, session, shift_keycode, true) {
            log::error!("input_text_via_clipboard: failed to press Shift: {:?}", e);
            return;
        }

        // Press Insert
        if let Err(e) = notify_keyboard_keycode(backend, &conn, session, insert_keycode, true) {
            log::error!("input_text_via_clipboard: failed to press Insert: {:?}", e);
            // Still try to release Shift.
            // Note: clipboard has already been set by set_clipboard_for_paste_sync but paste
            // never happened. We don't attempt to restore the previous clipboard contents
            // because reading the clipboard on Wayland requires focus/permission.
            if let Err(release_err) =
                notify_keyboard_keycode(backend, &conn, session, shift_keycode, false)
            {
                log::warn!(
                    "input_text_via_clipboard: failed to release Shift after Insert error: {:?}",
                    release_err
                );
            }
            return;
        }

        // Release Insert
        if let Err(e) = notify_keyboard_keycode(backend, &conn, session, insert_keycode, false) {
            log::error!(
                "input_text_via_clipboard: failed to release Insert: {:?}",
                e
            );
        }

        // Release Shift
        if let Err(e) = notify_keyboard_keycode(backend, &conn, session, shift_keycode, false) {
            log::error!("input_text_via_clipboard: failed to release Shift: {:?}", e);
        }
    }

    fn desktop_is_niri(desktop: &str) -> bool {
        desktop
            .split(':')
            .any(|name| name.eq_ignore_ascii_case("niri"))
    }

    lazy_static::lazy_static! {
        static ref SHOULD_SCALE_POINTER_COORDINATES: bool =
            std::env::var(XDG_CURRENT_DESKTOP)
                .map(|desktop| desktop == DISPLAY_DESKTOP_KDE || desktop_is_niri(&desktop))
                .unwrap_or(false);
    }

    fn select_stream_rect(
        rects: impl IntoIterator<Item = ((i32, i32), (usize, usize))>,
        x: f64,
        y: f64,
    ) -> Option<(usize, f64, f64)> {
        let mut first = None;
        for (index, (position, size)) in rects.into_iter().enumerate() {
            let local_x = (x - position.0 as f64).clamp(0.0, size.0.saturating_sub(1) as f64);
            let local_y = (y - position.1 as f64).clamp(0.0, size.1.saturating_sub(1) as f64);
            first.get_or_insert((index, local_x, local_y));
            if x >= position.0 as f64
                && y >= position.1 as f64
                && x < position.0 as f64 + size.0 as f64
                && y < position.1 as f64 + size.1 as f64
            {
                return Some((index, local_x, local_y));
            }
        }
        first
    }

    pub struct RdpInputMouse {
        conn: Arc<SyncConnection>,
        session: Path<'static>,
        backend: RdpInputBackend,
        streams: Vec<PwStreamInfo>,
        scale: Option<f64>,
        position: (f64, f64),
        motion_error_logged: bool,
    }

    impl RdpInputMouse {
        pub fn new(
            conn: Arc<SyncConnection>,
            session: Path<'static>,
            backend: RdpInputBackend,
            streams: Vec<PwStreamInfo>,
            resolution: (usize, usize),
        ) -> ResultType<Self> {
            let stream = streams
                .first()
                .ok_or_else(|| hbb_common::anyhow::anyhow!("RDP input has no PipeWire stream"))?;
            // https://github.com/rustdesk/rustdesk/pull/9019#issuecomment-2295252388
            // There may be a bug in Rdp input on Gnome util Ubuntu 24.04 (Gnome 46)
            //
            // eg. Resolution 800x600, Fractional scale: 200% (logic size: 400x300)
            // https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.RemoteDesktop.html#:~:text=new%20pointer%20position-,in%20the%20streams%20logical%20coordinate%20space,-.
            // Then (x,y) in `mouse_move_to()` and `mouse_move_relative()` should be scaled to the logic size(stream.get_size()), which is from (0,0) to (400,300).
            // For Ubuntu 24.04(Gnome 46), (x,y) is restricted from (0,0) to (400,300), but the actual range in screen is:
            // Logic coordinate from (0,0) to (200x150).
            // Or physical coordinate from (0,0) to (400,300).
            let scale = if backend == RdpInputBackend::Portal && *SHOULD_SCALE_POINTER_COORDINATES {
                if resolution.0 == 0 || stream.get_size().0 == 0 {
                    Some(1.0f64)
                } else {
                    Some(resolution.0 as f64 / stream.get_size().0 as f64)
                }
            } else {
                None
            };
            let pos = stream.get_position();
            Ok(Self {
                conn,
                session,
                backend,
                streams,
                scale,
                position: (pos.0 as f64, pos.1 as f64),
                motion_error_logged: false,
            })
        }

        fn mutter_stream_at(&self, x: f64, y: f64) -> Option<(&PwStreamInfo, f64, f64)> {
            let (index, local_x, local_y) = select_stream_rect(
                self.streams
                    .iter()
                    .map(|stream| (stream.get_position(), stream.get_size())),
                x,
                y,
            )?;
            Some((self.streams.get(index)?, local_x, local_y))
        }

        fn report_motion_result(&mut self, kind: &str, result: ResultType<()>) {
            match result {
                Ok(()) => self.motion_error_logged = false,
                Err(err) if !self.motion_error_logged => {
                    log::error!("Failed to send {kind} pointer motion: {err:#}");
                    self.motion_error_logged = true;
                }
                Err(_) => {}
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            desktop_is_niri, mutter_stream_name, select_stream_rect,
            MUTTER_POINTER_MOTION_RELATIVE_METHOD,
        };

        #[test]
        fn detects_niri_in_desktop_list() {
            assert!(desktop_is_niri("niri"));
            assert!(desktop_is_niri("NIRI"));
            assert!(desktop_is_niri("GNOME:niri"));
            assert!(!desktop_is_niri("GNOME"));
        }

        #[test]
        fn maps_global_pointer_to_the_correct_mutter_monitor() {
            let rects = [
                ((0, 0), (1920, 1080)),
                ((1920, 0), (1920, 1080)),
                ((3840, 0), (1920, 1080)),
            ];
            assert_eq!(
                select_stream_rect(rects, 100.0, 50.0),
                Some((0, 100.0, 50.0))
            );
            assert_eq!(
                select_stream_rect(rects, 2000.0, 60.0),
                Some((1, 80.0, 60.0))
            );
            assert_eq!(
                select_stream_rect(rects, 5000.0, 1079.0),
                Some((2, 1160.0, 1079.0))
            );
        }

        #[test]
        fn clamps_an_out_of_layout_pointer_to_the_fallback_stream() {
            let rects = [((-1920, 0), (1920, 1080)), ((0, 0), (1920, 1080))];
            assert_eq!(
                select_stream_rect(rects, 9000.0, -20.0),
                Some((0, 1919.0, 0.0))
            );
        }

        #[test]
        fn matches_mutter_pointer_motion_wire_contract() {
            assert_eq!(
                MUTTER_POINTER_MOTION_RELATIVE_METHOD,
                "NotifyPointerMotionRelative"
            );
            let path = dbus::Path::new("/org/gnome/Mutter/ScreenCast/Stream/u1").unwrap();
            let stream_name: String = mutter_stream_name(&path);
            assert_eq!(stream_name, "/org/gnome/Mutter/ScreenCast/Stream/u1");
        }

        #[test]
        #[cfg(feature = "gnome-mutter")]
        #[ignore = "requires an active GNOME Wayland session and moves the local pointer"]
        fn live_mutter_pointer_methods_match_the_session_contract() {
            let session = scrap::wayland::mutter::request_session().unwrap();
            super::notify_pointer_motion(
                scrap::wayland::pipewire::RdpInputBackend::Mutter,
                &session.conn,
                &session.session,
                0.0,
                0.0,
            )
            .unwrap();

            let stream = session
                .streams
                .iter()
                .find(|stream| stream.get_position() == (1920, 0))
                .or_else(|| session.streams.first())
                .unwrap();
            let size = stream.get_size();
            super::notify_pointer_motion_absolute(
                scrap::wayland::pipewire::RdpInputBackend::Mutter,
                &session.conn,
                &session.session,
                stream,
                size.0 as f64 / 2.0,
                size.1 as f64 / 2.0,
            )
            .unwrap();
        }
    }

    impl MouseControllable for RdpInputMouse {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn mouse_move_to(&mut self, x: i32, y: i32) {
            if self.backend == RdpInputBackend::Mutter {
                if let Some((stream, local_x, local_y)) = self.mutter_stream_at(x as f64, y as f64)
                {
                    let result = notify_pointer_motion_absolute(
                        self.backend,
                        &self.conn,
                        &self.session,
                        stream,
                        local_x,
                        local_y,
                    );
                    self.report_motion_result("absolute", result);
                }
                return;
            }

            let x = if let Some(s) = self.scale {
                x as f64 / s
            } else {
                x as f64
            };
            let y = if let Some(s) = self.scale {
                y as f64 / s
            } else {
                y as f64
            };
            let x = x - self.position.0;
            let y = y - self.position.1;
            if let Some(stream) = self.streams.first() {
                let result = notify_pointer_motion_absolute(
                    self.backend,
                    &self.conn,
                    &self.session,
                    stream,
                    x,
                    y,
                );
                self.report_motion_result("absolute", result);
            }
        }
        fn mouse_move_relative(&mut self, x: i32, y: i32) {
            let x = if let Some(s) = self.scale {
                x as f64 / s
            } else {
                x as f64
            };
            let y = if let Some(s) = self.scale {
                y as f64 / s
            } else {
                y as f64
            };
            let result = notify_pointer_motion(self.backend, &self.conn, &self.session, x, y);
            self.report_motion_result("relative", result);
        }
        fn mouse_down(&mut self, button: MouseButton) -> enigo::ResultType {
            if let Err(err) =
                handle_mouse(true, button, self.conn.clone(), &self.session, self.backend)
            {
                log::error!("Failed to send pointer button down: {err:#}");
            }
            Ok(())
        }
        fn mouse_up(&mut self, button: MouseButton) {
            if let Err(err) = handle_mouse(
                false,
                button,
                self.conn.clone(),
                &self.session,
                self.backend,
            ) {
                log::error!("Failed to send pointer button up: {err:#}");
            }
        }
        fn mouse_click(&mut self, button: MouseButton) {
            if let Err(err) =
                handle_mouse(true, button, self.conn.clone(), &self.session, self.backend)
            {
                log::error!("Failed to send pointer click down: {err:#}");
            }
            if let Err(err) = handle_mouse(
                false,
                button,
                self.conn.clone(),
                &self.session,
                self.backend,
            ) {
                log::error!("Failed to send pointer click up: {err:#}");
            }
        }
        fn mouse_scroll_x(&mut self, length: i32) {
            if let Err(err) =
                notify_pointer_axis(self.backend, &self.conn, &self.session, length as f64, 0.0)
            {
                log::error!("Failed to send horizontal pointer scroll: {err:#}");
            }
        }
        fn mouse_scroll_y(&mut self, length: i32) {
            if let Err(err) =
                notify_pointer_axis(self.backend, &self.conn, &self.session, 0.0, length as f64)
            {
                log::error!("Failed to send vertical pointer scroll: {err:#}");
            }
        }
    }

    fn notify_keyboard_keycode(
        backend: RdpInputBackend,
        conn: &SyncConnection,
        session: &Path<'static>,
        keycode: i32,
        down: bool,
    ) -> ResultType<()> {
        match backend {
            RdpInputBackend::Portal => {
                let portal = get_portal(conn);
                let state = if down {
                    PRESSED_DOWN_STATE
                } else {
                    PRESSED_UP_STATE
                };
                remote_desktop_portal::notify_keyboard_keycode(
                    &portal,
                    session,
                    HashMap::new(),
                    keycode,
                    state,
                )?;
            }
            RdpInputBackend::Mutter => {
                let proxy =
                    conn.with_proxy(MUTTER_REMOTE_DESKTOP_NAME, session.clone(), DBUS_TIMEOUT);
                let _: () = proxy.method_call(
                    MUTTER_REMOTE_DESKTOP_SESSION_IFACE,
                    "NotifyKeyboardKeycode",
                    (keycode as u32, down),
                )?;
            }
        }
        Ok(())
    }

    fn notify_keyboard_keysym(
        backend: RdpInputBackend,
        conn: &SyncConnection,
        session: &Path<'static>,
        keysym: i32,
        down: bool,
    ) -> ResultType<()> {
        match backend {
            RdpInputBackend::Portal => {
                let portal = get_portal(conn);
                let state = if down {
                    PRESSED_DOWN_STATE
                } else {
                    PRESSED_UP_STATE
                };
                remote_desktop_portal::notify_keyboard_keysym(
                    &portal,
                    session,
                    HashMap::new(),
                    keysym,
                    state,
                )?;
            }
            RdpInputBackend::Mutter => {
                let proxy =
                    conn.with_proxy(MUTTER_REMOTE_DESKTOP_NAME, session.clone(), DBUS_TIMEOUT);
                let _: () = proxy.method_call(
                    MUTTER_REMOTE_DESKTOP_SESSION_IFACE,
                    "NotifyKeyboardKeysym",
                    (keysym as u32, down),
                )?;
            }
        }
        Ok(())
    }

    fn notify_pointer_motion_absolute(
        backend: RdpInputBackend,
        conn: &SyncConnection,
        session: &Path<'static>,
        stream: &PwStreamInfo,
        x: f64,
        y: f64,
    ) -> ResultType<()> {
        match backend {
            RdpInputBackend::Portal => {
                let portal = get_portal(conn);
                remote_desktop_portal::notify_pointer_motion_absolute(
                    &portal,
                    session,
                    HashMap::new(),
                    stream.path as u32,
                    x,
                    y,
                )?;
            }
            RdpInputBackend::Mutter => {
                let stream_path = stream.get_mutter_path().ok_or_else(|| {
                    hbb_common::anyhow::anyhow!("Mutter stream is missing its D-Bus object path")
                })?;
                let stream_name = mutter_stream_name(stream_path);
                let proxy =
                    conn.with_proxy(MUTTER_REMOTE_DESKTOP_NAME, session.clone(), DBUS_TIMEOUT);
                let _: () = proxy.method_call(
                    MUTTER_REMOTE_DESKTOP_SESSION_IFACE,
                    "NotifyPointerMotionAbsolute",
                    (stream_name, x, y),
                )?;
            }
        }
        Ok(())
    }

    fn notify_pointer_motion(
        backend: RdpInputBackend,
        conn: &SyncConnection,
        session: &Path<'static>,
        x: f64,
        y: f64,
    ) -> ResultType<()> {
        match backend {
            RdpInputBackend::Portal => {
                let portal = get_portal(conn);
                remote_desktop_portal::notify_pointer_motion(
                    &portal,
                    session,
                    HashMap::new(),
                    x,
                    y,
                )?;
            }
            RdpInputBackend::Mutter => {
                let proxy =
                    conn.with_proxy(MUTTER_REMOTE_DESKTOP_NAME, session.clone(), DBUS_TIMEOUT);
                let _: () = proxy.method_call(
                    MUTTER_REMOTE_DESKTOP_SESSION_IFACE,
                    MUTTER_POINTER_MOTION_RELATIVE_METHOD,
                    (x, y),
                )?;
            }
        }
        Ok(())
    }

    fn mutter_stream_name(stream_path: &Path<'_>) -> String {
        // Mutter's private RemoteDesktop API intentionally declares this argument as D-Bus
        // string (`s`), even though its value is a ScreenCast object path. Passing Path would
        // encode it as object-path (`o`) and make every absolute motion call fail validation.
        stream_path.to_string()
    }

    fn notify_pointer_axis(
        backend: RdpInputBackend,
        conn: &SyncConnection,
        session: &Path<'static>,
        x: f64,
        y: f64,
    ) -> ResultType<()> {
        match backend {
            RdpInputBackend::Portal => {
                let portal = get_portal(conn);
                remote_desktop_portal::notify_pointer_axis(&portal, session, HashMap::new(), x, y)?;
            }
            RdpInputBackend::Mutter => {
                let proxy =
                    conn.with_proxy(MUTTER_REMOTE_DESKTOP_NAME, session.clone(), DBUS_TIMEOUT);
                let _: () = proxy.method_call(
                    MUTTER_REMOTE_DESKTOP_SESSION_IFACE,
                    "NotifyPointerAxis",
                    (x, y, 0u32),
                )?;
            }
        }
        Ok(())
    }

    /// Send a keysym through the active remote-desktop backend.
    fn send_keysym(
        keysym: i32,
        down: bool,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
        backend: RdpInputBackend,
    ) -> ResultType<()> {
        let state: u32 = if down {
            PRESSED_DOWN_STATE
        } else {
            PRESSED_UP_STATE
        };
        log::trace!(
            "send_keysym: calling notify_keyboard_keysym, keysym={:#x}, state={}",
            keysym,
            state
        );
        match notify_keyboard_keysym(backend, &conn, session, keysym, down) {
            Ok(_) => {
                log::trace!("send_keysym: notify_keyboard_keysym succeeded");
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn get_raw_evdev_keycode(key: u16) -> i32 {
        // 8 is the offset between xkb and evdev
        let mut key = key as i32 - 8;
        // fix for right_meta key
        if key == 126 {
            key = 125;
        }
        key
    }

    fn handle_key(
        down: bool,
        key: Key,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
        backend: RdpInputBackend,
    ) -> ResultType<()> {
        match key {
            Key::Raw(key) => {
                let key = get_raw_evdev_keycode(key);
                notify_keyboard_keycode(backend, &conn, session, key, down)?;
            }
            _ => {
                if let Ok((key, is_shift)) = map_key(&key) {
                    let shift_keycode = evdev::Key::KEY_LEFTSHIFT.code() as i32;
                    if down {
                        // Press: Shift down first, then key down
                        if is_shift {
                            if let Err(e) = notify_keyboard_keycode(
                                backend,
                                &conn,
                                session,
                                shift_keycode,
                                true,
                            ) {
                                log::error!("handle_key: failed to press Shift: {:?}", e);
                                return Err(e.into());
                            }
                        }
                        if let Err(e) = notify_keyboard_keycode(
                            backend,
                            &conn,
                            session,
                            key.code() as i32,
                            true,
                        ) {
                            log::error!("handle_key: failed to press key: {:?}", e);
                            // Best-effort: release Shift if it was pressed
                            if is_shift {
                                if let Err(e) = notify_keyboard_keycode(
                                    backend,
                                    &conn,
                                    session,
                                    shift_keycode,
                                    false,
                                ) {
                                    log::warn!(
                                        "handle_key: best-effort Shift release also failed: {:?}",
                                        e
                                    );
                                }
                            }
                            return Err(e.into());
                        }
                    } else {
                        // Release: key up first, then Shift up
                        if let Err(e) = notify_keyboard_keycode(
                            backend,
                            &conn,
                            session,
                            key.code() as i32,
                            false,
                        ) {
                            log::error!("handle_key: failed to release key: {:?}", e);
                            // Best-effort: still try to release Shift
                            if is_shift {
                                if let Err(e) = notify_keyboard_keycode(
                                    backend,
                                    &conn,
                                    session,
                                    shift_keycode,
                                    false,
                                ) {
                                    log::warn!(
                                        "handle_key: best-effort Shift release also failed: {:?}",
                                        e
                                    );
                                }
                            }
                            return Err(e.into());
                        }
                        if is_shift {
                            if let Err(e) = notify_keyboard_keycode(
                                backend,
                                &conn,
                                session,
                                shift_keycode,
                                false,
                            ) {
                                log::error!("handle_key: failed to release Shift: {:?}", e);
                                return Err(e.into());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_mouse(
        down: bool,
        button: MouseButton,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
        backend: RdpInputBackend,
    ) -> ResultType<()> {
        let but_key = match button {
            MouseButton::Left => EVDEV_MOUSE_LEFT,
            MouseButton::Right => EVDEV_MOUSE_RIGHT,
            MouseButton::Middle => EVDEV_MOUSE_MIDDLE,
            _ => return Ok(()),
        };
        match backend {
            RdpInputBackend::Portal => {
                let portal = get_portal(&conn);
                let state = if down {
                    PRESSED_DOWN_STATE
                } else {
                    PRESSED_UP_STATE
                };
                remote_desktop_portal::notify_pointer_button(
                    &portal,
                    session,
                    HashMap::new(),
                    but_key,
                    state,
                )?;
            }
            RdpInputBackend::Mutter => {
                let proxy =
                    conn.with_proxy(MUTTER_REMOTE_DESKTOP_NAME, session.clone(), DBUS_TIMEOUT);
                let _: () = proxy.method_call(
                    MUTTER_REMOTE_DESKTOP_SESSION_IFACE,
                    "NotifyPointerButton",
                    (but_key, down),
                )?;
            }
        }
        Ok(())
    }
}
