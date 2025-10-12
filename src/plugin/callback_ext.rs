// External support for callback.
// 1. Support block input for some plugins.
// -----------------------------------------------------------------------------

use super::*;

const EXT_SUPPORT_BLOCK_INPUT: &str = "block-input";

pub(super) fn ext_support_callback(
    id: &str,
    peer: &str,
    msg: &super::callback_msg::MsgToExtSupport,
) -> PluginReturn {
    match &msg.r#type as _ {
        EXT_SUPPORT_BLOCK_INPUT => {
            // let supported_plugins = [];
            // let supported = supported_plugins.contains(&id);
            let supported = true;
            if supported {
                if msg.data.len() != 1 {
                    return PluginReturn::new(
                        errno::ERR_CALLBACK_INVALID_ARGS,
                        "Invalid data length",
                    );
                }
                let block = msg.data[0] != 0;
                if crate::server::plugin_block_input(peer, block) == block {
                    PluginReturn::success()
                } else {
                    PluginReturn::new(errno::ERR_CALLBACK_FAILED, "")
                }
            } else {
                PluginReturn::new(
                    errno::ERR_CALLBACK_PLUGIN_ID,
                    &format!("This operation is not supported for plugin '{}', please contact the RustDesk team for support.", id),
                )
            }
        }
        _ => PluginReturn::new(
            errno::ERR_CALLBACK_TARGET_TYPE,
            &format!("Unknown target type '{}'", &msg.r#type),
        ),
    }
}
