// Farooq Remote branding layer.
//
// Farooq Remote is a build of RustDesk (AGPL-3.0, https://github.com/rustdesk/rustdesk)
// developed by Mohammad Farooq — https://www.farooqmusic.com
// Downloads and self-hosting guides: https://www.mymandoob.com/farooqremote
//
// This file is the single place where the product is renamed and pointed at the
// Farooq Remote ID/relay server. Everything else in the code base stays upstream.
//
// How it works (all upstream mechanisms, nothing patched elsewhere):
//   * APP_NAME != "RustDesk" switches the app into upstream's "custom client" mode:
//     every translated string that says "RustDesk" shows the new name, the window
//     title / service name / data folders use it, and the upstream update-checker
//     is turned off.
//   * The ID server, relay and public key are written to DEFAULT_SETTINGS, i.e. the
//     *default* values of Settings → Network. A user can still overwrite them there
//     (that is the "use your own server" mode); clearing the field falls back to
//     these defaults again.

use hbb_common::{
    config::{keys, APP_NAME, DEFAULT_SETTINGS},
    log,
};
use std::sync::Once;

/// Product name. No spaces: it is also used for the Windows service name,
/// data folders and the tray tooltip.
pub const APP_DISPLAY_NAME: &str = "FarooqRemote";

/// Farooq Remote public ID server (hbbs). Hostname remote.farooqmusic.com will
/// replace the raw IP once DNS is set; keep both in sync with the server kit.
pub const ID_SERVER: &str = "145.241.158.114";
/// Relay server (hbbr).
pub const RELAY_SERVER: &str = "145.241.158.114:21117";
/// Public key of the Farooq Remote ID server (data/id_ed25519.pub on the VM).
pub const SERVER_PUBLIC_KEY: &str = "11rU8HpBVbadu59plPSdMpFvkaJ2amnfcB+rJlZ2EWs=";

pub const WEBSITE_URL: &str = "https://www.farooqmusic.com";
pub const DOWNLOAD_URL: &str = "https://www.mymandoob.com/farooqremote";
pub const PRIVACY_URL: &str = "https://www.mymandoob.com/farooqremote/privacy";
pub const SOURCE_URL: &str = "https://github.com/farooqmusicai/FarooqRemote-Client";

static APPLIED: Once = Once::new();

/// Apply the branding. Safe to call more than once; runs only the first time.
pub fn apply() {
    APPLIED.call_once(|| {
        *APP_NAME.write().unwrap() = APP_DISPLAY_NAME.to_owned();
        let mut defaults = DEFAULT_SETTINGS.write().unwrap();
        defaults.insert(
            keys::OPTION_CUSTOM_RENDEZVOUS_SERVER.to_owned(),
            ID_SERVER.to_owned(),
        );
        defaults.insert(
            keys::OPTION_RELAY_SERVER.to_owned(),
            RELAY_SERVER.to_owned(),
        );
        defaults.insert(keys::OPTION_KEY.to_owned(), SERVER_PUBLIC_KEY.to_owned());
        log::info!(
            "{} branding applied: id server {}, relay {}",
            APP_DISPLAY_NAME,
            ID_SERVER,
            RELAY_SERVER
        );
    });
}
