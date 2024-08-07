#[cfg(target_os = "linux")]
use gdk4;
#[cfg(target_os = "linux")]
use gtk4;
#[cfg(target_os = "linux")]
use gdk4::prelude::{MonitorExt, DisplayExt, ListModelExt, Cast};

fn main() {
    #[cfg(target_os = "linux")]
    {
        let _ = gtk4::init();
        let display = gdk4::Display::default().unwrap();
        let monitor = display.monitors().item(0).unwrap().downcast::<gdk4::Monitor>().unwrap();
        let scale = monitor.scale();
        print!("{}", scale);
    }
}
