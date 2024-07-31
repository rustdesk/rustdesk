use dbus::blocking::Connection;
use dbus::arg;
use hbb_common::log;
use std::env::var;
use std::time::Duration;

const XDG_CURRENT_DESKTOP: &'static str = "XDG_CURRENT_DESKTOP";
const XDG_SESSION_TYPE: &'static str = "XDG_SESSION_TYPE";

fn is_gnome_wayland() -> bool {
    let e1 = var(XDG_CURRENT_DESKTOP).map(|e| e == "GNOME").unwrap_or(false);
    let e2 = var(XDG_SESSION_TYPE).map(|e| e == "wayland").unwrap_or(false);
    return e1 && e2;
}

type Res = (u32,
            Vec<(u32, i32, i32, i32, i32, i32, u32, Vec<u32>, arg::PropMap)>,
            Vec<(u32, i32, i32, Vec<u32>, String, Vec<u32>, Vec<u32>, arg::PropMap)>,
            Vec<(u32, i32, u32, u32, f64, u32)>,
            i32,
            i32);

// return (width, height) 
// Reference https://wiki.gnome.org/Initiatives/Wayland/Gaps/DisplayConfig
pub fn gnome_wayland_get_resolution() -> Option<(i32, i32)> {
    if !is_gnome_wayland() {
        return None;
    }
    let conn = Connection::new_session();
    if let Ok(conn) = conn {
        // Open a proxy to the Mutter DisplayConfig
        let proxy = conn.with_proxy(
            "org.gnome.Mutter.DisplayConfig",
            "/org/gnome/Mutter/DisplayConfig",
            Duration::from_millis(5000),
        );

        let res: Result<Res, _> = proxy.method_call("org.gnome.Mutter.DisplayConfig", "GetResources", ("max_screen_width", "max_screen_height"));
        if let Ok(res) = res {
            Some((res.4, res.5))
        } else {
            None
        }
    } else {
        None
    }
}
