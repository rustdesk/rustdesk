#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::clipboard::{update_clipboard, ClipboardSide};
use hbb_common::{message_proto::*, ResultType};
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref SCREENSHOT: Mutex<Screenshot> = Default::default();
}

pub enum ScreenshotAction {
    SaveAs(String),
    CopyToClipboard,
    Discard,
}

impl Default for ScreenshotAction {
    fn default() -> Self {
        Self::Discard
    }
}

impl From<&str> for ScreenshotAction {
    fn from(value: &str) -> Self {
        match value.chars().next() {
            Some('0') => {
                if let Some((pos, _)) = value.char_indices().nth(2) {
                    let substring = &value[pos..];
                    Self::SaveAs(substring.to_string())
                } else {
                    Self::default()
                }
            }
            Some('1') => Self::CopyToClipboard,
            Some('2') => Self::default(),
            _ => Self::default(),
        }
    }
}

impl Into<String> for ScreenshotAction {
    fn into(self) -> String {
        match self {
            Self::SaveAs(p) => format!("0:{p}"),
            Self::CopyToClipboard => "1".to_owned(),
            Self::Discard => "2".to_owned(),
        }
    }
}

#[derive(Default)]
pub struct Screenshot {
    data: Option<bytes::Bytes>,
}

impl Screenshot {
    fn set_screenshot(&mut self, data: bytes::Bytes) {
        self.data.replace(data);
    }

    fn handle_screenshot(&mut self, action: String) -> String {
        let Some(data) = self.data.take() else {
            return "No cached screenshot".to_owned();
        };
        match Self::handle_screenshot_(data, action) {
            Ok(()) => "".to_owned(),
            Err(e) => e.to_string(),
        }
    }

    fn handle_screenshot_(data: bytes::Bytes, action: String) -> ResultType<()> {
        match ScreenshotAction::from(&action as &str) {
            ScreenshotAction::SaveAs(p) => {
                std::fs::write(p, data)?;
            }
            ScreenshotAction::CopyToClipboard => {
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    let clips = vec![Clipboard {
                        compress: false,
                        content: data,
                        format: ClipboardFormat::ImagePng.into(),
                        ..Default::default()
                    }];
                    update_clipboard(clips, ClipboardSide::Client);
                }
            }
            ScreenshotAction::Discard => {}
        }
        Ok(())
    }
}

pub fn set_screenshot(data: bytes::Bytes) {
    SCREENSHOT.lock().unwrap().set_screenshot(data);
}

pub fn handle_screenshot(action: String) -> String {
    SCREENSHOT.lock().unwrap().handle_screenshot(action)
}
