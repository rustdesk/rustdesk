use hbb_common::{bail, ResultType};
use std::sync::atomic::{AtomicBool, Ordering};

pub use base::config::keys::OPTION_ALLOW_FORWARDED_MICROPHONE as ALLOW;
pub const TOGGLE: &str = "forward-microphone";
static IN_USE: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod platform {
    use hbb_common::{bail, ResultType};
    pub struct Route;
    impl Route {
        pub fn open() -> ResultType<Self> {
            bail!("This platform cannot receive a virtual microphone")
        }
        pub fn output(&self) -> &str {
            ""
        }
        pub fn restore(&mut self) {}
    }
}

pub struct RouteLease(platform::Route);
impl RouteLease {
    pub fn open() -> ResultType<Self> {
        if IN_USE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("Another connection is using the remote microphone");
        }
        match platform::Route::open() {
            Ok(route) => Ok(Self(route)),
            Err(error) => {
                IN_USE.store(false, Ordering::Release);
                Err(error)
            }
        }
    }
    pub fn output(&self) -> &str {
        self.0.output()
    }
}
impl Drop for RouteLease {
    fn drop(&mut self) {
        // The route is restored before another connection can acquire the lease.
        self.0.restore();
        IN_USE.store(false, Ordering::Release);
    }
}

pub fn message(
    request_id: i64,
    enabled: bool,
    response: bool,
    error: &str,
) -> base::message_proto::Message {
    use base::message_proto::{Message, MicrophoneForwarding, Misc};
    let mut misc = Misc::new();
    misc.set_microphone_forwarding(MicrophoneForwarding {
        request_id,
        enabled,
        response,
        error: error.to_owned(),
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_misc(misc);
    message
}

static CAPTURE_IN_USE: AtomicBool = AtomicBool::new(false);
pub fn capture_in_use() -> bool {
    CAPTURE_IN_USE.load(Ordering::Acquire)
}
pub struct CaptureLease;
impl CaptureLease {
    pub fn acquire() -> Option<Self> {
        CAPTURE_IN_USE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}
impl Drop for CaptureLease {
    fn drop(&mut self) {
        CAPTURE_IN_USE.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::{message, misc, Message};
    use hbb_common::protobuf::Message as ProtobufMessage;

    #[test]
    fn capture_lease_excludes_another_session_and_releases_on_drop() {
        let lease = CaptureLease::acquire().unwrap();
        assert!(capture_in_use());
        assert!(CaptureLease::acquire().is_none());
        drop(lease);
        let replacement = CaptureLease::acquire().unwrap();
        drop(replacement);
        assert!(!capture_in_use());
    }

    #[test]
    fn forwarding_response_preserves_session_and_error_without_legacy_audio() {
        let wire = message(123, false, true, "Permission revoked")
            .write_to_bytes()
            .unwrap();
        let decoded = Message::parse_from_bytes(&wire).unwrap();
        match decoded.union {
            Some(message::Union::Misc(value)) => match value.union {
                Some(misc::Union::MicrophoneForwarding(response)) => {
                    assert_eq!(response.request_id, 123);
                    assert!(!response.enabled);
                    assert!(response.response);
                    assert!(!response.ready);
                    assert_eq!(response.error, "Permission revoked");
                }
                _ => panic!("Expected isolated forwarding control"),
            },
            _ => panic!("Forwarding control must not be a legacy audio frame"),
        }
    }
}
