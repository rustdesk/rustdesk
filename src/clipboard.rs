use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use arboard::{ClipboardData, ClipboardFormat};
use clipboard_master::{CallbackResult, ClipboardHandler, Master, Shutdown};
use hbb_common::{log, message_proto::*, ResultType};

pub const CLIPBOARD_NAME: &'static str = "clipboard";
pub const CLIPBOARD_INTERVAL: u64 = 333;

// This format is used to store the flag in the clipboard.
const RUSTDESK_CLIPBOARD_OWNER_FORMAT: &'static str = "dyn.com.rustdesk.owner";

lazy_static::lazy_static! {
    static ref ARBOARD_MTX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    // cache the clipboard msg
    static ref LAST_MULTI_CLIPBOARDS: Arc<Mutex<MultiClipboards>> = Arc::new(Mutex::new(MultiClipboards::new()));
}

const SUPPORTED_FORMATS: &[ClipboardFormat] = &[
    ClipboardFormat::Text,
    ClipboardFormat::Html,
    ClipboardFormat::Rtf,
    ClipboardFormat::ImageRgba,
    ClipboardFormat::ImagePng,
    ClipboardFormat::ImageSvg,
    ClipboardFormat::Special(RUSTDESK_CLIPBOARD_OWNER_FORMAT),
];

#[cfg(all(target_os = "linux", feature = "unix-file-copy-paste"))]
static X11_CLIPBOARD: once_cell::sync::OnceCell<x11_clipboard::Clipboard> =
    once_cell::sync::OnceCell::new();

#[cfg(all(target_os = "linux", feature = "unix-file-copy-paste"))]
fn get_clipboard() -> Result<&'static x11_clipboard::Clipboard, String> {
    X11_CLIPBOARD
        .get_or_try_init(|| x11_clipboard::Clipboard::new())
        .map_err(|e| e.to_string())
}

#[cfg(all(target_os = "linux", feature = "unix-file-copy-paste"))]
pub struct ClipboardContext {
    string_setter: x11rb::protocol::xproto::Atom,
    string_getter: x11rb::protocol::xproto::Atom,
    text_uri_list: x11rb::protocol::xproto::Atom,

    clip: x11rb::protocol::xproto::Atom,
    prop: x11rb::protocol::xproto::Atom,
}

#[cfg(all(target_os = "linux", feature = "unix-file-copy-paste"))]
fn parse_plain_uri_list(v: Vec<u8>) -> Result<String, String> {
    let text = String::from_utf8(v).map_err(|_| "ConversionFailure".to_owned())?;
    let mut list = String::new();
    for line in text.lines() {
        if !line.starts_with("file://") {
            continue;
        }
        let decoded = percent_encoding::percent_decode_str(line)
            .decode_utf8()
            .map_err(|_| "ConversionFailure".to_owned())?;
        list = list + "\n" + decoded.trim_start_matches("file://");
    }
    list = list.trim().to_owned();
    Ok(list)
}

#[cfg(all(target_os = "linux", feature = "unix-file-copy-paste"))]
impl ClipboardContext {
    pub fn new(_listen: bool) -> Result<Self, String> {
        let clipboard = get_clipboard()?;
        let string_getter = clipboard
            .getter
            .get_atom("UTF8_STRING")
            .map_err(|e| e.to_string())?;
        let string_setter = clipboard
            .setter
            .get_atom("UTF8_STRING")
            .map_err(|e| e.to_string())?;
        let text_uri_list = clipboard
            .getter
            .get_atom("text/uri-list")
            .map_err(|e| e.to_string())?;
        let prop = clipboard.getter.atoms.property;
        let clip = clipboard.getter.atoms.clipboard;
        Ok(Self {
            text_uri_list,
            string_setter,
            string_getter,
            clip,
            prop,
        })
    }

    pub fn get_text(&mut self) -> Result<String, String> {
        let clip = self.clip;
        let prop = self.prop;

        const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(120);

        let text_content = get_clipboard()?
            .load(clip, self.string_getter, prop, TIMEOUT)
            .map_err(|e| e.to_string())?;

        let file_urls = get_clipboard()?.load(clip, self.text_uri_list, prop, TIMEOUT)?;

        if file_urls.is_err() || file_urls.as_ref().is_empty() {
            log::trace!("clipboard get text, no file urls");
            return String::from_utf8(text_content).map_err(|e| e.to_string());
        }

        let file_urls = parse_plain_uri_list(file_urls)?;

        let text_content = String::from_utf8(text_content).map_err(|e| e.to_string())?;

        if text_content.trim() == file_urls.trim() {
            log::trace!("clipboard got text but polluted");
            return Err(String::from("polluted text"));
        }

        Ok(text_content)
    }

    pub fn set_text(&mut self, content: String) -> Result<(), String> {
        let clip = self.clip;

        let value = content.clone().into_bytes();
        get_clipboard()?
            .store(clip, self.string_setter, value)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn check_clipboard(ctx: &mut Option<ClipboardContext>, side: ClipboardSide) -> Option<Message> {
    if ctx.is_none() {
        *ctx = ClipboardContext::new(true).ok();
    }
    let ctx2 = ctx.as_mut()?;
    let content = ctx2.get(side);
    if let Ok(content) = content {
        if !content.is_empty() {
            let mut msg = Message::new();
            let clipboards = proto::create_multi_clipboards(content);
            msg.set_multi_clipboards(clipboards.clone());
            *LAST_MULTI_CLIPBOARDS.lock().unwrap() = clipboards;
            return Some(msg);
        }
    }
    None
}

fn update_clipboard_(multi_clipboards: Vec<Clipboard>, side: ClipboardSide) {
    let mut to_update_data = proto::from_multi_clipbards(multi_clipboards);
    if to_update_data.is_empty() {
        return;
    }
    match ClipboardContext::new(false) {
        Ok(mut ctx) => {
            to_update_data.push(ClipboardData::Special((
                RUSTDESK_CLIPBOARD_OWNER_FORMAT.to_owned(),
                side.get_owner_data(),
            )));
            if let Err(e) = ctx.set(&to_update_data) {
                log::debug!("Failed to set clipboard: {}", e);
            } else {
                log::debug!("{} updated on {}", CLIPBOARD_NAME, side);
            }
        }
        Err(err) => {
            log::error!("Failed to create clipboard context: {}", err);
        }
    }
}

pub fn update_clipboard(multi_clipboards: Vec<Clipboard>, side: ClipboardSide) {
    std::thread::spawn(move || {
        update_clipboard_(multi_clipboards, side);
    });
}

#[cfg(not(any(all(target_os = "linux", feature = "unix-file-copy-paste"))))]
pub struct ClipboardContext {
    inner: arboard::Clipboard,
    counter: (Arc<AtomicU64>, u64),
    shutdown: Option<Shutdown>,
}

#[cfg(not(any(all(target_os = "linux", feature = "unix-file-copy-paste"))))]
#[allow(unreachable_code)]
impl ClipboardContext {
    pub fn new(listen: bool) -> ResultType<ClipboardContext> {
        let board;
        #[cfg(not(target_os = "linux"))]
        {
            board = arboard::Clipboard::new()?;
        }
        #[cfg(target_os = "linux")]
        {
            let mut i = 1;
            loop {
                // Try 5 times to create clipboard
                // Arboard::new() connect to X server or Wayland compositor, which shoud be ok at most time
                // But sometimes, the connection may fail, so we retry here.
                match arboard::Clipboard::new() {
                    Ok(x) => {
                        board = x;
                        break;
                    }
                    Err(e) => {
                        if i == 5 {
                            return Err(e.into());
                        } else {
                            std::thread::sleep(std::time::Duration::from_millis(30 * i));
                        }
                    }
                }
                i += 1;
            }
        }

        // starting from 1 so that we can always get initial clipboard data no matter if change
        let change_count: Arc<AtomicU64> = Arc::new(AtomicU64::new(1));
        let mut shutdown = None;
        if listen {
            struct Handler(Arc<AtomicU64>);
            impl ClipboardHandler for Handler {
                fn on_clipboard_change(&mut self) -> CallbackResult {
                    self.0.fetch_add(1, Ordering::SeqCst);
                    CallbackResult::Next
                }

                fn on_clipboard_error(&mut self, error: std::io::Error) -> CallbackResult {
                    log::trace!("Error of clipboard listener: {}", error);
                    CallbackResult::Next
                }
            }
            let change_count_cloned = change_count.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessage#:~:text=The%20window%20must%20belong%20to%20the%20current%20thread.
            std::thread::spawn(move || match Master::new(Handler(change_count_cloned)) {
                Ok(mut master) => {
                    tx.send(master.shutdown_channel()).ok();
                    log::debug!("Clipboard listener started");
                    if let Err(err) = master.run() {
                        log::error!("Failed to run clipboard listener: {}", err);
                    } else {
                        log::debug!("Clipboard listener stopped");
                    }
                }
                Err(err) => {
                    log::error!("Failed to create clipboard listener: {}", err);
                }
            });
            if let Ok(st) = rx.recv() {
                shutdown = Some(st);
            }
        }
        Ok(ClipboardContext {
            inner: board,
            counter: (change_count, 0),
            shutdown,
        })
    }

    #[inline]
    pub fn change_count(&self) -> u64 {
        debug_assert!(self.shutdown.is_some());
        self.counter.0.load(Ordering::SeqCst)
    }

    pub fn get(&mut self, side: ClipboardSide) -> ResultType<Vec<ClipboardData>> {
        let cn = self.change_count();
        let _lock = ARBOARD_MTX.lock().unwrap();
        if cn != self.counter.1 {
            self.counter.1 = cn;
            let data = self.inner.get_formats(SUPPORTED_FORMATS)?;
            if !data.is_empty() {
                for c in data.iter() {
                    if let ClipboardData::Special((_, d)) = c {
                        if side.is_owner(d) {
                            return Ok(vec![]);
                        }
                    }
                }
            }
            return Ok(data);
        }
        Ok(vec![])
    }

    fn set(&mut self, data: &[ClipboardData]) -> ResultType<()> {
        let _lock = ARBOARD_MTX.lock().unwrap();
        self.inner.set_formats(data)?;
        Ok(())
    }
}

impl Drop for ClipboardContext {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.signal();
        }
    }
}

pub fn is_support_multi_clipboard(peer_version: &str, peer_platform: &str) -> bool {
    use hbb_common::get_version_number;
    get_version_number(peer_version) >= get_version_number("1.2.7")
        && !["", "Android", &whoami::Platform::Ios.to_string()].contains(&peer_platform)
}

pub fn get_cache_msg(peer_version: &str, peer_platform: &str) -> Option<Message> {
    let multi_clipboards = LAST_MULTI_CLIPBOARDS.lock().unwrap().clone();
    if multi_clipboards.clipboards.is_empty() {
        return None;
    }

    let mut msg = Message::new();
    if is_support_multi_clipboard(peer_version, peer_platform) {
        msg.set_multi_clipboards(multi_clipboards);
    } else {
        for clipboard in multi_clipboards.clipboards.iter() {
            if clipboard.format.enum_value() == Ok(hbb_common::message_proto::ClipboardFormat::Text)
            {
                msg.set_clipboard(clipboard.clone());
                break;
            }
        }
    }
    Some(msg)
}

pub fn reset_cache() {
    *LAST_MULTI_CLIPBOARDS.lock().unwrap() = MultiClipboards::new();
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ClipboardSide {
    Host,
    Client,
}

impl ClipboardSide {
    // 01: the clipboard is owned by the host
    // 10: the clipboard is owned by the client
    fn get_owner_data(&self) -> Vec<u8> {
        match self {
            ClipboardSide::Host => vec![0b01],
            ClipboardSide::Client => vec![0b10],
        }
    }

    fn is_owner(&self, data: &[u8]) -> bool {
        if data.len() == 0 {
            return false;
        }
        match self {
            ClipboardSide::Host => data[0] & 0b01 == 0b01,
            ClipboardSide::Client => data[0] & 0b10 == 0b10,
        }
    }
}

impl std::fmt::Display for ClipboardSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardSide::Host => write!(f, "host"),
            ClipboardSide::Client => write!(f, "client"),
        }
    }
}

pub use proto::get_msg_if_not_support_multi_clip;
mod proto {
    use arboard::ClipboardData;
    use hbb_common::{
        compress::{compress as compress_func, decompress},
        message_proto::{Clipboard, ClipboardFormat, Message, MultiClipboards},
    };

    fn plain_to_proto(s: String, format: ClipboardFormat) -> Clipboard {
        let compressed = compress_func(s.as_bytes());
        let compress = compressed.len() < s.as_bytes().len();
        let content = if compress {
            compressed
        } else {
            s.bytes().collect::<Vec<u8>>()
        };
        Clipboard {
            compress,
            content: content.into(),
            format: format.into(),
            ..Default::default()
        }
    }

    fn image_to_proto(a: arboard::ImageData) -> Clipboard {
        match &a {
            arboard::ImageData::Rgba(rgba) => {
                let compressed = compress_func(&a.bytes());
                let compress = compressed.len() < a.bytes().len();
                let content = if compress {
                    compressed
                } else {
                    a.bytes().to_vec()
                };
                Clipboard {
                    compress,
                    content: content.into(),
                    width: rgba.width as _,
                    height: rgba.height as _,
                    format: ClipboardFormat::ImageRgba.into(),
                    ..Default::default()
                }
            }
            arboard::ImageData::Png(png) => Clipboard {
                compress: false,
                content: png.to_owned().to_vec().into(),
                format: ClipboardFormat::ImagePng.into(),
                ..Default::default()
            },
            arboard::ImageData::Svg(_) => {
                let compressed = compress_func(&a.bytes());
                let compress = compressed.len() < a.bytes().len();
                let content = if compress {
                    compressed
                } else {
                    a.bytes().to_vec()
                };
                Clipboard {
                    compress,
                    content: content.into(),
                    format: ClipboardFormat::ImageSvg.into(),
                    ..Default::default()
                }
            }
        }
    }

    fn clipboard_data_to_proto(data: ClipboardData) -> Option<Clipboard> {
        let d = match data {
            ClipboardData::Text(s) => plain_to_proto(s, ClipboardFormat::Text),
            ClipboardData::Rtf(s) => plain_to_proto(s, ClipboardFormat::Rtf),
            ClipboardData::Html(s) => plain_to_proto(s, ClipboardFormat::Html),
            ClipboardData::Image(a) => image_to_proto(a),
            _ => return None,
        };
        Some(d)
    }

    pub fn create_multi_clipboards(vec_data: Vec<ClipboardData>) -> MultiClipboards {
        MultiClipboards {
            clipboards: vec_data
                .into_iter()
                .filter_map(clipboard_data_to_proto)
                .collect(),
            ..Default::default()
        }
    }

    fn from_clipboard(clipboard: Clipboard) -> Option<ClipboardData> {
        let data = if clipboard.compress {
            decompress(&clipboard.content)
        } else {
            clipboard.content.into()
        };
        match clipboard.format.enum_value() {
            Ok(ClipboardFormat::Text) => String::from_utf8(data).ok().map(ClipboardData::Text),
            Ok(ClipboardFormat::Rtf) => String::from_utf8(data).ok().map(ClipboardData::Rtf),
            Ok(ClipboardFormat::Html) => String::from_utf8(data).ok().map(ClipboardData::Html),
            Ok(ClipboardFormat::ImageRgba) => Some(ClipboardData::Image(arboard::ImageData::rgba(
                clipboard.width as _,
                clipboard.height as _,
                data.into(),
            ))),
            Ok(ClipboardFormat::ImagePng) => {
                Some(ClipboardData::Image(arboard::ImageData::png(data.into())))
            }
            Ok(ClipboardFormat::ImageSvg) => Some(ClipboardData::Image(arboard::ImageData::svg(
                std::str::from_utf8(&data).unwrap_or_default(),
            ))),
            _ => None,
        }
    }

    pub fn from_multi_clipbards(multi_clipboards: Vec<Clipboard>) -> Vec<ClipboardData> {
        multi_clipboards
            .into_iter()
            .filter_map(from_clipboard)
            .collect()
    }

    pub fn get_msg_if_not_support_multi_clip(
        version: &str,
        platform: &str,
        multi_clipboards: &MultiClipboards,
    ) -> Option<Message> {
        if crate::clipboard::is_support_multi_clipboard(version, platform) {
            return None;
        }
        let mut msg = Message::new();
        // Find the first text clipboard and send it.
        for clipboard in multi_clipboards.clipboards.iter() {
            if clipboard.format.enum_value() == Ok(ClipboardFormat::Text) {
                msg.set_clipboard(clipboard.clone());
                break;
            }
        }
        Some(msg)
    }
}
