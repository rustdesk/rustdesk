use gdk4;
use gtk4;
use gdk4::prelude::{MonitorExt, DisplayExt, ListModelExt, Cast};

fn main() {
    let _ = gtk4::init();
    let display = gdk4::Display::default().unwrap();
    let monitor = display.monitors().item(0).unwrap().downcast::<gdk4::Monitor>().unwrap();
    let scale = monitor.scale();
    print!("{}", scale);
}
