use super::{source::Source, Keymap, LayoutKey, CAPS_LOCK_BIT, NUM_LOCK_BIT};
use base::message_proto::{key_event, KeyEvent, KeyboardMode};
use hbb_common::{anyhow::anyhow, lazy_static, ResultType};
use std::{
    sync::{Mutex, RwLock},
    time::{Duration, Instant},
};

const LAYOUT_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

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
        refresh.source = None;
        *CURRENT.write().unwrap() = Err(error.to_string());
    }
    refresh.checked = Some(Instant::now());
}

impl Refresh {
    fn update(&mut self) -> ResultType<()> {
        let source = Source::read()?;
        if self.source.as_ref() == Some(&source) {
            return Ok(());
        }
        let keymap = Keymap::load(&source)?;
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
    let keymap = current.as_ref().map_err(|error| anyhow!("{}", error))?;
    let index = usize::from(locks.0) * CAPS_LOCK_BIT + usize::from(locks.1) * NUM_LOCK_BIT;
    keymap.maps[index]
        .get(&character)
        .cloned()
        .ok_or_else(|| anyhow!("Character cannot be generated in the detected XKB layout"))
}
