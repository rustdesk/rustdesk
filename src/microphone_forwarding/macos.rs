use hbb_common::{bail, ResultType};
extern "C" {
    fn rd_mic_open(previous: *mut u32, target: *mut u32) -> i32;
    fn rd_mic_restore(previous: u32, target: u32) -> i32;
}
pub struct Route {
    previous: u32,
    target: u32,
}
impl Route {
    pub fn open() -> ResultType<Self> {
        let mut route = Self {
            previous: 0,
            target: 0,
        };
        let result = unsafe { rd_mic_open(&mut route.previous, &mut route.target) };
        if result != 0 {
            bail!("Install BlackHole 2ch and allow audio access (CoreAudio error {result})");
        }
        Ok(route)
    }
    pub fn output(&self) -> &str {
        "BlackHole 2ch"
    }
    pub fn restore(&mut self) {
        if self.target != 0 {
            let result = unsafe { rd_mic_restore(self.previous, self.target) };
            if result != 0 {
                hbb_common::log::warn!("Unable to restore microphone: {result}");
            }
            self.target = 0;
        }
    }
}
