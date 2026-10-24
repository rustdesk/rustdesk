use base::message_proto::KeyEvent;
use rdev::{Event, EventType, Key};

const HIGH_SURROGATES: std::ops::RangeInclusive<u16> = 0xD800..=0xDBFF;

pub(super) fn is_text_event(event: &Event) -> bool {
    let key = match event.event_type {
        EventType::KeyPress(key) | EventType::KeyRelease(key) => key,
        _ => return false,
    };
    #[cfg(target_os = "macos")]
    return key == Key::Unknown(u32::from(rdev::kVK_ANSI_A));
    #[cfg(target_os = "linux")]
    return matches!(key, Key::Unknown(_));
}

#[derive(Default)]
pub(super) struct UnicodeInput {
    high_surrogate: Option<u16>,
}

impl UnicodeInput {
    pub(super) fn decode(&mut self, event: &Event) -> Result<Option<KeyEvent>, &'static str> {
        if matches!(event.event_type, EventType::KeyRelease(_)) {
            return Ok(None);
        }
        let mut units: Vec<_> = self.high_surrogate.take().into_iter().collect();
        let unicode = event
            .unicode
            .as_ref()
            .ok_or("Unknown key has no Unicode text")?;
        if unicode.is_dead {
            return Ok(None);
        }
        if unicode.unicode.is_empty() {
            let text = unicode
                .name
                .as_ref()
                .ok_or("Unknown key has no Unicode text")?;
            units.extend(text.encode_utf16());
        } else {
            units.extend_from_slice(&unicode.unicode);
        }
        if units
            .last()
            .is_some_and(|unit| HIGH_SURROGATES.contains(unit))
        {
            self.high_surrogate = units.pop();
        }
        let text = String::from_utf16(&units).map_err(|_| "Invalid Unicode keyboard sequence")?;
        if text.is_empty() {
            return Ok(None);
        }
        let mut event = KeyEvent::new();
        event.set_seq(text);
        Ok(Some(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autotype_text_survives_surrogate_key_releases() {
        let expected = "a8*6 Hello café Grüße 日本語 한국어 汉字 Привет Ελληνικά مرحبا नमस्ते 😀🦀END";
        let mut input = UnicodeInput::default();
        let mut received = String::new();
        let units: Vec<_> = expected.encode_utf16().collect();
        for unit in units {
            let mut event = Event {
                event_type: EventType::KeyPress(Key::Unknown(0)),
                time: std::time::SystemTime::now(),
                unicode: Some(rdev::UnicodeInfo {
                    name: String::from_utf16(&[unit]).ok(),
                    unicode: vec![unit],
                    is_dead: false,
                }),
                platform_code: 0,
                position_code: 0,
                usb_hid: 0,
                #[cfg(target_os = "macos")]
                extra_data: 0,
            };
            assert!(is_text_event(&event));
            if let Some(message) = input.decode(&event).unwrap() {
                received.push_str(message.seq());
            }
            event.event_type = EventType::KeyRelease(Key::Unknown(0));
            event.unicode = None;
            assert!(input.decode(&event).unwrap().is_none());
            event.event_type = EventType::KeyPress(Key::KeyA);
            assert!(!is_text_event(&event));
        }
        assert_eq!(received, expected);
    }
}
