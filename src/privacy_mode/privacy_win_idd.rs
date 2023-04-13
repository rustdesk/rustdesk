use super::{PrivacyModeState, NO_DISPLAYS};
use hbb_common::{allow_err, bail, lazy_static, log, ResultType};
use scrap::dxgi;
use winapi::{
    shared::{
        minwindef::{DWORD, FALSE},
        ntdef::{NULL, WCHAR},
    },
    um::{
        errhandlingapi::GetLastError,
        wingdi::{DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DM_POSITION},
        winuser::{
            ChangeDisplaySettingsExW, EnumDisplayDevicesW, EnumDisplaySettingsW, CDS_NORESET,
            CDS_SET_PRIMARY, CDS_UPDATEREGISTRY, DISP_CHANGE_SUCCESSFUL,
            EDD_GET_DEVICE_INTERFACE_NAME, ENUM_CURRENT_SETTINGS,
        },
    },
};

const IDD_DEVICE_STRING: &'static str = "RustDeskIddDriver Device";

struct Display {
    dm: DEVMODEW,
    name: [WCHAR; 32],
    primary: bool,
}

pub struct PrivacyModeImpl {
    conn_id: i32,
    displays: Vec<Display>,
    virtual_displays: Vec<Display>,
    virtual_displays_added: Option<u32>,
}

impl PrivacyModeImpl {
    fn set_displays(&mut self) {
        self.displays.clear();
        self.virtual_displays.clear();
        for display in dxgi::Displays::get_from_gdi().into_iter() {
            if let Some(gdi_info) = display.gdi() {
                if let Ok(s) = std::string::String::from_utf16(&gdi_info.dd.DeviceString) {
                    if s == IDD_DEVICE_STRING {
                        self.virtual_displays.push(Display {
                            dm: gdi_info.dm,
                            name: gdi_info.dd.DeviceName,
                            primary: gdi_info.is_primary,
                        });
                        continue;
                    }
                }
                self.displays.push(Display {
                    dm: gdi_info.dm,
                    name: gdi_info.dd.DeviceName,
                    primary: gdi_info.is_primary,
                });
            }
        }
    }

    fn restore(&self) {}

    fn set_primary_display(&mut self) -> ResultType<()> {
        self.ensure_virtual_display()?;
        if self.virtual_displays.is_empty() {
            bail!("No virtual displays");
        }
        let display = &self.virtual_displays[0];

        let mut new_primary_dm: DEVMODEW = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        new_primary_dm.dmSize = std::mem::size_of::<DEVMODEW>() as _;
        new_primary_dm.dmDriverExtra = 0;
        unsafe {
            if FALSE
                == EnumDisplaySettingsW(
                    display.name.as_ptr(),
                    ENUM_CURRENT_SETTINGS,
                    &mut new_primary_dm,
                )
            {
                bail!(
                    "Failed EnumDisplaySettingsW, device name: {:?}, error code: {}",
                    std::string::String::from_utf16(&display.name),
                    GetLastError()
                );
            }

            let mut i: DWORD = 0;
            loop {
                let mut flags = CDS_UPDATEREGISTRY | CDS_NORESET;
                let mut dd: DISPLAY_DEVICEW = std::mem::MaybeUninit::uninit().assume_init();
                dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as _;
                if FALSE
                    == EnumDisplayDevicesW(NULL as _, i, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME)
                {
                    break;
                }
                i += 1;
                if (dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) == 0 {
                    continue;
                }

                if dd.DeviceName == display.name {
                    flags |= CDS_SET_PRIMARY;
                }

                let mut dm: DEVMODEW = std::mem::MaybeUninit::uninit().assume_init();
                dm.dmSize = std::mem::size_of::<DEVMODEW>() as _;
                dm.dmDriverExtra = 0;
                if FALSE
                    == EnumDisplaySettingsW(dd.DeviceName.as_ptr(), ENUM_CURRENT_SETTINGS, &mut dm)
                {
                    bail!(
                        "Failed EnumDisplaySettingsW, device name: {:?}, error code: {}",
                        std::string::String::from_utf16(&dd.DeviceName),
                        GetLastError()
                    );
                }

                dm.u1.s2_mut().dmPosition.x -= new_primary_dm.u1.s2().dmPosition.x;
                dm.u1.s2_mut().dmPosition.y -= new_primary_dm.u1.s2().dmPosition.y;
                dm.dmFields |= DM_POSITION;
                let rc = ChangeDisplaySettingsExW(
                    dd.DeviceName.as_ptr(),
                    &mut dm,
                    NULL as _,
                    flags,
                    NULL,
                );

                if rc != DISP_CHANGE_SUCCESSFUL {
                    log::error!(
                        "Failed ChangeDisplaySettingsEx, device name: {:?}, flags: {}, ret: {}",
                        std::string::String::from_utf16(&dd.DeviceName),
                        flags,
                        rc
                    );
                    bail!("Failed ChangeDisplaySettingsEx, ret: {}", rc);
                }
            }
        }

        Ok(())
    }

    fn ensure_virtual_display(&mut self) -> ResultType<()> {
        if self.virtual_displays.is_empty() {
            virtual_display::create_device()?;
        }
        if virtual_display::is_device_created() {
            // to-do: add monitor index here
            virtual_display::plug_in_monitor()?;
            self.virtual_displays_added = Some(0);
        }

        self.set_displays();
        Ok(())
    }
}

impl super::PrivacyMode for PrivacyModeImpl {
    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if self.check_on_conn_id(conn_id)? {
            return Ok(true);
        }
        self.set_displays();
        if self.displays.is_empty() {
            bail!(NO_DISPLAYS);
        }
        self.set_primary_display()?;

        bail!("unimplemented")
    }

    fn turn_off_privacy(
        &mut self,
        conn_id: i32,
        state: Option<PrivacyModeState>,
    ) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        self.restore();
        bail!("unimplemented")
    }

    #[inline]
    fn cur_conn_id(&self) -> i32 {
        self.conn_id
    }
}
