use super::{source::Source, Keymap, LayoutKey, CAPS_LOCK_BIT, NUM_LOCK_BIT, SHIFT_BIT};
use crate::server::input_service::is_ascii_printable;
use base::message_proto::{key_event, KeyEvent, KeyboardMode};
use hbb_common::{bail, lazy_static, ResultType};
use std::{
    sync::{mpsc, Mutex, RwLock, TryLockError},
    thread,
    time::{Duration, Instant},
};

const LAYOUT_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
// GNOME 46/Plasma 6.6 probes (200-400 initial samples): preparation p99 7-16 ms;
// repeated refresh medians were 2-6 ms. KDE query outliers exceeded 100 ms,
// so this limits input waiting while the background query continues.
const LAYOUT_INPUT_WAIT: Duration = Duration::from_millis(25);
const LAYOUT_WARNING_INTERVAL: Duration = Duration::from_secs(5);

lazy_static::lazy_static! {
    static ref REFRESH: Mutex<Refresh> = Mutex::new(Refresh::default());
    // Only the first relevant input takes the startup query's completion receiver.
    static ref PREWARM_RECEIVER: Mutex<Option<mpsc::Receiver<()>>> = Mutex::new(None);
    static ref CURRENT: RwLock<Result<Keymap, String>> =
        RwLock::new(Err("Uinput keyboard layout has not been detected".to_owned()));
}

#[derive(Default)]
struct Refresh {
    checked: Option<Instant>,
    source: Option<Source>,
    // Cover scheduling until the worker acquires REFRESH.
    running: bool,
}

pub(in crate::server::uinput) fn prepare_layout(event: &KeyEvent) {
    if !crate::server::input_service::wayland_use_uinput() || !needs_layout(event) {
        return;
    }
    let deadline = Instant::now() + LAYOUT_INPUT_WAIT;
    let receiver = {
        let mut prewarm = PREWARM_RECEIVER.lock().unwrap();
        // Prefer a due refresh over an old, already-completed startup query.
        start_refresh().or(prewarm.take())
    };
    if let Some(receiver) = receiver {
        wait_for_refresh(receiver, deadline);
    }
}

pub(in crate::server::uinput) fn prewarm_layout() {
    // Use server startup time for discovery before the first remote character.
    // This reduces cold-start fallback; a still-pending query cannot guarantee readiness.
    let mut prewarm = PREWARM_RECEIVER.lock().unwrap();
    if let Some(receiver) = start_refresh() {
        *prewarm = Some(receiver);
    }
}

fn start_refresh() -> Option<mpsc::Receiver<()>> {
    // A stalled desktop query must not block later input or spawn more workers.
    let mut refresh = match REFRESH.try_lock() {
        Ok(refresh) => refresh,
        Err(TryLockError::WouldBlock) => return None,
        Err(TryLockError::Poisoned(error)) => {
            hbb_common::throttled_log!(
                LAYOUT_WARNING_INTERVAL,
                error,
                "Uinput layout refresh lock is poisoned: {}",
                error
            );
            return None;
        }
    };
    if refresh.running
        || refresh
            .checked
            .is_some_and(|checked| checked.elapsed() < LAYOUT_REFRESH_INTERVAL)
    {
        return None;
    }
    let (completed, receiver) = mpsc::channel();
    refresh.running = true;
    if let Err(error) = thread::Builder::new()
        .name("uinput-layout".to_owned())
        .spawn(move || refresh_layout(completed))
    {
        refresh.running = false;
        refresh.checked = Some(Instant::now());
        hbb_common::throttled_log!(
            LAYOUT_WARNING_INTERVAL,
            error,
            "Cannot start uinput layout refresh: {}",
            error
        );
        return None;
    }
    drop(refresh);
    Some(receiver)
}

fn refresh_layout(completed: mpsc::Sender<()>) {
    let mut refresh = REFRESH.lock().unwrap();
    if let Err(error) = refresh.update() {
        let retained = {
            let mut current = CURRENT.write().unwrap();
            if current.is_err() {
                *current = Err(error.to_string());
            }
            current.is_ok()
        };
        hbb_common::throttled_log!(
            LAYOUT_WARNING_INTERVAL,
            warn,
            "Uinput layout refresh failed (previous mapping retained: {}): {}",
            retained,
            error
        );
    }
    refresh.checked = Some(Instant::now());
    refresh.running = false;
    drop(refresh);
    // An input timeout drops its receiver but leaves this worker running.
    if completed.send(()).is_err() {
        hbb_common::log::trace!("Uinput layout refresh finished without an input waiter");
    }
}

fn wait_for_refresh(receiver: mpsc::Receiver<()>, deadline: Instant) {
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(()) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => hbb_common::throttled_log!(
            LAYOUT_WARNING_INTERVAL,
            warn,
            "Uinput layout refresh exceeded the {} ms input wait; query continues in the background",
            LAYOUT_INPUT_WAIT.as_millis()
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => hbb_common::throttled_log!(
            LAYOUT_WARNING_INTERVAL,
            error,
            "Uinput layout refresh worker stopped before completion"
        ),
    }
}

impl Refresh {
    fn update(&mut self) -> ResultType<()> {
        let source = Source::read().map_err(|error| {
            if error.is::<super::source::UnreliableSource>() {
                // Unlike a transient query failure, this invalidates the old map.
                self.source = None;
                *CURRENT.write().unwrap() = Err(error.to_string());
            }
            error
        })?;
        if self.source.as_ref() == Some(&source) {
            return Ok(());
        }
        let keymap = match Keymap::load(&source) {
            Ok(keymap) => keymap,
            Err(error) => {
                // A newly observed layout cannot use the previous layout's map.
                self.source = None;
                *CURRENT.write().unwrap() = Err(error.to_string());
                return Err(error);
            }
        };
        *CURRENT.write().unwrap() = Ok(keymap);
        self.source = Some(source);
        Ok(())
    }
}

fn needs_layout(event: &KeyEvent) -> bool {
    let mode = event.mode.enum_value_or(KeyboardMode::Legacy);
    if mode == KeyboardMode::Map {
        return false;
    }
    match &event.union {
        Some(key_event::Union::Seq(sequence)) => {
            // Translate shortcuts use Key::Layout even for non-ASCII text.
            !sequence.is_empty()
                && (mode == KeyboardMode::Translate || sequence.chars().all(is_ascii_printable))
        }
        Some(key_event::Union::Chr(_)) => mode == KeyboardMode::Legacy && event.down,
        Some(key_event::Union::Unicode(character)) => {
            mode == KeyboardMode::Legacy
                && char::from_u32(*character).is_some_and(is_ascii_printable)
        }
        _ => false,
    }
}

pub(in crate::server::uinput) fn is_available() -> bool {
    let current = CURRENT.read().unwrap();
    if let Err(error) = current.as_ref() {
        hbb_common::throttled_log!(
            LAYOUT_WARNING_INTERVAL,
            warn,
            "Uinput layout unavailable: {}; using legacy character mapping (may differ from the host layout)",
            error
        );
    }
    current.is_ok()
}

pub(in crate::server::uinput) fn resolve(
    character: char,
    locks: (bool, bool),
    shortcut_shift: Option<bool>,
) -> ResultType<LayoutKey> {
    let current = CURRENT.read().unwrap();
    let keymap = match current.as_ref() {
        Ok(keymap) => keymap,
        Err(error) => {
            hbb_common::throttled_log!(
                LAYOUT_WARNING_INTERVAL,
                warn,
                "Uinput layout unavailable: {}; using legacy character mapping (may differ from the host layout)",
                error
            );
            return Ok(LayoutKey {
                key: enigo::Key::Layout(character),
                modifiers: Vec::new(),
            });
        }
    };
    let index = usize::from(locks.0) * CAPS_LOCK_BIT + usize::from(locks.1) * NUM_LOCK_BIT;
    // Letter shortcuts keep their physical key even when Shift changes case.
    // Other shortcuts prefer a symbol compatible with the caller's held Shift.
    let shifted = (shortcut_shift == Some(true) && !character.is_alphabetic())
        .then(|| keymap.maps[index | SHIFT_BIT].get(&character))
        .flatten();
    if let Some(mapping) = shifted.or_else(|| keymap.maps[index].get(&character)) {
        return Ok(mapping.clone());
    }
    if shortcut_shift.is_some() && character.is_ascii_graphic() {
        return legacy_shortcut(character);
    }
    bail!("Character cannot be generated in the detected XKB layout")
}

fn legacy_shortcut(character: char) -> ResultType<LayoutKey> {
    // Uppercase letters must not synthesize Shift for a shortcut, but shifted
    // punctuation still needs the legacy table's modifier.
    let (key, shift) =
        super::super::service::map_key(&enigo::Key::Layout(character.to_ascii_lowercase()))?;
    hbb_common::throttled_log!(
        LAYOUT_WARNING_INTERVAL,
        warn,
        "Uinput layout lacks an ASCII shortcut character; using its legacy physical mapping"
    );
    Ok(LayoutKey {
        key: enigo::Key::Raw(key.code() + super::XKB_KEYCODE_OFFSET),
        modifiers: if shift {
            vec![evdev::Key::KEY_LEFTSHIFT.code() + super::XKB_KEYCODE_OFFSET]
        } else {
            Vec::new()
        },
    })
}
