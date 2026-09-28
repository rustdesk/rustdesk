use super::*;

#[derive(Default)]
pub(super) struct State {
    pending_modifier: Option<Event>,
    pending_session: u128,
    used: bool,
    blocked_digits: u8,
}

enum Action {
    Pass,
    Consume,
    Flush(Event),
}

fn modifier() -> Key {
    #[cfg(target_os = "macos")]
    return Key::MetaRight;
    #[cfg(target_os = "windows")]
    return Key::ControlRight;
}

fn digit(key: Key) -> Option<(usize, u8)> {
    match key {
        Key::Num1 => Some((0, 1)),
        Key::Num2 => Some((1, 2)),
        _ => None,
    }
}

fn can_start() -> bool {
    if !KEYBOARD_HOOKED.load(Ordering::SeqCst) || flutter::get_cur_session_display_count() < 2 {
        return false;
    }
    let Some(session) = flutter::get_cur_session() else {
        return false;
    };
    if !session.is_default()
        || !*session.server_keyboard_enabled.read().unwrap()
        || session.lc.read().unwrap().view_only.v
    {
        return false;
    }
    !MODIFIERS_STATE
        .lock()
        .unwrap()
        .iter()
        .any(|(key, down)| *down && *key != modifier())
}

impl State {
    fn on_key(&mut self, event: &Event, key: Key, is_press: bool) -> Action {
        if !KEYBOARD_HOOKED.load(Ordering::SeqCst)
            || ((self.pending_modifier.is_some() || self.blocked_digits != 0)
                && self.pending_session != flutter::get_cur_session_id().as_u128())
        {
            self.pending_modifier = None;
            self.used = false;
            self.blocked_digits = 0;
        }

        if let Some((_, bit)) = digit(key) {
            if !is_press && self.blocked_digits & bit != 0 {
                self.blocked_digits &= !bit;
                return Action::Consume;
            }
        }

        if self.pending_modifier.is_some() {
            if key == modifier() {
                if is_press {
                    return Action::Consume;
                }
                let pending = self.pending_modifier.take();
                let used = std::mem::take(&mut self.used);
                return if used {
                    Action::Consume
                } else {
                    pending.map(Action::Flush).unwrap_or(Action::Pass)
                };
            }
            if is_press {
                if let Some((display, bit)) = digit(key) {
                    if display < flutter::get_cur_session_display_count() && can_start() {
                        if self.blocked_digits & bit == 0 {
                            self.blocked_digits |= bit;
                            self.used = true;
                            let display = display.to_string();
                            flutter::push_session_event(
                                &flutter::get_cur_session_id(),
                                "switch_display_hotkey",
                                vec![("display", &display)],
                            );
                        }
                        return Action::Consume;
                    }
                }
            }
            self.used = false;
            return self
                .pending_modifier
                .take()
                .map(Action::Flush)
                .unwrap_or(Action::Pass);
        }

        if key == modifier() && is_press && can_start() {
            self.pending_modifier = Some(event.clone());
            self.pending_session = flutter::get_cur_session_id().as_u128();
            return Action::Consume;
        }
        Action::Pass
    }
}

pub(super) fn handle<F>(state: &mut State, event: Event, send: &F) -> Option<Event>
where
    F: Fn(Event, Key, bool) -> Option<Event>,
{
    let (key, is_press) = match event.event_type {
        EventType::KeyPress(key) => (key, true),
        EventType::KeyRelease(key) => (key, false),
        EventType::ButtonPress(..) | EventType::ButtonRelease(..) => {
            if let Some(pending) = state.pending_modifier.take() {
                state.used = false;
                let _ = send(pending, modifier(), true);
            }
            return Some(event);
        }
        _ => return Some(event),
    };
    match state.on_key(&event, key, is_press) {
        Action::Pass => send(event, key, is_press),
        Action::Consume => None,
        Action::Flush(pending) => {
            let _ = send(pending, modifier(), true);
            send(event, key, is_press)
        }
    }
}
