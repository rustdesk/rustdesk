use super::*;

pub mod audio_service {
    use super::*;
    pub const NAME: &'static str = "audio";
    pub fn new() -> GenericService {
        let sp = GenericService::new(NAME, true);
        sp
    }
    pub fn restart() {}
}

pub mod video_service {
    use super::*;
    pub const NAME: &'static str = "video";
    pub fn new() -> GenericService {
        let sp = GenericService::new(NAME, true);
        sp
    }
    pub fn is_privacy_mode_supported() -> bool {
        false
    }
    pub fn test_create_capturer(privacy_mode_id: i32, timeout_millis: u64) -> bool {
        false
    }
    pub fn refresh() {}
    pub async fn switch_display(i: i32) {}
    pub async fn get_displays() -> ResultType<(usize, Vec<DisplayInfo>)> {
        bail!("No displayes");
    }
    pub fn is_inited_msg() -> Option<Message> {
        None
    }
    pub fn capture_cursor_embeded() -> bool {
        false
    }
    pub async fn switch_to_primary() {}
    pub fn set_privacy_mode_conn_id(_: i32) {}
    pub fn get_privacy_mode_conn_id() -> i32 {
        0
    }
    pub fn notify_video_frame_feched(_: i32, _: Option<std::time::Instant>) {}
    lazy_static::lazy_static! {
        pub static ref VIDEO_QOS: Arc<Mutex<video_qos::VideoQoS>> = Default::default();
    }
    pub const SCRAP_X11_REQUIRED: &str = "";
    pub const SCRAP_X11_REF_URL: &str = "";
}
