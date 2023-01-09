use enigo::{Enigo, MouseButton, MouseControllable};
use std::thread;
use std::time::Duration;

fn main() {
    let wait_time = Duration::from_secs(2);
    let mut enigo = Enigo::new();

    thread::sleep(wait_time);

    enigo.mouse_move_to(500, 200);
    thread::sleep(wait_time);

    enigo.mouse_down(MouseButton::Left).ok();
    thread::sleep(wait_time);

    enigo.mouse_move_relative(100, 100);
    thread::sleep(wait_time);

    enigo.mouse_up(MouseButton::Left);
    thread::sleep(wait_time);

    enigo.mouse_click(MouseButton::Left);
    thread::sleep(wait_time);

    #[cfg(not(target_os = "macos"))]
    {
        enigo.mouse_scroll_x(2);
        thread::sleep(wait_time);

        enigo.mouse_scroll_x(-2);
        thread::sleep(wait_time);

        enigo.mouse_scroll_y(2);
        thread::sleep(wait_time);

        enigo.mouse_scroll_y(-2);
        thread::sleep(wait_time);
    }
}
