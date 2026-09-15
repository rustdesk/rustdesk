use super::super::{get_cursor, get_cursor_data};
use hbb_common::ResultType;
use x11rb::{
    connection::Connection, protocol::xproto::*, rust_connection::RustConnection,
    wrapper::ConnectionExt as _, CURRENT_TIME,
};

struct Settings {
    connection: RustConnection,
    owner: Window,
    property: Atom,
}

impl Settings {
    fn new() -> ResultType<Self> {
        // This test replaces the settings manager; never run on a real desktop.
        assert_eq!(
            std::env::var("RUSTDESK_X11_CURSOR_TEST").as_deref(),
            Ok("1")
        );
        let (connection, screen) = x11rb::connect(None)?;
        let selection = connection
            .intern_atom(false, format!("_XSETTINGS_S{screen}").as_bytes())?
            .reply()?
            .atom;
        let property = connection
            .intern_atom(false, b"_XSETTINGS_SETTINGS")?
            .reply()?
            .atom;
        let owner = connection.generate_id()?;
        connection
            .create_window(
                0,
                owner,
                connection.setup().roots[screen].root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )?
            .check()?;
        connection
            .set_selection_owner(owner, selection, CURRENT_TIME)?
            .check()?;
        Ok(Self {
            connection,
            owner,
            property,
        })
    }

    fn property(&self, bytes: &[u8]) {
        self.connection
            .change_property8(
                PropMode::REPLACE,
                self.owner,
                self.property,
                self.property,
                bytes,
            )
            .unwrap()
            .check()
            .unwrap();
    }

    fn scale(&self, value: i32) {
        const ALIGNMENT: usize = 4;
        let name = b"Gdk/WindowScalingFactor";
        let mut bytes = vec![0, 0, 0, 0];
        bytes.extend(1_u32.to_le_bytes()); // Serial.
        bytes.extend(1_u32.to_le_bytes()); // One integer setting.
        bytes.extend([0, 0]);
        bytes.extend((name.len() as u16).to_le_bytes());
        bytes.extend(name);
        bytes.resize(bytes.len().div_ceil(ALIGNMENT) * ALIGNMENT, 0);
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend(value.to_le_bytes());
        self.property(&bytes);
    }

    fn arrow(&self) {
        const LEFT_PTR: u16 = 68;
        const WHITE: u16 = u16::MAX;
        let font = self.connection.generate_id().unwrap();
        let cursor = self.connection.generate_id().unwrap();
        self.connection
            .open_font(font, b"cursor")
            .unwrap()
            .check()
            .unwrap();
        self.connection
            .create_glyph_cursor(
                cursor,
                font,
                font,
                LEFT_PTR,
                LEFT_PTR + 1,
                0,
                0,
                0,
                WHITE,
                WHITE,
                WHITE,
            )
            .unwrap()
            .check()
            .unwrap();
        self.connection
            .change_window_attributes(
                self.connection.setup().roots[0].root,
                &ChangeWindowAttributesAux::new().cursor(cursor),
            )
            .unwrap()
            .check()
            .unwrap();
    }
}

fn cursor(scale: f64) -> u64 {
    let id = get_cursor().unwrap().expect("Xvfb must have a cursor");
    let data = get_cursor_data(id).unwrap();
    assert_eq!((data.id, data.scale), (id, scale));
    assert!(data.width > 0 && data.height > 0 && !data.colors.is_empty());
    id
}

#[test]
#[ignore = "requires isolated Xvfb and RUSTDESK_FORCED_DISPLAY_SERVER=x11"]
fn x11_metadata_errors_preserve_cursor_delivery_and_recover() {
    let settings = Settings::new().unwrap();
    let unknown = cursor(0.0); // Selection owner exists but has no property.
    settings.scale(2);
    let known = cursor(2.0);
    assert_ne!(known, unknown);
    settings.property(&[0]); // Truncated header.
    assert_eq!(cursor(0.0), unknown);
    settings.scale(0); // Invalid density.
    assert_eq!(cursor(0.0), unknown);
    settings.scale(2);
    assert_eq!(cursor(2.0), known);
    settings.property(&[0]);
    settings.arrow();
    assert_ne!(cursor(0.0), unknown); // New shapes still arrive during failure.
}

#[test]
#[ignore = "requires isolated Xvfb and RUSTDESK_FORCED_DISPLAY_SERVER=x11"]
fn x11_metadata_change_between_id_and_bitmap_keeps_snapshot() {
    let settings = Settings::new().unwrap();
    settings.scale(2);
    let id = get_cursor().unwrap().unwrap();
    settings
        .connection
        .destroy_window(settings.owner)
        .unwrap()
        .check()
        .unwrap();
    let data = get_cursor_data(id).unwrap();
    assert_eq!((data.id, data.scale), (id, 2.0));
    let unknown = get_cursor().unwrap().unwrap();
    assert_ne!(unknown, id);
    let recovered = Settings::new().unwrap();
    recovered.scale(2);
    let data = get_cursor_data(unknown).unwrap();
    assert_eq!((data.id, data.scale), (unknown, 0.0));
    assert_eq!(cursor(2.0), id);
}
