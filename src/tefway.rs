//! Tefway Remote build profile and distribution configuration.
//!
//! Both products are built from this one tree. `TEFWAY_PROFILE` is read by
//! `build.rs` and compiled in, so no preference, command-line argument or FFI
//! call can select or change the profile at runtime. An unset profile keeps
//! stock RustDesk behaviour for ordinary development builds.

use base::config::keys;

/// Tefway rendezvous (ID) server. The default rendezvous port 21116 is used,
/// so the value is a bare host.
pub const RENDEZVOUS_SERVER: &str = "remote.tefway.com.br";
/// Tefway relay server. The default relay port 21117 is used.
pub const RELAY_SERVER: &str = "remote.tefway.com.br";
/// Public key of the Tefway rendezvous/relay server. Not a secret: the same key
/// is handed to every client.
pub const PUBLIC_KEY: &str = "3KBIfqldtiCmJkAl2O6rammBCLK4iRiTH3vQdRXoEEs=";

pub const MASTER_NAME: &str = "Tefway Remote Master";
pub const CLIENT_NAME: &str = "Tefway Remote Client";

pub const MASTER_LOGIN_REFUSED: &str = "Tefway Remote Master does not accept incoming connections";

/// Builtin option key the Flutter UI reads to know which product it runs in.
/// Builtin options are not user preferences, so this cannot be rewritten.
pub const PROFILE_OPTION: &str = "tefway-profile";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// No profile asked for: stock RustDesk.
    Standard,
    Master,
    Client,
}

fn parse(value: Option<&str>) -> Profile {
    match value {
        Some("master") => Profile::Master,
        Some("client") => Profile::Client,
        _ => Profile::Standard,
    }
}

pub fn profile() -> Profile {
    parse(option_env!("TEFWAY_PROFILE"))
}

#[inline]
pub fn is_master() -> bool {
    profile() == Profile::Master
}

#[inline]
pub fn is_client() -> bool {
    profile() == Profile::Client
}

/// Product name of the running build, empty for a stock build.
pub fn product_name() -> &'static str {
    match profile() {
        Profile::Master => MASTER_NAME,
        Profile::Client => CLIENT_NAME,
        Profile::Standard => "",
    }
}

fn profile_id() -> &'static str {
    match profile() {
        Profile::Master => "master",
        Profile::Client => "client",
        Profile::Standard => "",
    }
}

/// Incoming-only (no outgoing connections) for the Client product, plus
/// whatever the hard settings of a stock deployment asked for.
pub fn is_incoming_only() -> bool {
    is_client() || hbb_common::config::is_incoming_only()
}

/// The Master product always refuses sessions offered to it.
#[inline]
pub fn refuses_incoming() -> bool {
    is_master()
}

/// The Client product requires a fresh local click for every connection.
#[inline]
pub fn requires_manual_approval() -> bool {
    is_client()
}

/// Server-side options the build forces. Values live in `OVERWRITE_SETTINGS`,
/// which outranks the user's config file, so they cannot be edited from the
/// settings UI or a preferences file, and `set_option` cannot store a
/// replacement.
pub fn server_options() -> Vec<(&'static str, &'static str)> {
    if profile() == Profile::Standard {
        return Vec::new();
    }
    let mut options = vec![
        (keys::OPTION_CUSTOM_RENDEZVOUS_SERVER, RENDEZVOUS_SERVER),
        (keys::OPTION_RELAY_SERVER, RELAY_SERVER),
        (keys::OPTION_KEY, PUBLIC_KEY),
        // Blank: this deployment has no RustDesk API server, and an empty
        // overwrite value also stops a stored one from being used.
        (keys::OPTION_API_SERVER, ""),
    ];
    if is_client() {
        // Consent is a click on the screen the session is offered to. A
        // permanent password must never stand in for it, and the one-time
        // password path is the one that leads to that dialog.
        options.push((keys::OPTION_APPROVE_MODE, "click"));
        options.push((keys::OPTION_VERIFICATION_METHOD, "use-temporary-password"));
        options.push((keys::OPTION_ENABLE_TRUSTED_DEVICES, "N"));
    }
    options
}

/// Builtin (non-preference) options the build forces.
pub fn builtin_options() -> Vec<(&'static str, &'static str)> {
    let mut options = vec![(PROFILE_OPTION, profile_id())];
    if is_client() {
        options.push((keys::OPTION_DISABLE_CHANGE_PERMANENT_PASSWORD, "Y"));
        options.push((keys::OPTION_ALLOW_LOGON_SCREEN_PASSWORD, "N"));
    }
    options
}

/// Install the profile's fixed configuration into this process.
///
/// Called once per process, after `load_custom_client()` so that a bundled
/// custom client cannot replace the Tefway server or re-enable unattended
/// access.
pub fn apply() {
    if profile() == Profile::Standard {
        return;
    }
    if let Ok(mut name) = hbb_common::config::APP_NAME.write() {
        *name = product_name().to_owned();
    }
    if let Ok(mut settings) = hbb_common::config::OVERWRITE_SETTINGS.write() {
        for (key, value) in server_options() {
            settings.insert(key.to_owned(), value.to_owned());
        }
    }
    if let Ok(mut builtin) = hbb_common::config::BUILTIN_SETTINGS.write() {
        for (key, value) in builtin_options() {
            builtin.insert(key.to_owned(), value.to_owned());
        }
    }
    log::info!(
        "Tefway Remote profile: {}, rendezvous: {}",
        product_name(),
        RENDEZVOUS_SERVER
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_SERVER_KEY: &str = "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=";
    const PUBLIC_SERVER: &str = "rs-ny.rustdesk.com";

    fn option_value<'a>(options: &'a [(&'static str, &'static str)], key: &str) -> Option<&'a str> {
        options.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    #[test]
    fn only_the_two_tefway_profiles_select_a_product() {
        assert_eq!(parse(Some("master")), Profile::Master);
        assert_eq!(parse(Some("client")), Profile::Client);
        assert_eq!(parse(None), Profile::Standard);
        assert_eq!(parse(Some("")), Profile::Standard);
        assert_eq!(parse(Some("Master")), Profile::Standard);
        assert_eq!(parse(Some("master,client")), Profile::Standard);
    }

    #[test]
    fn distribution_never_falls_back_to_public_servers() {
        for options in [server_options(), Vec::new()] {
            if let Some(host) = option_value(&options, keys::OPTION_CUSTOM_RENDEZVOUS_SERVER) {
                assert_ne!(host, PUBLIC_SERVER);
                assert_eq!(host, RENDEZVOUS_SERVER);
            }
        }
        assert_ne!(PUBLIC_KEY, PUBLIC_SERVER_KEY);
        assert_eq!(PUBLIC_KEY.len(), 44);
    }

    #[test]
    fn stock_build_forces_nothing() {
        if profile() == Profile::Standard {
            assert!(server_options().is_empty());
            assert!(!refuses_incoming());
            assert!(!requires_manual_approval());
            assert!(!is_client());
        }
    }

    #[test]
    fn client_requires_a_click_and_blocks_unattended_access() {
        if !is_client() {
            return;
        }
        let server = server_options();
        assert_eq!(
            option_value(&server, keys::OPTION_APPROVE_MODE),
            Some("click")
        );
        assert_eq!(
            option_value(&server, keys::OPTION_VERIFICATION_METHOD),
            Some("use-temporary-password")
        );
        assert_eq!(
            option_value(&server, keys::OPTION_ENABLE_TRUSTED_DEVICES),
            Some("N")
        );
        let builtin = builtin_options();
        assert_eq!(
            option_value(&builtin, keys::OPTION_DISABLE_CHANGE_PERMANENT_PASSWORD),
            Some("Y")
        );
        assert_eq!(
            option_value(&builtin, keys::OPTION_ALLOW_LOGON_SCREEN_PASSWORD),
            Some("N")
        );
        assert!(requires_manual_approval());
        assert!(is_incoming_only());
        assert!(!refuses_incoming());
    }

    #[test]
    fn master_accepts_outgoing_and_refuses_incoming() {
        if !is_master() {
            return;
        }
        assert!(refuses_incoming());
        assert!(!requires_manual_approval());
        assert!(!is_client());
        assert_eq!(
            option_value(&server_options(), keys::OPTION_APPROVE_MODE),
            None
        );
    }
}
