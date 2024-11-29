#[cfg(not(target_os = "android"))]
use arboard::{ClipboardData, ClipboardFormat};
#[cfg(not(target_os = "android"))]
use clipboard_master::{ClipboardHandler, Master, Shutdown};
use hbb_common::{bail, log, message_proto::*, ResultType};
use std::{
    sync::{mpsc::Sender, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

pub const CLIPBOARD_NAME: &'static str = "clipboard";
pub const CLIPBOARD_INTERVAL: u64 = 333;

// This format is used to store the flag in the clipboard.
const RUSTDESK_CLIPBOARD_OWNER_FORMAT: &'static str = "dyn.com.rustdesk.owner";

// Add special format for Excel XML Spreadsheet
const CLIPBOARD_FORMAT_EXCEL_XML_SPREADSHEET: &'static str = "XML Spreadsheet";

#[cfg(not(target_os = "android"))]
lazy_static::lazy_static! {
    static ref ARBOARD_MTX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    // cache the clipboard msg
    static ref LAST_MULTI_CLIPBOARDS: Arc<Mutex<MultiClipboards>> = Arc::new(Mutex::new(MultiClipboards::new()));
    // For updating in server and getting content in cm.
    // Clipboard on Linux is "server--clients" mode.
    // The clipboard content is owned by the server and passed to the clients when requested.
    // Plain text is the only exception, it does not require the server to be present.
    static ref CLIPBOARD_CTX: Arc<Mutex<Option<ClipboardContext>>> = Arc::new(Mutex::new(None));
}

#[cfg(not(target_os = "android"))]
const CLIPBOARD_GET_MAX_RETRY: usize = 3;
#[cfg(not(target_os = "android"))]
const CLIPBOARD_GET_RETRY_INTERVAL_DUR: Duration = Duration::from_millis(33);

#[cfg(not(target_os = "android"))]
const SUPPORTED_FORMATS: &[ClipboardFormat] = &[
    ClipboardFormat::Text,
    ClipboardFormat::Html,
    ClipboardFormat::Rtf,
    ClipboardFormat::ImageRgba,
    ClipboardFormat::ImagePng,
    ClipboardFormat::ImageSvg,
    ClipboardFormat::Special(CLIPBOARD_FORMAT_EXCEL_XML_SPREADSHEET),
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
    pub fn new() -> Result<Self, String> {
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

#[cfg(not(target_os = "android"))]
pub fn check_clipboard(
    ctx: &mut Option<ClipboardContext>,
    side: ClipboardSide,
    force: bool,
) -> Option<Message> {
    if ctx.is_none() {
        *ctx = ClipboardContext::new().ok();
    }
    let ctx2 = ctx.as_mut()?;
    match ctx2.get(side, force) {
        Ok(content) => {
            if !content.is_empty() {
                let mut msg = Message::new();
                let clipboards = proto::create_multi_clipboards(content);
                msg.set_multi_clipboards(clipboards.clone());
                *LAST_MULTI_CLIPBOARDS.lock().unwrap() = clipboards;
                return Some(msg);
            }
        }
        Err(e) => {
            log::error!("Failed to get clipboard content. {}", e);
        }
    }
    None
}

#[cfg(target_os = "windows")]
pub fn check_clipboard_cm() -> ResultType<MultiClipboards> {
    let mut ctx = CLIPBOARD_CTX.lock().unwrap();
    if ctx.is_none() {
        match ClipboardContext::new() {
            Ok(x) => {
                *ctx = Some(x);
            }
            Err(e) => {
                hbb_common::bail!("Failed to create clipboard context: {}", e);
            }
        }
    }
    if let Some(ctx) = ctx.as_mut() {
        let content = ctx.get(ClipboardSide::Host, false)?;
        let clipboards = proto::create_multi_clipboards(content);
        Ok(clipboards)
    } else {
        hbb_common::bail!("Failed to create clipboard context");
    }
}

#[cfg(not(target_os = "android"))]
fn update_clipboard_(multi_clipboards: Vec<Clipboard>, side: ClipboardSide) {
    let mut to_update_data = proto::from_multi_clipbards(multi_clipboards);
    if to_update_data.is_empty() {
        return;
    }
    let mut ctx = CLIPBOARD_CTX.lock().unwrap();
    if ctx.is_none() {
        match ClipboardContext::new() {
            Ok(x) => {
                *ctx = Some(x);
            }
            Err(e) => {
                log::error!("Failed to create clipboard context: {}", e);
                return;
            }
        }
    }
    if let Some(ctx) = ctx.as_mut() {
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
}

#[cfg(not(target_os = "android"))]
pub fn update_clipboard(multi_clipboards: Vec<Clipboard>, side: ClipboardSide) {
    std::thread::spawn(move || {
        update_clipboard_(multi_clipboards, side);
    });
}

#[cfg(not(target_os = "android"))]
#[cfg(not(any(all(target_os = "linux", feature = "unix-file-copy-paste"))))]
pub struct ClipboardContext {
    inner: arboard::Clipboard,
}

#[cfg(not(target_os = "android"))]
#[cfg(not(any(all(target_os = "linux", feature = "unix-file-copy-paste"))))]
#[allow(unreachable_code)]
impl ClipboardContext {
    pub fn new() -> ResultType<ClipboardContext> {
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

        Ok(ClipboardContext { inner: board })
    }

    fn get_formats(&mut self, formats: &[ClipboardFormat]) -> ResultType<Vec<ClipboardData>> {
        // If there're multiple threads or processes trying to access the clipboard at the same time,
        // the previous clipboard owner will fail to access the clipboard.
        // `GetLastError()` will return `ERROR_CLIPBOARD_NOT_OPEN` (OSError(1418): Thread does not have a clipboard open) at this time.
        // See https://github.com/rustdesk-org/arboard/blob/747ab2d9b40a5c9c5102051cf3b0bb38b4845e60/src/platform/windows.rs#L34
        //
        // This is a common case on Windows, so we retry here.
        // Related issues:
        // https://github.com/rustdesk/rustdesk/issues/9263
        // https://github.com/rustdesk/rustdesk/issues/9222#issuecomment-2329233175
        for i in 0..CLIPBOARD_GET_MAX_RETRY {
            match self.inner.get_formats(SUPPORTED_FORMATS) {
                Ok(data) => {
                    return Ok(data
                        .into_iter()
                        .filter(|c| !matches!(c, arboard::ClipboardData::None))
                        .collect())
                }
                Err(e) => match e {
                    arboard::Error::ClipboardOccupied => {
                        log::debug!("Failed to get clipboard formats, clipboard is occupied, retrying... {}", i + 1);
                        std::thread::sleep(CLIPBOARD_GET_RETRY_INTERVAL_DUR);
                    }
                    _ => {
                        log::error!("Failed to get clipboard formats, {}", e);
                        return Err(e.into());
                    }
                },
            }
        }
        bail!("Failed to get clipboard formats, clipboard is occupied, {CLIPBOARD_GET_MAX_RETRY} retries failed");
    }

    pub fn get(&mut self, side: ClipboardSide, force: bool) -> ResultType<Vec<ClipboardData>> {
        let _lock = ARBOARD_MTX.lock().unwrap();
        let data = self.get_formats(SUPPORTED_FORMATS)?;
        if data.is_empty() {
            return Ok(data);
        }
        if !force {
            for c in data.iter() {
                if let ClipboardData::Special((s, d)) = c {
                    if s == RUSTDESK_CLIPBOARD_OWNER_FORMAT && side.is_owner(d) {
                        return Ok(vec![]);
                    }
                }
            }
        }
        Ok(data
            .into_iter()
            .filter(|c| match c {
                ClipboardData::Special((s, _)) => s != RUSTDESK_CLIPBOARD_OWNER_FORMAT,
                _ => true,
            })
            .collect())
    }

    fn set(&mut self, data: &[ClipboardData]) -> ResultType<()> {
        let _lock = ARBOARD_MTX.lock().unwrap();
        self.inner.set_formats(data)?;
        Ok(())
    }
}

pub fn is_support_multi_clipboard(peer_version: &str, peer_platform: &str) -> bool {
    use hbb_common::get_version_number;
    if get_version_number(peer_version) < get_version_number("1.3.0") {
        return false;
    }
    if ["", &whoami::Platform::Ios.to_string()].contains(&peer_platform) {
        return false;
    }
    if "Android" == peer_platform && get_version_number(peer_version) < get_version_number("1.3.3")
    {
        return false;
    }
    true
}

#[cfg(not(target_os = "android"))]
pub fn get_current_clipboard_msg(
    peer_version: &str,
    peer_platform: &str,
    side: ClipboardSide,
) -> Option<Message> {
    let mut multi_clipboards = LAST_MULTI_CLIPBOARDS.lock().unwrap();
    if multi_clipboards.clipboards.is_empty() {
        let mut ctx = ClipboardContext::new().ok()?;
        *multi_clipboards = proto::create_multi_clipboards(ctx.get(side, true).ok()?);
    }
    if multi_clipboards.clipboards.is_empty() {
        return None;
    }

    if is_support_multi_clipboard(peer_version, peer_platform) {
        let mut msg = Message::new();
        msg.set_multi_clipboards(multi_clipboards.clone());
        Some(msg)
    } else {
        // Find the first text clipboard and send it.
        multi_clipboards
            .clipboards
            .iter()
            .find(|c| c.format.enum_value() == Ok(hbb_common::message_proto::ClipboardFormat::Text))
            .map(|c| {
                let mut msg = Message::new();
                msg.set_clipboard(c.clone());
                msg
            })
    }
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
        data[0] & 0b11 != 0
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

#[cfg(not(target_os = "android"))]
pub fn start_clipbard_master_thread(
    handler: impl ClipboardHandler + Send + 'static,
    tx_start_res: Sender<(Option<Shutdown>, String)>,
) -> JoinHandle<()> {
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessage#:~:text=The%20window%20must%20belong%20to%20the%20current%20thread.
    let h = std::thread::spawn(move || match Master::new(handler) {
        Ok(mut master) => {
            tx_start_res
                .send((Some(master.shutdown_channel()), "".to_owned()))
                .ok();
            log::debug!("Clipboard listener started");
            if let Err(err) = master.run() {
                log::error!("Failed to run clipboard listener: {}", err);
            } else {
                log::debug!("Clipboard listener stopped");
            }
        }
        Err(err) => {
            tx_start_res
                .send((
                    None,
                    format!("Failed to create clipboard listener: {}", err),
                ))
                .ok();
        }
    });
    h
}

pub use proto::get_msg_if_not_support_multi_clip;
mod proto {
    #[cfg(not(target_os = "android"))]
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

    #[cfg(not(target_os = "android"))]
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

    fn special_to_proto(d: Vec<u8>, s: String) -> Clipboard {
        let compressed = compress_func(&d);
        let compress = compressed.len() < d.len();
        let content = if compress {
            compressed
        } else {
            s.bytes().collect::<Vec<u8>>()
        };
        Clipboard {
            compress,
            content: content.into(),
            format: ClipboardFormat::Special.into(),
            special_name: s,
            ..Default::default()
        }
    }

    #[cfg(not(target_os = "android"))]
    fn clipboard_data_to_proto(data: ClipboardData) -> Option<Clipboard> {
        let d = match data {
            ClipboardData::Text(s) => plain_to_proto(s, ClipboardFormat::Text),
            ClipboardData::Rtf(s) => plain_to_proto(s, ClipboardFormat::Rtf),
            ClipboardData::Html(s) => plain_to_proto(s, ClipboardFormat::Html),
            ClipboardData::Image(a) => image_to_proto(a),
            ClipboardData::Special((s, d)) => special_to_proto(d, s),
            _ => return None,
        };
        Some(d)
    }

    #[cfg(not(target_os = "android"))]
    pub fn create_multi_clipboards(vec_data: Vec<ClipboardData>) -> MultiClipboards {
        MultiClipboards {
            clipboards: vec_data
                .into_iter()
                .filter_map(clipboard_data_to_proto)
                .collect(),
            ..Default::default()
        }
    }

    #[cfg(not(target_os = "android"))]
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
            Ok(ClipboardFormat::Special) => {
                Some(ClipboardData::Special((clipboard.special_name, data)))
            }
            _ => None,
        }
    }

    #[cfg(not(target_os = "android"))]
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

        // Find the first text clipboard and send it.
        multi_clipboards
            .clipboards
            .iter()
            .find(|c| c.format.enum_value() == Ok(ClipboardFormat::Text))
            .map(|c| {
                let mut msg = Message::new();
                msg.set_clipboard(c.clone());
                msg
            })
    }
}

#[cfg(target_os = "android")]
pub fn handle_msg_clipboard(mut cb: Clipboard) {
    use hbb_common::protobuf::Message;

    if cb.compress {
        cb.content = bytes::Bytes::from(hbb_common::compress::decompress(&cb.content));
    }
    let multi_clips = MultiClipboards {
        clipboards: vec![cb],
        ..Default::default()
    };
    if let Ok(bytes) = multi_clips.write_to_bytes() {
        let _ = scrap::android::ffi::call_clipboard_manager_update_clipboard(&bytes);
    }
}

#[cfg(target_os = "android")]
pub fn handle_msg_multi_clipboards(mut mcb: MultiClipboards) {
    use hbb_common::protobuf::Message;

    for cb in mcb.clipboards.iter_mut() {
        if cb.compress {
            cb.content = bytes::Bytes::from(hbb_common::compress::decompress(&cb.content));
        }
    }
    if let Ok(bytes) = mcb.write_to_bytes() {
        let _ = scrap::android::ffi::call_clipboard_manager_update_clipboard(&bytes);
    }
}

#[cfg(target_os = "android")]
pub fn get_clipboards_msg(client: bool) -> Option<Message> {
    let mut clipboards = scrap::android::ffi::get_clipboards(client)?;
    let mut msg = Message::new();
    for c in &mut clipboards.clipboards {
        let compressed = hbb_common::compress::compress(&c.content);
        let compress = compressed.len() < c.content.len();
        if compress {
            c.content = compressed.into();
        }
        c.compress = compress;
    }
    msg.set_multi_clipboards(clipboards);
    Some(msg)
}
