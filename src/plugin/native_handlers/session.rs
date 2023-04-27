use super::PluginNativeHandler;


/// Session related handler for librustdesk core.
pub struct PluginNativeSessionHandler;


impl PluginNativeHandler for PluginNativeSessionHandler {
    fn on_message(method: &String, data: &serde_json::Map<String, serde_json::Value>) -> Option<super::NR> {
        None
    }

    fn on_message_raw(method: &String, data: &serde_json::Map<String, serde_json::Value>, raw: *const std::ffi::c_void, raw_len: usize) -> Option<super::NR> {
        None
    }

}

impl PluginNativeSessionHandler {
    fn create_session()  {

    }


    fn add_session_hook() {
        
    }
}