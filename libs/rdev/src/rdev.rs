#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::{fmt, fmt::Display};

// /// Callback type to send to listen function.
// pub type Callback = dyn FnMut(Event) -> ();

/// Callback type to send to grab function.
pub type GrabCallback = fn(event: Event) -> Option<Event>;

/// Errors that occur when trying to capture OS events.
/// Be careful on Mac, not setting accessibility does not cause an error
/// it justs ignores events.
#[derive(Debug)]
#[non_exhaustive]
pub enum ListenError {
    /// MacOS
    EventTapError,
    /// MacOS
    LoopSourceError,
    /// Linux
    MissingDisplayError,
    /// Linux
    KeyboardError,
    /// Linux
    RecordContextEnablingError,
    /// Linux
    RecordContextError,
    /// Linux
    XRecordExtensionError,
    /// Windows
    KeyHookError(u32),
    /// Windows
    MouseHookError(u32),
}

/// Errors that occur when trying to grab OS events.
/// Be careful on Mac, not setting accessibility does not cause an error
/// it justs ignores events.
#[derive(Debug)]
#[non_exhaustive]
pub enum GrabError {
    ListenError,
    /// MacOS
    EventTapError,
    /// MacOS
    LoopSourceError,
    /// Linux
    MissingDisplayError,
    /// Linux
    MissingScreenError,
    // Linux
    InvalidFileDescriptor,
    /// Linux
    KeyboardError,
    /// Windows
    KeyHookError(u32),
    /// Windows
    MouseHookError(u32),
    /// All
    SimulateError,
    /// All
    ExitGrabError(String),

    IoError(std::io::Error),
}

// impl From<std::io::Error> for GrabError {
//     fn from(err: std::io::Error) -> GrabError {
//         GrabError::IoError(err)
//     }
// }

/// Errors that occur when trying to get display size.
#[non_exhaustive]
#[derive(Debug)]
pub enum DisplayError {
    NoDisplay,
    ConversionError,
}

impl From<SimulateError> for GrabError {
    fn from(_: SimulateError) -> GrabError {
        GrabError::SimulateError
    }
}

/// Marking an error when we tried to simulate and event
#[derive(Debug)]
pub struct SimulateError;

impl Display for SimulateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not simulate event")
    }
}

impl std::error::Error for SimulateError {}

// Some keys from https://github.com/chromium/chromium/blob/main/ui/events/keycodes/dom/dom_code_data.inc

/// Key names based on physical location on the device
/// Merge Option(MacOS) and Alt(Windows, Linux) into Alt
/// Merge Windows (Windows), Meta(Linux), Command(MacOS) into Meta
/// Characters based on Qwerty layout, don't use this for characters as it WILL
/// depend on the layout. Use Event.name instead. Key modifiers gives those keys
/// a different value too.
/// Careful, on Windows KpReturn does not exist, it' s strictly equivalent to Return, also Keypad keys
/// get modified if NumLock is Off and ARE pagedown and so on.
use strum_macros::EnumIter; // 0.17.1
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, EnumIter)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum Key {
    /// Alt key on Linux and Windows (option key on macOS)
    Alt,
    AltGr,
    Backspace,
    CapsLock,
    ControlLeft,
    ControlRight,
    Delete,
    DownArrow,
    End,
    Escape,
    F1,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    Home,
    LeftArrow,
    /// also known as "windows", "super", and "command"
    MetaLeft,
    /// also known as "windows", "super", and "command"
    MetaRight,
    PageDown,
    PageUp,
    Return,
    RightArrow,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    UpArrow,
    PrintScreen,
    ScrollLock,
    Pause,
    NumLock,
    BackQuote,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Num0,
    Minus,
    Equal,
    KeyQ,
    KeyW,
    KeyE,
    KeyR,
    KeyT,
    KeyY,
    KeyU,
    KeyI,
    KeyO,
    KeyP,
    LeftBracket,
    RightBracket,
    KeyA,
    KeyS,
    KeyD,
    KeyF,
    KeyG,
    KeyH,
    KeyJ,
    KeyK,
    KeyL,
    SemiColon,
    Quote,
    BackSlash,
    IntlBackslash,
    IntlRo,   // Brazilian /? and Japanese _ 'ro'
    IntlYen,  // Japanese yen key.
    KanaMode, // Japanese Hiragana/Katakana key.
    KeyZ,
    KeyX,
    KeyC,
    KeyV,
    KeyB,
    KeyN,
    KeyM,
    Comma,
    Dot,
    Slash,
    Insert,
    KpReturn,
    KpMinus,
    KpPlus,
    KpMultiply,
    KpDivide,
    KpDecimal,
    KpEqual,
    KpComma,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    VolumeUp,
    VolumeDown,
    VolumeMute,
    Lang1, // Korean Hangul/English toggle key, and as the Kana key on the Apple Japanese keyboard.
    Lang2, // Korean Hanja conversion key, and as the Eisu key on the Apple Japanese keyboard.
    Lang3, // Japanese Katakana key.
    Lang4, // Japanese Hiragana key.
    Lang5, // Japanese Zenkaku/Hankaku (Fullwidth/halfwidth) key.
    Function,
    Apps,
    Cancel,
    Clear,
    Kana,
    Hangul,
    Junja,
    Final,
    Hanja,
    Hanji,
    Print,
    Select,
    Execute,
    Help,
    Sleep,
    Separator,
    Unknown(u32),
    RawKey(RawKey),
    // Keep new variants last to preserve existing serialized discriminants.
    /// Japanese Henkan (Convert) key.
    Convert,
    /// Japanese Muhenkan (NonConvert) key.
    NonConvert,
}

#[cfg(not(target_os = "macos"))]
pub type KeyCode = u32;
#[cfg(target_os = "macos")]
pub type KeyCode = crate::CGKeyCode;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, EnumIter)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum RawKey {
    ScanCode(KeyCode),
    WinVirtualKeycode(KeyCode),
    LinuxXorgKeycode(KeyCode),
    LinuxConsoleKeycode(KeyCode),
    MacVirtualKeycode(KeyCode),
}

impl Default for RawKey {
    fn default() -> Self {
        Self::ScanCode(0)
    }
}

/// Standard mouse buttons
/// Some mice have more than 3 buttons. These are not defined, and different
/// OSs will give different `Button::Unknown` values.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum Button {
    Left,
    Right,
    Middle,
    Unknown(u8),
}

/// In order to manage different OSs, the current EventType choices are a mix and
/// match to account for all possible events.
#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum EventType {
    /// The keys correspond to a standard qwerty layout, they don't correspond
    /// To the actual letter a user would use, that requires some layout logic to be added.
    KeyPress(Key),
    KeyRelease(Key),
    /// Mouse Button
    ButtonPress(Button),
    ButtonRelease(Button),
    /// Values in pixels. `EventType::MouseMove{x: 0, y: 0}` corresponds to the
    /// top left corner, with x increasing downward and y increasing rightward
    MouseMove {
        x: f64,
        y: f64,
    },
    /// `delta_y` represents vertical scroll and `delta_x` represents horizontal scroll.
    /// Positive values correspond to scrolling up or right and negative values
    /// correspond to scrolling down or left
    /// Note: Linux does not support horizontal scroll. When simulating scroll on Linux,
    /// only the sign of delta_y is considered, and not the magnitude to determine wheelup or wheeldown.
    Wheel {
        delta_x: i64,
        delta_y: i64,
    },
}

/// The Unicode information of input.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnicodeInfo {
    pub name: Option<String>,
    pub unicode: Vec<u16>,
    pub is_dead: bool,
}

/// When events arrive from the OS they get some additional information added from
/// EventType, which is the time when this event was received, and the name Option
/// which contains what characters should be emmitted from that event. This relies
/// on the OS layout and keyboard state machinery.
/// Caveat: Dead keys don't function on Linux(X11) yet. You will receive None for
/// a dead key, and the raw letter instead of accentuated letter.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Event {
    pub time: SystemTime,
    pub unicode: Option<UnicodeInfo>,
    pub event_type: EventType,
    // Linux: keysym
    // WIndows: vkcod
    pub platform_code: u32,
    pub position_code: u32,
    pub usb_hid: u32,
    #[cfg(target_os = "windows")]
    pub extra_data: winapi::shared::basetsd::ULONG_PTR,
    #[cfg(target_os = "macos")]
    pub extra_data: i64,
}

/// We can define a dummy Keyboard, that we will use to detect
/// what kind of EventType trigger some String. We get the currently used
/// layout for now !
/// Caveat : This is layout dependent. If your app needs to support
/// layout switching don't use this !
/// Caveat: On Linux, the dead keys mechanism is not implemented.
/// Caveat: Only shift and dead keys are implemented, Alt+unicode code on windows
/// won't work.
///
/// ```no_run
/// use rdev::{Keyboard, EventType, Key, KeyboardState};
///
/// let mut keyboard = Keyboard::new().unwrap();
/// let string = keyboard.add(&EventType::KeyPress(Key::KeyS)).unwrap().name.unwrap();
/// // string == Some("s")
/// ```
pub trait KeyboardState {
    /// Changes the keyboard state as if this event happened. we don't
    /// really hit the OS here, which might come handy to test what should happen
    /// if we were to hit said key.
    fn add(&mut self, event_type: &EventType) -> Option<UnicodeInfo>;

    // Resets the keyboard state as if we never touched it (no shift, caps_lock and so on)
    // fn reset(&mut self);
}
