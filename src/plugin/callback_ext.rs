// External support for callback.
// 1. Support block input for some plugins.
// -----------------------------------------------------------------------------

use super::*;
use std::{ffi::c_void, ptr::null, str::FromStr};

const EXT_SUPPORT_BLOCK_INPUT: &str = "block-input";

pub(super) fn ext_support_callback(
    id: &str,
    peer: &str,
    msg: &super::callback_msg::MsgToExtSupport,
) -> *const c_void {
    match &msg.r#type as _ {
        EXT_SUPPORT_BLOCK_INPUT => {
            // let supported_plugins = [];
            // let supported = supported_plugins.contains(&id);
            let supported = true;
            if supported {
                match bool::from_str(&msg.data) {
                    Ok(block) => {
                        if crate::server::plugin_block_input(peer, block) == block {
                            null()
                        } else {
                            make_return_code_msg(
                                errno::ERR_CALLBACK_FAILED,
                                "Failed to block input",
                            )
                        }
                    }
                    Err(err) => make_return_code_msg(
                        errno::ERR_CALLBACK_INVALID_ARGS,
                        &format!("Failed to parse data: {}", err),
                    ),
                }
            } else {
                make_return_code_msg(
                    errno::ERR_CALLBACK_PLUGIN_ID,
                    &format!("This operation is not supported for plugin '{}', please contact the RustDesk team for support.", id),
                )
            }
        }
        _ => make_return_code_msg(
            errno::ERR_CALLBACK_TARGET_TYPE,
            &format!("Unknown target type '{}'", &msg.r#type),
        ),
    }
}
