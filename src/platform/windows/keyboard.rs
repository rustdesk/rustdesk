use base::message_proto::KeyEvent;
use rdev::Event;
use winapi::um::winuser::VK_PACKET;

const HIGH_SURROGATES: std::ops::RangeInclusive<u16> = 0xD800..=0xDBFF;

pub(crate) fn is_unicode_packet(event: &Event) -> bool {
    event.platform_code == VK_PACKET as u32
}

#[derive(Default)]
pub(crate) struct UnicodePacket {
    high_surrogate: Option<u16>,
}

impl UnicodePacket {
    pub(crate) fn decode(&mut self, code: u32) -> Result<Option<KeyEvent>, &'static str> {
        let previous = self.high_surrogate.take();
        let code = u16::try_from(code).map_err(|_| "Invalid Unicode keyboard packet")?;
        if HIGH_SURROGATES.contains(&code) {
            self.high_surrogate = Some(code);
            return if previous.is_some() {
                Err("Unpaired Unicode keyboard surrogate")
            } else {
                Ok(None)
            };
        }

        let units = match previous {
            Some(high) => vec![high, code],
            None => vec![code],
        };
        let text = String::from_utf16(&units).map_err(|_| "Invalid Unicode keyboard sequence")?;
        let mut event = KeyEvent::new();
        // Legacy text input already injects Unicode on the peer, including the sign-in screen.
        event.set_seq(text);
        Ok(Some(event))
    }
}
