pub(super) fn is_x11() -> bool {
    // XWayland windows can use rdev even when the desktop session is Wayland.
    unsafe {
        let display = gtk::gdk::ffi::gdk_display_get_default();
        let x11_type = gtk::glib::gobject_ffi::g_type_from_name(b"GdkX11Display\0".as_ptr().cast());
        !display.is_null()
            && x11_type != 0
            && gtk::glib::gobject_ffi::g_type_check_instance_is_a(display.cast(), x11_type) != 0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Requires a Wayland desktop with an X11 GTK display"]
    fn xwayland_offers_native_keyboard_input() {
        gtk::init().unwrap();
        assert!(!crate::platform::linux::is_x11());
        let supported = super::super::get_supported_input_source();
        assert!(supported
            .iter()
            .any(|(source, _)| source == super::super::CONFIG_INPUT_SOURCE_1));
    }

    #[test]
    #[ignore = "Requires a native Wayland GTK display"]
    fn native_wayland_keeps_flutter_input() {
        gtk::init().unwrap();
        assert!(!super::is_x11());
        let supported = super::super::get_supported_input_source();
        assert_eq!(supported.len(), 1);
        assert_eq!(supported[0].0, super::super::CONFIG_INPUT_SOURCE_2);
    }
}
