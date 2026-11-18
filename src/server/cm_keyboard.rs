use crate::ipc::Data;
use base::message_proto::{key_event, ControlKey, KeyEvent};
use hbb_common::{tokio::sync::mpsc::UnboundedSender, ResultType};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(super) struct CmKeyboardGate {
    blocked: AtomicBool,
}

impl CmKeyboardGate {
    pub(super) fn on_cm_ipc_disconnect(&self, tx: &UnboundedSender<Data>) -> ResultType<()> {
        // Block before notifying the connection loop, which may await network I/O.
        self.set_blocked(true);
        tx.send(Data::CmIpcDisconnected)?;
        Ok(())
    }

    pub(super) fn set_blocked(&self, blocked: bool) {
        self.blocked.store(blocked, Ordering::SeqCst);
    }

    pub(super) fn allows(&self, event: &KeyEvent) -> bool {
        if !self.blocked.load(Ordering::SeqCst) {
            return true;
        }
        if event.down || event.press {
            return false;
        }
        // Release keys held before CM gained focus. Text and function commands
        // can inject input even with down=false, so they are not key releases.
        match &event.union {
            Some(key_event::Union::Chr(_) | key_event::Union::Win2winHotkey(_)) => true,
            Some(key_event::Union::ControlKey(key)) => !matches!(
                key.enum_value(),
                Ok(ControlKey::CtrlAltDel | ControlKey::LockScreen)
            ),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::KeyboardMode;
    use hbb_common::tokio::sync::mpsc;

    #[test]
    fn focused_cm_blocks_queued_keys_and_text_until_blur() {
        let gate = CmKeyboardGate::default();
        let mut key = KeyEvent::new();
        key.down = true;
        key.set_chr(u32::from('a'));
        assert!(gate.allows(&key));

        gate.set_blocked(true);
        assert!(!gate.allows(&key));
        key.down = false;
        key.set_unicode(u32::from('界'));
        assert!(!gate.allows(&key));
        key.set_seq("remote text".to_owned());
        assert!(!gate.allows(&key));

        gate.set_blocked(false);
        assert!(gate.allows(&key));
    }

    #[test]
    fn blocked_cm_allows_key_releases_but_not_commands() {
        let gate = CmKeyboardGate::default();
        gate.set_blocked(true);
        let mut key = KeyEvent::new();
        key.set_control_key(ControlKey::Control);
        assert!(gate.allows(&key));
        key.set_chr(u32::from('a'));
        assert!(gate.allows(&key));
        key.press = true;
        assert!(!gate.allows(&key));
        key.press = false;
        key.mode = KeyboardMode::Translate.into();
        const VK_LCONTROL: u32 = 0xa2;
        key.set_win2win_hotkey(VK_LCONTROL << u16::BITS);
        assert!(gate.allows(&key));
        key.down = true;
        assert!(!gate.allows(&key));
        key.down = false;
        key.press = true;
        assert!(!gate.allows(&key));
        key.press = false;
        key.set_control_key(ControlKey::CtrlAltDel);
        assert!(!gate.allows(&key));
        key.set_control_key(ControlKey::LockScreen);
        assert!(!gate.allows(&key));
    }

    #[test]
    fn cm_ipc_disconnect_blocks_before_connection_receiver_runs() {
        for blocked in [false, true] {
            let gate = CmKeyboardGate::default();
            gate.set_blocked(blocked);
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut key = KeyEvent::new();
            key.set_seq("queued text".to_owned());

            gate.on_cm_ipc_disconnect(&tx).unwrap();
            assert!(!gate.allows(&key));
            assert!(matches!(rx.try_recv().unwrap(), Data::CmIpcDisconnected));
        }
    }
}
