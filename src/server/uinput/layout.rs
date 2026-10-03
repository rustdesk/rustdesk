use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{collections::HashMap, ops::RangeInclusive, ptr::NonNull};
use xkbcommon_dl::{self as xkb, keysyms};

mod cache;
mod source;
mod wayland;

pub(super) use cache::{prepare_layout, resolve};

pub(super) const XKB_KEYCODE_OFFSET: u16 = 8;
const CAPS_LOCK_BIT: usize = 1;
const NUM_LOCK_BIT: usize = 2;
const LOCK_STATE_COUNT: usize = 4;
const DEFAULT_GROUP: u32 = 0;

#[derive(Clone)]
pub(super) struct LayoutKey {
    pub keycode: u16,
    pub modifiers: Vec<u16>,
}

struct Keymap {
    maps: [HashMap<char, LayoutKey>; LOCK_STATE_COUNT],
}

impl Keymap {
    fn load(source: &source::Source) -> ResultType<Self> {
        let keymap = XkbKeymap::open(source)?;
        let mut state = State::new(&keymap)?;
        let caps = state.locked_mask(keysyms::Caps_Lock);
        let num = state.locked_mask(keysyms::Num_Lock);
        let modifiers = [
            state.modifier(&[keysyms::Shift_L, keysyms::Shift_R]),
            state.modifier(&[keysyms::ISO_Level3_Shift]),
        ];
        let maps = std::array::from_fn(|index| {
            let locked = if index & CAPS_LOCK_BIT != 0 { caps } else { 0 }
                | if index & NUM_LOCK_BIT != 0 { num } else { 0 };
            state.mappings(locked, &modifiers)
        });
        Ok(Self { maps })
    }
}

struct XkbKeymap {
    api: &'static xkb::XkbCommon,
    map: NonNull<xkb::xkb_keymap>,
    group: u32,
}

impl XkbKeymap {
    fn open(source: &source::Source) -> ResultType<Self> {
        let api = xkb::xkbcommon_option().ok_or_else(|| anyhow!("Cannot load libxkbcommon"))?;
        let mut keymap = Self {
            api,
            map: compile(api, source)?,
            group: DEFAULT_GROUP,
        };
        let groups = unsafe { (api.xkb_keymap_num_layouts)(keymap.map.as_ptr()) };
        if let source::Source::Wayland { group, .. } = source {
            if groups > 1 {
                keymap.group =
                    group.ok_or_else(|| anyhow!("Cannot determine the active XKB group"))?;
            }
        }
        if keymap.group >= groups {
            bail!("The detected keyboard layout group is absent from the keymap");
        }
        Ok(keymap)
    }

    fn keycodes(&self) -> RangeInclusive<u32> {
        unsafe {
            (self.api.xkb_keymap_min_keycode)(self.map.as_ptr())
                ..=(self.api.xkb_keymap_max_keycode)(self.map.as_ptr())
        }
    }
}

impl Drop for XkbKeymap {
    fn drop(&mut self) {
        unsafe { (self.api.xkb_keymap_unref)(self.map.as_ptr()) };
    }
}

fn compile(api: &xkb::XkbCommon, source: &source::Source) -> ResultType<NonNull<xkb::xkb_keymap>> {
    let context =
        unsafe { (api.xkb_context_new)(xkb::xkb_context_flags::XKB_CONTEXT_NO_ENVIRONMENT_NAMES) };
    let context = NonNull::new(context).ok_or_else(|| anyhow!("Cannot create XKB context"))?;
    let flags = xkb::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS;
    let map = unsafe {
        match source {
            source::Source::Gnome(names) => {
                let names = xkb::xkb_rule_names {
                    rules: std::ptr::null(),
                    model: names.model.as_ptr(),
                    layout: names.layout.as_ptr(),
                    variant: names.variant.as_ptr(),
                    options: names.options.as_ptr(),
                };
                (api.xkb_keymap_new_from_names)(context.as_ptr(), &names, flags)
            }
            source::Source::Wayland { keymap: bytes, .. } => (api.xkb_keymap_new_from_string)(
                context.as_ptr(),
                bytes.as_ptr().cast(),
                xkb::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                flags,
            ),
        }
    };
    unsafe { (api.xkb_context_unref)(context.as_ptr()) };
    NonNull::new(map).ok_or_else(|| anyhow!("Cannot compile the detected XKB keyboard map"))
}

#[derive(Clone, Copy)]
struct Modifier {
    keycode: u16,
    mask: u32,
}

struct State<'a> {
    keymap: &'a XkbKeymap,
    state: NonNull<xkb::xkb_state>,
}

impl<'a> State<'a> {
    fn new(keymap: &'a XkbKeymap) -> ResultType<Self> {
        let state = unsafe { (keymap.api.xkb_state_new)(keymap.map.as_ptr()) };
        let state = NonNull::new(state).ok_or_else(|| anyhow!("Cannot create XKB state"))?;
        let mut state = Self { keymap, state };
        state.set_modifiers(0, 0);
        Ok(state)
    }

    fn set_modifiers(&mut self, depressed: u32, locked: u32) {
        unsafe {
            (self.keymap.api.xkb_state_update_mask)(
                self.state.as_ptr(),
                depressed,
                0,
                locked,
                0,
                0,
                self.keymap.group,
            );
        }
    }

    fn symbol(&self, keycode: u16) -> u32 {
        unsafe {
            (self.keymap.api.xkb_state_key_get_one_sym)(self.state.as_ptr(), u32::from(keycode))
        }
    }

    fn update_key(&mut self, keycode: u16, direction: xkb::xkb_key_direction) {
        unsafe {
            (self.keymap.api.xkb_state_update_key)(
                self.state.as_ptr(),
                u32::from(keycode),
                direction,
            )
        };
    }

    fn locked_mask(&mut self, symbol: u32) -> u32 {
        let keycode = self
            .keymap
            .keycodes()
            .filter_map(supported_keycode)
            .find(|keycode| self.symbol(*keycode) == symbol);
        let Some(keycode) = keycode else { return 0 };
        self.update_key(keycode, xkb::xkb_key_direction::XKB_KEY_DOWN);
        self.update_key(keycode, xkb::xkb_key_direction::XKB_KEY_UP);
        let mask = unsafe {
            (self.keymap.api.xkb_state_serialize_mods)(
                self.state.as_ptr(),
                xkb::xkb_state_component::XKB_STATE_MODS_LOCKED,
            )
        };
        self.set_modifiers(0, 0);
        mask
    }

    fn modifier(&mut self, symbols: &[u32]) -> Option<Modifier> {
        // Prefer physical modifier keys to XKB's synthetic <LVL3> keycode.
        let preferred = [
            evdev::Key::KEY_LEFTSHIFT,
            evdev::Key::KEY_RIGHTSHIFT,
            evdev::Key::KEY_RIGHTALT,
        ];
        let preferred = preferred
            .iter()
            .map(|key| u32::from(key.code() + XKB_KEYCODE_OFFSET));
        for keycode in preferred
            .chain(self.keymap.keycodes())
            .filter_map(supported_keycode)
        {
            if !symbols.contains(&self.symbol(keycode)) {
                continue;
            }
            self.update_key(keycode, xkb::xkb_key_direction::XKB_KEY_DOWN);
            let mask = unsafe {
                (self.keymap.api.xkb_state_serialize_mods)(
                    self.state.as_ptr(),
                    xkb::xkb_state_component::XKB_STATE_MODS_DEPRESSED,
                )
            };
            self.update_key(keycode, xkb::xkb_key_direction::XKB_KEY_UP);
            self.set_modifiers(0, 0);
            if mask != 0 {
                return Some(Modifier { keycode, mask });
            }
        }
        None
    }

    fn mappings(
        &mut self,
        locked: u32,
        modifiers: &[Option<Modifier>],
    ) -> HashMap<char, LayoutKey> {
        let mut mappings = HashMap::new();
        for combination in 0..(1usize << modifiers.len()) {
            let selected: Vec<_> = modifiers
                .iter()
                .enumerate()
                .filter(|(index, _)| combination & (1 << *index) != 0)
                .filter_map(|(_, modifier)| *modifier)
                .collect();
            let mask = selected
                .iter()
                .fold(0, |mask, modifier| mask | modifier.mask);
            self.set_modifiers(mask, locked);
            for keycode in self.keymap.keycodes().filter_map(supported_keycode) {
                let codepoint =
                    unsafe { (self.keymap.api.xkb_keysym_to_utf32)(self.symbol(keycode)) };
                if let Some(character) =
                    char::from_u32(codepoint).filter(|character| *character != '\0')
                {
                    mappings.entry(character).or_insert_with(|| LayoutKey {
                        keycode,
                        modifiers: selected.iter().map(|modifier| modifier.keycode).collect(),
                    });
                }
            }
        }
        mappings
    }
}

impl Drop for State<'_> {
    fn drop(&mut self) {
        unsafe { (self.keymap.api.xkb_state_unref)(self.state.as_ptr()) };
    }
}

fn supported_keycode(keycode: u32) -> Option<u16> {
    let keycode = u16::try_from(keycode).ok()?;
    let code = keycode.checked_sub(XKB_KEYCODE_OFFSET)?;
    // Match the keys enabled by create_uinput_keyboard().
    let supported = (evdev::Key::KEY_ESC.code()..=evdev::Key::BTN_TRIGGER_HAPPY40.code())
        .contains(&code)
        && !format!("{:?}", evdev::Key::new(code)).contains("unknown");
    supported.then_some(keycode)
}
