use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use clipboard_master::{CallbackResult, ClipboardHandler, Master, Shutdown};
use hbb_common::{
    allow_err,
    compress::{compress as compress_func, decompress},
    log,
    message_proto::*,
    ResultType,
};

pub const CLIPBOARD_NAME: &'static str = "clipboard";
pub const CLIPBOARD_INTERVAL: u64 = 333;

lazy_static::lazy_static! {
    pub static ref CONTENT: Arc<Mutex<ClipboardData>> = Default::default();
    static ref ARBOARD_MTX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
}

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

        let file_urls = get_clipboard()?.load(clip, self.text_uri_list, prop, TIMEOUT);

        if file_urls.is_err() || file_urls.as_ref().unwrap().is_empty() {
            log::trace!("clipboard get text, no file urls");
            return String::from_utf8(text_content).map_err(|e| e.to_string());
        }

        let file_urls = parse_plain_uri_list(file_urls.unwrap())?;

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

pub fn check_clipboard(
    ctx: &mut Option<ClipboardContext>,
    old: Option<Arc<Mutex<ClipboardData>>>,
) -> Option<Message> {
    if ctx.is_none() {
        *ctx = ClipboardContext::new(true).ok();
    }
    let ctx2 = ctx.as_mut()?;
    let side = if old.is_none() { "host" } else { "client" };
    let old = if let Some(old) = old {
        old
    } else {
        CONTENT.clone()
    };
    let content = ctx2.get();
    if let Ok(content) = content {
        if !content.is_empty() {
            let changed = content != *old.lock().unwrap();
            if changed {
                log::info!("{} update found on {}", CLIPBOARD_NAME, side);
                let msg = content.create_msg();
                *old.lock().unwrap() = content;
                return Some(msg);
            }
        }
    }
    None
}

fn update_clipboard_(clipboard: Clipboard, old: Option<Arc<Mutex<ClipboardData>>>) {
    let content = ClipboardData::from_msg(clipboard);
    if content.is_empty() {
        return;
    }
    match ClipboardContext::new(false) {
        Ok(mut ctx) => {
            let side = if old.is_none() { "host" } else { "client" };
            let old = if let Some(old) = old {
                old
            } else {
                CONTENT.clone()
            };
            allow_err!(ctx.set(&content));
            *old.lock().unwrap() = content;
            log::debug!("{} updated on {}", CLIPBOARD_NAME, side);
        }
        Err(err) => {
            log::error!("Failed to create clipboard context: {}", err);
        }
    }
}

pub fn update_clipboard(clipboard: Clipboard, old: Option<Arc<Mutex<ClipboardData>>>) {
    std::thread::spawn(move || {
        update_clipboard_(clipboard, old);
    });
}

#[derive(Clone)]
pub enum ClipboardData {
    Text(String),
    Image(arboard::ImageData<'static>, u64),
    Empty,
}

impl Default for ClipboardData {
    fn default() -> Self {
        ClipboardData::Empty
    }
}

impl ClipboardData {
    fn image(image: arboard::ImageData<'static>) -> ClipboardData {
        let hash = 0;
        /*
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        image.bytes.hash(&mut hasher);
        let hash = hasher.finish();
        */
        ClipboardData::Image(image, hash)
    }

    pub fn is_empty(&self) -> bool {
        match self {
            ClipboardData::Empty => true,
            ClipboardData::Text(s) => s.is_empty(),
            ClipboardData::Image(a, _) => a.bytes.is_empty(),
        }
    }

    fn from_msg(clipboard: Clipboard) -> Self {
        let is_image = clipboard.width > 0 && clipboard.height > 0;
        let data = if clipboard.compress {
            decompress(&clipboard.content)
        } else {
            clipboard.content.into()
        };
        if is_image {
            ClipboardData::Image(
                arboard::ImageData {
                    bytes: data.into(),
                    width: clipboard.width as _,
                    height: clipboard.height as _,
                },
                0,
            )
        } else {
            if let Ok(content) = String::from_utf8(data) {
                ClipboardData::Text(content)
            } else {
                ClipboardData::Empty
            }
        }
    }

    pub fn create_msg(&self) -> Message {
        let mut msg = Message::new();

        match self {
            ClipboardData::Text(s) => {
                let compressed = compress_func(s.as_bytes());
                let compress = compressed.len() < s.as_bytes().len();
                let content = if compress {
                    compressed
                } else {
                    s.clone().into_bytes()
                };
                msg.set_clipboard(Clipboard {
                    compress,
                    content: content.into(),
                    ..Default::default()
                });
            }
            ClipboardData::Image(a, _) => {
                let compressed = compress_func(&a.bytes);
                let compress = compressed.len() < a.bytes.len();
                let content = if compress {
                    compressed
                } else {
                    a.bytes.to_vec()
                };
                msg.set_clipboard(Clipboard {
                    compress,
                    content: content.into(),
                    width: a.width as _,
                    height: a.height as _,
                    ..Default::default()
                });
            }
            _ => {}
        }
        msg
    }
}

impl PartialEq for ClipboardData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ClipboardData::Text(a), ClipboardData::Text(b)) => a == b,
            (ClipboardData::Image(a, _), ClipboardData::Image(b, _)) => {
                a.width == b.width && a.height == b.height && a.bytes == b.bytes
            }
            (ClipboardData::Empty, ClipboardData::Empty) => true,
            _ => false,
        }
    }
}

#[cfg(not(any(all(target_os = "linux", feature = "unix-file-copy-paste"))))]
pub struct ClipboardContext(arboard::Clipboard, (Arc<AtomicU64>, u64), Option<Shutdown>);

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
        Ok(ClipboardContext(board, (change_count, 0), shutdown))
    }

    #[inline]
    pub fn change_count(&self) -> u64 {
        debug_assert!(self.2.is_some());
        self.1 .0.load(Ordering::SeqCst)
    }

    pub fn get(&mut self) -> ResultType<ClipboardData> {
        let cn = self.change_count();
        let _lock = ARBOARD_MTX.lock().unwrap();
        // only for image for the time being,
        // because I do not want to change behavior of text clipboard for the time being
        if cn != self.1 .1 {
            self.1 .1 = cn;
            if let Ok(image) = self.0.get_image() {
                if image.width > 0 && image.height > 0 {
                    return Ok(ClipboardData::image(image));
                }
            }
        }
        Ok(ClipboardData::Text(self.0.get_text()?))
    }

    fn set(&mut self, data: &ClipboardData) -> ResultType<()> {
        let _lock = ARBOARD_MTX.lock().unwrap();
        match data {
            ClipboardData::Text(s) => self.0.set_text(s)?,
            ClipboardData::Image(a, _) => self.0.set_image(a.clone())?,
            _ => {}
        }
        Ok(())
    }
}

impl Drop for ClipboardContext {
    fn drop(&mut self) {
        if let Some(shutdown) = self.2.take() {
            let _ = shutdown.signal();
        }
    }
}
