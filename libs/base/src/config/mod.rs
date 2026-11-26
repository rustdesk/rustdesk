pub mod keys;

use hbb_common::config::{Config, Config2};
use std::collections::HashMap;

// A GUI hosting its own IPC can write the same snapshot again after the reply.
pub fn set_ipc_options(options: HashMap<String, String>) {
    Config::set_options_preserving_option(options, keys::OPTION_REG_RECOVERY);
}

pub fn set_ipc_config2(config: Config2) -> bool {
    #[cfg(windows)]
    return Config2::set_preserving_option(config, keys::OPTION_REG_RECOVERY);
    #[cfg(not(windows))]
    Config2::set(config)
}
