use super::{source::Source, Keymap, LayoutKey, CAPS_LOCK_BIT, NUM_LOCK_BIT};
use base::{
    config::keys::OPTION_DISABLE_UINPUT_LAYOUT_FALLBACK,
    message_proto::{key_event, KeyEvent, KeyboardMode},
};
use hbb_common::{anyhow::anyhow, config::Config, lazy_static, ResultType};
use std::{
    sync::{Mutex, RwLock},
    time::{Duration, Instant},
};

const LAYOUT_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const LAYOUT_WARNING_INTERVAL: Duration = Duration::from_secs(5);

lazy_static::lazy_static! {
    static ref REFRESH: Mutex<Refresh> = Mutex::new(Refresh::default());
    static ref CURRENT: RwLock<Result<Keymap, String>> =
        RwLock::new(Err("Uinput keyboard layout has not been detected".to_owned()));
}

#[derive(Default)]
struct Refresh {
    checked: Option<Instant>,
    source: Option<Source>,
}

pub(in crate::server::uinput) fn prepare_layout(event: &KeyEvent) {
    if !crate::server::input_service::wayland_use_uinput() || !needs_layout(event) {
        return;
    }
    // Only preparation takes REFRESH. Character dispatch takes CURRENT alone,
    // so desktop IO never holds a lock needed by ENIGO's character resolver.
    let mut refresh = REFRESH.lock().unwrap();
    if refresh
        .checked
        .is_some_and(|checked| checked.elapsed() < LAYOUT_REFRESH_INTERVAL)
    {
        return;
    }
    if let Err(error) = refresh.update() {
        let mut current = CURRENT.write().unwrap();
        if current.is_err() {
            *current = Err(error.to_string());
        }
        hbb_common::throttled_log!(
            LAYOUT_WARNING_INTERVAL,
            warn,
            "Uinput layout refresh failed (previous mapping retained: {}): {}",
            current.is_ok(),
            error
        );
    }
    refresh.checked = Some(Instant::now());
}

impl Refresh {
    fn update(&mut self) -> ResultType<()> {
        let source = Source::read()?;
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
        Some(key_event::Union::Seq(sequence)) => !sequence.is_empty(),
        Some(key_event::Union::Chr(_)) => mode == KeyboardMode::Legacy && event.down,
        Some(key_event::Union::Unicode(_)) => mode == KeyboardMode::Legacy,
        _ => false,
    }
}

pub(in crate::server::uinput) fn resolve(
    character: char,
    locks: (bool, bool),
) -> ResultType<LayoutKey> {
    let current = CURRENT.read().unwrap();
    let keymap = match current.as_ref() {
        Ok(keymap) => keymap,
        Err(error) => {
            if Config::get_option(OPTION_DISABLE_UINPUT_LAYOUT_FALLBACK) == "Y" {
                return Err(anyhow!("{}", error));
            }
            hbb_common::throttled_log!(
                LAYOUT_WARNING_INTERVAL,
                warn,
                "Uinput layout unavailable: {}; using legacy character mapping (may differ from the host layout). Set {}=Y to disable",
                error,
                OPTION_DISABLE_UINPUT_LAYOUT_FALLBACK
            );
            return Ok(LayoutKey {
                key: enigo::Key::Layout(character),
                modifiers: Vec::new(),
            });
        }
    };
    let index = usize::from(locks.0) * CAPS_LOCK_BIT + usize::from(locks.1) * NUM_LOCK_BIT;
    keymap.maps[index]
        .get(&character)
        .cloned()
        .ok_or_else(|| anyhow!("Character cannot be generated in the detected XKB layout"))
}
