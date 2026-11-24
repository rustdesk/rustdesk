use std::{io, mem, sync::OnceLock};

use base::message_proto::{native_touch_event::Phase, NativeTouchEvent};
use hbb_common::log;
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress};
use windows::{
    core::BOOL,
    Win32::{
        Foundation::POINT,
        Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONULL},
        UI::{
            Controls::{
                HSYNTHETICPOINTERDEVICE, POINTER_FEEDBACK_MODE, POINTER_FEEDBACK_NONE,
                POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
            },
            Input::Pointer::{
                POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE, POINTER_FLAG_UP,
                POINTER_FLAG_UPDATE, POINTER_TOUCH_INFO,
            },
            WindowsAndMessaging::{POINTER_INPUT_TYPE, PT_TOUCH},
        },
    },
};

const MAX_CONTACTS: usize = 10;

type CreateDevice = unsafe extern "system" fn(
    POINTER_INPUT_TYPE,
    u32,
    POINTER_FEEDBACK_MODE,
) -> HSYNTHETICPOINTERDEVICE;
type InjectFrame =
    unsafe extern "system" fn(HSYNTHETICPOINTERDEVICE, *const POINTER_TYPE_INFO, u32) -> BOOL;
type DestroyDevice = unsafe extern "system" fn(HSYNTHETICPOINTERDEVICE);

struct Api {
    create: CreateDevice,
    inject: InjectFrame,
    destroy: DestroyDevice,
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        // Resolve these APIs only on Windows versions that export them.
        let user32 = GetModuleHandleW(windows::core::w!("user32.dll").as_ptr());
        if user32.is_null() {
            return None;
        }
        let create = GetProcAddress(user32, b"CreateSyntheticPointerDevice\0".as_ptr().cast());
        let inject = GetProcAddress(user32, b"InjectSyntheticPointerInput\0".as_ptr().cast());
        let destroy = GetProcAddress(user32, b"DestroySyntheticPointerDevice\0".as_ptr().cast());
        if create.is_null() || inject.is_null() || destroy.is_null() {
            return None;
        }
        Some(Api {
            create: mem::transmute(create),
            inject: mem::transmute(inject),
            destroy: mem::transmute(destroy),
        })
    })
    .as_ref()
}

pub(crate) fn is_supported() -> bool {
    api().is_some()
}

struct Device {
    api: &'static Api,
    handle: HSYNTHETICPOINTERDEVICE,
}

impl Device {
    fn new() -> io::Result<Self> {
        let api = api().ok_or(io::ErrorKind::Unsupported)?;
        let handle = unsafe { (api.create)(PT_TOUCH, MAX_CONTACTS as u32, POINTER_FEEDBACK_NONE) };
        if handle.is_invalid() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { api, handle })
    }

    fn inject(&self, frame: &[POINTER_TYPE_INFO]) -> io::Result<()> {
        if !unsafe { (self.api.inject)(self.handle, frame.as_ptr(), frame.len() as u32) }.as_bool()
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { (self.api.destroy)(self.handle) };
    }
}

#[derive(Clone, Copy)]
struct Contact {
    remote_id: u32,
    point: POINT,
}

#[derive(Default)]
struct Contacts([Option<Contact>; MAX_CONTACTS]);

impl Contacts {
    fn apply(
        &mut self,
        phase: Phase,
        remote_id: u32,
        point: POINT,
    ) -> Option<Vec<POINTER_TYPE_INFO>> {
        let existing = self
            .0
            .iter()
            .position(|contact| contact.is_some_and(|contact| contact.remote_id == remote_id));
        let changed = match (phase, existing) {
            (Phase::DOWN, None) => self.0.iter().position(Option::is_none)?,
            (Phase::MOVE | Phase::UP, Some(index)) => index,
            _ => return None,
        };
        if phase != Phase::UP {
            self.0[changed] = Some(Contact { remote_id, point });
        }
        let frame = self
            .0
            .iter()
            .enumerate()
            .filter_map(|(index, contact)| {
                let contact = contact.as_ref()?;
                let mut touch = POINTER_TOUCH_INFO::default();
                touch.pointerInfo.pointerType = PT_TOUCH;
                touch.pointerInfo.pointerId = index as u32 + 1;
                touch.pointerInfo.ptPixelLocation = contact.point;
                touch.pointerInfo.pointerFlags = match (index == changed, phase) {
                    (true, Phase::DOWN) => {
                        POINTER_FLAG_DOWN | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT
                    }
                    (true, Phase::UP) => POINTER_FLAG_UP,
                    _ => POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
                };
                Some(POINTER_TYPE_INFO {
                    r#type: PT_TOUCH,
                    Anonymous: POINTER_TYPE_INFO_0 { touchInfo: touch },
                })
            })
            .collect();
        if phase == Phase::UP {
            self.0[changed] = None;
        }
        Some(frame)
    }
}

#[derive(Default)]
pub(crate) struct Session {
    device: Option<Device>,
    contacts: Contacts,
}

impl Session {
    pub(crate) fn reset(&mut self) {
        self.device = None;
        self.contacts = Contacts::default();
    }

    pub(crate) fn handle(&mut self, event: NativeTouchEvent) {
        let Ok(phase) = event.phase.enum_value() else {
            return;
        };
        if phase == Phase::CANCEL || super::try_change_desktop() {
            self.reset();
        }
        if matches!(phase, Phase::CANCEL | Phase::UNSPECIFIED)
            || (phase == Phase::DOWN && super::is_prelogin())
        {
            return;
        }
        let point = POINT {
            x: event.x,
            y: event.y,
        };
        if phase != Phase::UP
            && unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONULL) }.is_invalid()
        {
            return;
        }
        let Some(frame) = self.contacts.apply(phase, event.id, point) else {
            return;
        };
        if let Err(err) = self.inject(&frame) {
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(5),
                warn,
                "Native touch injection failed: {}",
                err
            );
            self.reset();
        }
    }

    fn inject(&mut self, frame: &[POINTER_TYPE_INFO]) -> io::Result<()> {
        if self.device.is_none() {
            self.device = Some(Device::new()?);
        }
        if let Some(device) = &self.device {
            device.inject(frame)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: i32) -> POINT {
        POINT { x, y: 10 }
    }

    fn info(contact: &POINTER_TYPE_INFO) -> windows::Win32::UI::Input::Pointer::POINTER_INFO {
        unsafe { contact.Anonymous.touchInfo.pointerInfo }
    }

    #[test]
    fn frames_include_other_active_contacts() {
        let mut contacts = Contacts::default();
        let first = contacts.apply(Phase::DOWN, 100, point(-32)).unwrap();
        let second = contacts.apply(Phase::DOWN, 200, point(40)).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 2);
        let first_id = info(&first[0]).pointerId;
        assert_eq!(info(&second[0]).pointerId, first_id);
        assert_eq!(
            info(&second[0]).pointerFlags & POINTER_FLAG_UPDATE,
            POINTER_FLAG_UPDATE
        );
        let up = contacts.apply(Phase::UP, 100, point(100)).unwrap();
        assert_eq!(up.len(), 2);
        assert_eq!(info(&up[0]).pointerFlags, POINTER_FLAG_UP);
        assert_eq!(info(&up[0]).ptPixelLocation.x, -32);
        let moved = contacts.apply(Phase::MOVE, 200, point(41)).unwrap();
        assert_eq!(moved.len(), 1);
        assert_eq!(info(&moved[0]).pointerId, info(&second[1]).pointerId);
        assert_eq!(info(&moved[0]).ptPixelLocation.x, 41);
    }

    #[test]
    fn ignores_unknown_duplicate_and_excess_contacts() {
        let mut contacts = Contacts::default();
        assert!(contacts.apply(Phase::MOVE, 1, point(0)).is_none());
        assert!(contacts.apply(Phase::UP, 1, point(0)).is_none());
        for id in 1..=MAX_CONTACTS as u32 {
            assert_eq!(
                contacts.apply(Phase::DOWN, id, point(0)).unwrap().len(),
                id as usize
            );
        }
        assert!(contacts.apply(Phase::DOWN, 1, point(0)).is_none());
        assert!(contacts.apply(Phase::DOWN, 20, point(0)).is_none());
        contacts.apply(Phase::UP, 1, point(0)).unwrap();
        assert_eq!(
            contacts.apply(Phase::DOWN, 20, point(0)).unwrap().len(),
            MAX_CONTACTS
        );
    }
}
