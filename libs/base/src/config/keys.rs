//! Option keys shared across the app.
//!
//! The handful that `hbb_common` itself reads stay defined there and are
//! re-exported here, so callers always use this one path.

pub use hbb_common::config::keys::*;

pub const OPTION_VIEW_ONLY: &str = "view_only";
pub const OPTION_SHOW_MONITORS_TOOLBAR: &str = "show_monitors_toolbar";
pub const OPTION_SHOW_REMOTE_CURSOR: &str = "show_remote_cursor";
pub const OPTION_FOLLOW_REMOTE_CURSOR: &str = "follow_remote_cursor";
pub const OPTION_FOLLOW_REMOTE_WINDOW: &str = "follow_remote_window";
pub const OPTION_SHOW_QUALITY_MONITOR: &str = "show_quality_monitor";
pub const OPTION_DISABLE_AUDIO: &str = "disable_audio";
pub const OPTION_ENABLE_REMOTE_PRINTER: &str = "enable-remote-printer";
pub const OPTION_DISABLE_CLIPBOARD: &str = "disable_clipboard";
pub const OPTION_LOCK_AFTER_SESSION_END: &str = "lock_after_session_end";
pub const OPTION_PRIVACY_MODE: &str = "privacy_mode";
pub const OPTION_TOUCH_MODE: &str = "touch-mode";
pub const OPTION_SYNC_INIT_CLIPBOARD: &str = "sync-init-clipboard";
pub const OPTION_THEME: &str = "theme";
pub const OPTION_REMOTE_MENUBAR_DRAG_LEFT: &str = "remote-menubar-drag-left";
pub const OPTION_REMOTE_MENUBAR_DRAG_RIGHT: &str = "remote-menubar-drag-right";
pub const OPTION_HIDE_AB_TAGS_PANEL: &str = "hideAbTagsPanel";
pub const OPTION_ENABLE_CONFIRM_CLOSING_TABS: &str = "enable-confirm-closing-tabs";
pub const OPTION_ENABLE_OPEN_NEW_CONNECTIONS_IN_TABS: &str = "enable-open-new-connections-in-tabs";
pub const OPTION_TEXTURE_RENDER: &str = "use-texture-render";
// Internal health record written by the texture-render watchdog/probe;
// "failed-*" flips the texture-render default to opt-in on this machine.
pub const OPTION_TEXTURE_RENDER_HEALTH: &str = "texture-render-health";
pub const OPTION_ALLOW_D3D_RENDER: &str = "allow-d3d-render";
pub const OPTION_ENABLE_CHECK_UPDATE: &str = "enable-check-update";
pub const OPTION_ALLOW_AUTO_UPDATE: &str = "allow-auto-update";
pub const OPTION_SYNC_AB_WITH_RECENT_SESSIONS: &str = "sync-ab-with-recent-sessions";
pub const OPTION_SYNC_AB_TAGS: &str = "sync-ab-tags";
pub const OPTION_FILTER_AB_BY_INTERSECTION: &str = "filter-ab-by-intersection";
pub const OPTION_ACCESS_MODE: &str = "access-mode";
pub const OPTION_ENABLE_KEYBOARD: &str = "enable-keyboard";
pub const OPTION_ENABLE_CLIPBOARD: &str = "enable-clipboard";
pub const OPTION_ENABLE_FILE_TRANSFER: &str = "enable-file-transfer";
pub const OPTION_ENABLE_CAMERA: &str = "enable-camera";
pub const OPTION_ENABLE_TERMINAL: &str = "enable-terminal";
pub const OPTION_TERMINAL_PERSISTENT: &str = "terminal-persistent";
pub const OPTION_ENABLE_AUDIO: &str = "enable-audio";
pub const OPTION_ENABLE_TUNNEL: &str = "enable-tunnel";
pub const OPTION_ENABLE_REMOTE_RESTART: &str = "enable-remote-restart";
pub const OPTION_ENABLE_RECORD_SESSION: &str = "enable-record-session";
pub const OPTION_ENABLE_BLOCK_INPUT: &str = "enable-block-input";
pub const OPTION_ENABLE_PRIVACY_MODE: &str = "enable-privacy-mode";
pub const OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW: &str = "enable-perm-change-in-accept-window";
pub const OPTION_ALLOW_SCOPE_VIOLATION_CLOSE: &str = "allow-scope-violation-close";
pub const OPTION_ALLOW_SCOPE_VIOLATION_ALARM: &str = "allow-scope-violation-alarm";
pub const OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION: &str = "allow-remote-config-modification";
pub const OPTION_ENABLE_LAN_DISCOVERY: &str = "enable-lan-discovery";
pub const OPTION_DIRECT_ACCESS_PORT: &str = "direct-access-port";
pub const OPTION_WHITELIST: &str = "whitelist";
pub const OPTION_ID_WHITELIST: &str = "id-whitelist";
pub const OPTION_ALLOW_AUTO_DISCONNECT: &str = "allow-auto-disconnect";
pub const OPTION_AUTO_DISCONNECT_TIMEOUT: &str = "auto-disconnect-timeout";
pub const OPTION_ALLOW_ONLY_CONN_WINDOW_OPEN: &str = "allow-only-conn-window-open";
pub const OPTION_ALLOW_AUTO_RECORD_INCOMING: &str = "allow-auto-record-incoming";
pub const OPTION_ALLOW_AUTO_RECORD_OUTGOING: &str = "allow-auto-record-outgoing";
pub const OPTION_HIDE_RECORDING_BUTTON: &str = "hide-recording-button";
pub const OPTION_WINDOWS_SERVICE_VIDEO_SAVE_DIRECTORY: &str =
    "windows-service-video-save-directory";
pub const OPTION_VIDEO_SAVE_DIRECTORY: &str = "video-save-directory";
pub const OPTION_ENABLE_ABR: &str = "enable-abr";
pub const OPTION_ALLOW_REMOVE_WALLPAPER: &str = "allow-remove-wallpaper";
pub const OPTION_ALLOW_ALWAYS_SOFTWARE_RENDER: &str = "allow-always-software-render";
pub const OPTION_ENABLE_HWCODEC: &str = "enable-hwcodec";
pub const OPTION_APPROVE_MODE: &str = "approve-mode";
pub const OPTION_VERIFICATION_METHOD: &str = "verification-method";
pub const OPTION_TEMPORARY_PASSWORD_LENGTH: &str = "temporary-password-length";
pub const OPTION_CUSTOM_RENDEZVOUS_SERVER: &str = "custom-rendezvous-server";
pub const OPTION_API_SERVER: &str = "api-server";
pub const OPTION_KEY: &str = "key";
pub const OPTION_PRESET_ADDRESS_BOOK_NAME: &str = "preset-address-book-name";
pub const OPTION_PRESET_ADDRESS_BOOK_TAG: &str = "preset-address-book-tag";
pub const OPTION_PRESET_ADDRESS_BOOK_ALIAS: &str = "preset-address-book-alias";
pub const OPTION_PRESET_ADDRESS_BOOK_PASSWORD: &str = "preset-address-book-password";
pub const OPTION_PRESET_ADDRESS_BOOK_NOTE: &str = "preset-address-book-note";
pub const OPTION_PRESET_DEVICE_USERNAME: &str = "preset-device-username";
pub const OPTION_PRESET_DEVICE_NAME: &str = "preset-device-name";
pub const OPTION_PRESET_NOTE: &str = "preset-note";
pub const OPTION_ENABLE_DIRECTX_CAPTURE: &str = "enable-directx-capture";
pub const OPTION_ENABLE_ANDROID_SOFTWARE_ENCODING_HALF_SCALE: &str =
    "enable-android-software-encoding-half-scale";
pub const OPTION_ENABLE_TRUSTED_DEVICES: &str = "enable-trusted-devices";
pub const OPTION_AV1_TEST: &str = "av1-test";
/// Maximum number of files allowed during a single file transfer request.
///
/// Key: `file-transfer-max-files`.
/// Unit: number of files (not bytes).
///
/// Behaviour:
/// - If set to a positive integer N, at most N files are allowed.
/// - If set to 0, a safe built-in default is used (see DEFAULT_MAX_VALIDATED_FILES).
/// - If unset, negative, or non-integer, no explicit limit is enforced for backward compatibility.
pub const OPTION_FILE_TRANSFER_MAX_FILES: &str = "file-transfer-max-files";
pub const OPTION_DISABLE_UDP: &str = "disable-udp";
pub const OPTION_SHOW_VIRTUAL_MOUSE: &str = "show-virtual-mouse";
// joystick is the virtual mouse.
// So `OPTION_SHOW_VIRTUAL_MOUSE` should also be set if `OPTION_SHOW_VIRTUAL_JOYSTICK` is set.
pub const OPTION_SHOW_VIRTUAL_JOYSTICK: &str = "show-virtual-joystick";
pub const OPTION_ENABLE_FLUTTER_HTTP_ON_RUST: &str = "enable-flutter-http-on-rust";
pub const OPTION_ALLOW_ASK_FOR_NOTE: &str = "allow-ask-for-note";

// built-in options
pub const OPTION_DISPLAY_NAME: &str = "display-name";
pub const OPTION_AVATAR: &str = "avatar";
pub const OPTION_PRESET_DEVICE_GROUP_NAME: &str = "preset-device-group-name";
pub const OPTION_PRESET_USERNAME: &str = "preset-user-name";
pub const OPTION_PRESET_STRATEGY_NAME: &str = "preset-strategy-name";
pub const OPTION_REMOVE_PRESET_PASSWORD_WARNING: &str = "remove-preset-password-warning";
pub const OPTION_HIDE_GENERAL_SETTINGS: &str = "hide-general-settings";
pub const OPTION_HIDE_SECURITY_SETTINGS: &str = "hide-security-settings";
pub const OPTION_HIDE_NETWORK_SETTINGS: &str = "hide-network-settings";
pub const OPTION_HIDE_SERVER_SETTINGS: &str = "hide-server-settings";
pub const OPTION_HIDE_PROXY_SETTINGS: &str = "hide-proxy-settings";
pub const OPTION_HIDE_REMOTE_PRINTER_SETTINGS: &str = "hide-remote-printer-settings";
pub const OPTION_HIDE_WEBSOCKET_SETTINGS: &str = "hide-websocket-settings";
pub const OPTION_HIDE_STOP_SERVICE: &str = "hide-stop-service";
pub const OPTION_ALLOW_COMMAND_LINE_SETTINGS_WHEN_SETTINGS_DISABLED: &str =
    "allow-command-line-settings-when-settings-disabled";

// Connection punch-through / port-forward options
pub const OPTION_ENABLE_TCP_PUNCH: &str = "enable-tcp-punch";
pub const OPTION_ENABLE_UDP_PUNCH: &str = "enable-udp-punch";
pub const OPTION_ENABLE_IPV6_PUNCH: &str = "enable-ipv6-punch";
pub const OPTION_ENABLE_PORT_FORWARD_MUX: &str = "enable-port-forward-mux";
pub const OPTION_ENABLE_WEBRTC: &str = "enable-webrtc";
pub const OPTION_ALLOW_KCP_CC: &str = "allow-kcp-congestion-control";
pub const OPTION_HIDE_USERNAME_ON_CARD: &str = "hide-username-on-card";
pub const OPTION_HIDE_HELP_CARDS: &str = "hide-help-cards";
pub const OPTION_DEFAULT_CONNECT_PASSWORD: &str = "default-connect-password";
pub const OPTION_HIDE_TRAY: &str = "hide-tray";
pub const OPTION_ONE_WAY_CLIPBOARD_REDIRECTION: &str = "one-way-clipboard-redirection";
pub const OPTION_ALLOW_LOGON_SCREEN_PASSWORD: &str = "allow-logon-screen-password";
pub const OPTION_ALLOW_DEEP_LINK_PASSWORD: &str = "allow-deep-link-password";
pub const OPTION_ALLOW_DEEP_LINK_SERVER_SETTINGS: &str = "allow-deep-link-server-settings";
pub const OPTION_ONE_WAY_FILE_TRANSFER: &str = "one-way-file-transfer";
pub const OPTION_ALLOW_HTTPS_21114: &str = "allow-https-21114";
pub const OPTION_USE_RAW_TCP_FOR_API: &str = "use-raw-tcp-for-api";
pub const OPTION_HIDE_POWERED_BY_ME: &str = "hide-powered-by-me";
pub const OPTION_MAIN_WINDOW_ALWAYS_ON_TOP: &str = "main-window-always-on-top";

// flutter local options
pub const OPTION_FLUTTER_REMOTE_MENUBAR_STATE: &str = "remoteMenubarState";
pub const OPTION_FLUTTER_PEER_SORTING: &str = "peer-sorting";
pub const OPTION_FLUTTER_PEER_TAB_INDEX: &str = "peer-tab-index";
pub const OPTION_FLUTTER_PEER_TAB_ORDER: &str = "peer-tab-order";
pub const OPTION_FLUTTER_PEER_TAB_VISIBLE: &str = "peer-tab-visible";
pub const OPTION_FLUTTER_PEER_CARD_UI_TYLE: &str = "peer-card-ui-type";
pub const OPTION_FLUTTER_CURRENT_AB_NAME: &str = "current-ab-name";
pub const OPTION_ALLOW_REMOTE_CM_MODIFICATION: &str = "allow-remote-cm-modification";
pub const OPTION_ALLOW_SYNC_CLIPBOARD_BETWEEN_SESSIONS: &str =
    "allow-sync-clipboard-between-sessions";

pub const OPTION_PRINTER_INCOMING_JOB_ACTION: &str = "printer-incomming-job-action";
pub const OPTION_PRINTER_ALLOW_AUTO_PRINT: &str = "allow-printer-auto-print";
pub const OPTION_PRINTER_SELECTED_NAME: &str = "printer-selected-name";

// android floating window options
pub const OPTION_DISABLE_FLOATING_WINDOW: &str = "disable-floating-window";
pub const OPTION_FLOATING_WINDOW_SIZE: &str = "floating-window-size";
pub const OPTION_FLOATING_WINDOW_UNTOUCHABLE: &str = "floating-window-untouchable";
pub const OPTION_FLOATING_WINDOW_TRANSPARENCY: &str = "floating-window-transparency";
pub const OPTION_FLOATING_WINDOW_SVG: &str = "floating-window-svg";

// android keep screen on
pub const OPTION_KEEP_SCREEN_ON: &str = "keep-screen-on";

// Server-side: keep host system awake during incoming sessions (Security setting)
pub const OPTION_KEEP_AWAKE_DURING_INCOMING_SESSIONS: &str = "keep-awake-during-incoming-sessions";

// Client-side: keep client system awake during outgoing sessions (General setting)
pub const OPTION_KEEP_AWAKE_DURING_OUTGOING_SESSIONS: &str = "keep-awake-during-outgoing-sessions";

pub const OPTION_DISABLE_GROUP_PANEL: &str = "disable-group-panel";
pub const OPTION_DISABLE_DISCOVERY_PANEL: &str = "disable-discovery-panel";
pub const OPTION_PRE_ELEVATE_SERVICE: &str = "pre-elevate-service";

// DEFAULT_DISPLAY_SETTINGS, OVERWRITE_DISPLAY_SETTINGS
pub const KEYS_DISPLAY_SETTINGS: &[&str] = &[
    OPTION_VIEW_ONLY,
    OPTION_SHOW_MONITORS_TOOLBAR,
    OPTION_COLLAPSE_TOOLBAR,
    OPTION_SHOW_REMOTE_CURSOR,
    OPTION_FOLLOW_REMOTE_CURSOR,
    OPTION_FOLLOW_REMOTE_WINDOW,
    OPTION_ZOOM_CURSOR,
    OPTION_SHOW_QUALITY_MONITOR,
    OPTION_DISABLE_AUDIO,
    OPTION_ENABLE_FILE_COPY_PASTE,
    OPTION_DISABLE_CLIPBOARD,
    OPTION_LOCK_AFTER_SESSION_END,
    OPTION_PRIVACY_MODE,
    OPTION_TOUCH_MODE,
    OPTION_I444,
    OPTION_REVERSE_MOUSE_WHEEL,
    OPTION_SWAP_LEFT_RIGHT_MOUSE,
    OPTION_DISPLAYS_AS_INDIVIDUAL_WINDOWS,
    OPTION_USE_ALL_MY_DISPLAYS_FOR_THE_REMOTE_SESSION,
    OPTION_VIEW_STYLE,
    OPTION_TERMINAL_PERSISTENT,
    OPTION_SCROLL_STYLE,
    OPTION_EDGE_SCROLL_EDGE_THICKNESS,
    OPTION_IMAGE_QUALITY,
    OPTION_CUSTOM_IMAGE_QUALITY,
    OPTION_CUSTOM_FPS,
    OPTION_CODEC_PREFERENCE,
    OPTION_SYNC_INIT_CLIPBOARD,
    OPTION_TRACKPAD_SPEED,
];
// DEFAULT_LOCAL_SETTINGS, OVERWRITE_LOCAL_SETTINGS
pub const KEYS_LOCAL_SETTINGS: &[&str] = &[
    OPTION_THEME,
    OPTION_LANGUAGE,
    OPTION_ENABLE_CONFIRM_CLOSING_TABS,
    OPTION_ENABLE_OPEN_NEW_CONNECTIONS_IN_TABS,
    OPTION_TEXTURE_RENDER,
    OPTION_ALLOW_D3D_RENDER,
    OPTION_SYNC_AB_WITH_RECENT_SESSIONS,
    OPTION_SYNC_AB_TAGS,
    OPTION_FILTER_AB_BY_INTERSECTION,
    OPTION_REMOTE_MENUBAR_DRAG_LEFT,
    OPTION_REMOTE_MENUBAR_DRAG_RIGHT,
    OPTION_HIDE_AB_TAGS_PANEL,
    OPTION_FLUTTER_REMOTE_MENUBAR_STATE,
    OPTION_FLUTTER_PEER_SORTING,
    OPTION_FLUTTER_PEER_TAB_INDEX,
    OPTION_FLUTTER_PEER_TAB_ORDER,
    OPTION_FLUTTER_PEER_TAB_VISIBLE,
    OPTION_FLUTTER_PEER_CARD_UI_TYLE,
    OPTION_FLUTTER_CURRENT_AB_NAME,
    OPTION_DISABLE_FLOATING_WINDOW,
    OPTION_FLOATING_WINDOW_SIZE,
    OPTION_FLOATING_WINDOW_UNTOUCHABLE,
    OPTION_FLOATING_WINDOW_TRANSPARENCY,
    OPTION_FLOATING_WINDOW_SVG,
    OPTION_KEEP_SCREEN_ON,
    // Client-side: keep client system awake during outgoing sessions (General setting)
    OPTION_KEEP_AWAKE_DURING_OUTGOING_SESSIONS,
    OPTION_DISABLE_GROUP_PANEL,
    OPTION_DISABLE_DISCOVERY_PANEL,
    OPTION_PRE_ELEVATE_SERVICE,
    OPTION_ALLOW_REMOTE_CM_MODIFICATION,
    OPTION_ALLOW_SYNC_CLIPBOARD_BETWEEN_SESSIONS,
    OPTION_ENABLE_CHECK_UPDATE,
    OPTION_PRINTER_INCOMING_JOB_ACTION,
    OPTION_PRINTER_ALLOW_AUTO_PRINT,
    OPTION_PRINTER_SELECTED_NAME,
    OPTION_ALLOW_AUTO_RECORD_OUTGOING,
    OPTION_HIDE_RECORDING_BUTTON,
    OPTION_VIDEO_SAVE_DIRECTORY,
    OPTION_ENABLE_TCP_PUNCH,
    OPTION_ENABLE_UDP_PUNCH,
    OPTION_ENABLE_IPV6_PUNCH,
    OPTION_ENABLE_PORT_FORWARD_MUX,
    OPTION_ENABLE_WEBRTC,
    OPTION_TOUCH_MODE,
    OPTION_SHOW_VIRTUAL_MOUSE,
    OPTION_SHOW_VIRTUAL_JOYSTICK,
    OPTION_ENABLE_FLUTTER_HTTP_ON_RUST,
    OPTION_ALLOW_ASK_FOR_NOTE,
];
// DEFAULT_SETTINGS, OVERWRITE_SETTINGS
pub const KEYS_SETTINGS: &[&str] = &[
    OPTION_ACCESS_MODE,
    OPTION_ENABLE_KEYBOARD,
    OPTION_ENABLE_CLIPBOARD,
    OPTION_ENABLE_FILE_TRANSFER,
    OPTION_ENABLE_CAMERA,
    OPTION_ENABLE_TERMINAL,
    OPTION_ENABLE_REMOTE_PRINTER,
    OPTION_ENABLE_AUDIO,
    OPTION_ENABLE_TUNNEL,
    OPTION_ENABLE_REMOTE_RESTART,
    OPTION_ENABLE_RECORD_SESSION,
    OPTION_ENABLE_BLOCK_INPUT,
    OPTION_ENABLE_PRIVACY_MODE,
    OPTION_ALLOW_SCOPE_VIOLATION_CLOSE,
    OPTION_ALLOW_SCOPE_VIOLATION_ALARM,
    OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION,
    OPTION_ALLOW_NUMERNIC_ONE_TIME_PASSWORD,
    OPTION_ENABLE_LAN_DISCOVERY,
    OPTION_DIRECT_SERVER,
    OPTION_DIRECT_ACCESS_PORT,
    OPTION_WHITELIST,
    OPTION_ID_WHITELIST,
    OPTION_ALLOW_AUTO_DISCONNECT,
    OPTION_AUTO_DISCONNECT_TIMEOUT,
    OPTION_ALLOW_ONLY_CONN_WINDOW_OPEN,
    OPTION_ALLOW_AUTO_RECORD_INCOMING,
    OPTION_WINDOWS_SERVICE_VIDEO_SAVE_DIRECTORY,
    OPTION_ENABLE_ABR,
    OPTION_ALLOW_REMOVE_WALLPAPER,
    OPTION_ALLOW_ALWAYS_SOFTWARE_RENDER,
    OPTION_ENABLE_HWCODEC,
    OPTION_APPROVE_MODE,
    OPTION_VERIFICATION_METHOD,
    OPTION_TEMPORARY_PASSWORD_LENGTH,
    OPTION_PROXY_URL,
    OPTION_PROXY_USERNAME,
    OPTION_PROXY_PASSWORD,
    OPTION_CUSTOM_RENDEZVOUS_SERVER,
    OPTION_API_SERVER,
    OPTION_KEY,
    OPTION_ALLOW_WEBSOCKET,
    OPTION_PRESET_ADDRESS_BOOK_NAME,
    OPTION_PRESET_ADDRESS_BOOK_TAG,
    OPTION_PRESET_ADDRESS_BOOK_ALIAS,
    OPTION_PRESET_ADDRESS_BOOK_PASSWORD,
    OPTION_PRESET_ADDRESS_BOOK_NOTE,
    OPTION_PRESET_DEVICE_USERNAME,
    OPTION_PRESET_DEVICE_NAME,
    OPTION_PRESET_NOTE,
    OPTION_ENABLE_DIRECTX_CAPTURE,
    OPTION_ENABLE_ANDROID_SOFTWARE_ENCODING_HALF_SCALE,
    OPTION_ENABLE_TRUSTED_DEVICES,
    OPTION_RELAY_SERVER,
    OPTION_ICE_SERVERS,
    OPTION_DISABLE_UDP,
    OPTION_ALLOW_INSECURE_TLS_FALLBACK,
    OPTION_KEEP_AWAKE_DURING_INCOMING_SESSIONS,
    OPTION_ALLOW_AUTO_UPDATE,
    OPTION_ALLOW_KCP_CC,
    OPTION_ALLOW_WEBRTC_CC,
];

// BUILDIN_SETTINGS
pub const KEYS_BUILDIN_SETTINGS: &[&str] = &[
    OPTION_DISPLAY_NAME,
    OPTION_AVATAR,
    OPTION_PRESET_DEVICE_GROUP_NAME,
    OPTION_PRESET_USERNAME,
    OPTION_PRESET_STRATEGY_NAME,
    OPTION_REMOVE_PRESET_PASSWORD_WARNING,
    OPTION_HIDE_GENERAL_SETTINGS,
    OPTION_HIDE_SECURITY_SETTINGS,
    OPTION_HIDE_NETWORK_SETTINGS,
    OPTION_HIDE_SERVER_SETTINGS,
    OPTION_HIDE_PROXY_SETTINGS,
    OPTION_HIDE_REMOTE_PRINTER_SETTINGS,
    OPTION_HIDE_WEBSOCKET_SETTINGS,
    OPTION_HIDE_STOP_SERVICE,
    OPTION_HIDE_USERNAME_ON_CARD,
    OPTION_HIDE_HELP_CARDS,
    OPTION_DEFAULT_CONNECT_PASSWORD,
    OPTION_HIDE_TRAY,
    OPTION_ONE_WAY_CLIPBOARD_REDIRECTION,
    OPTION_ALLOW_LOGON_SCREEN_PASSWORD,
    OPTION_ALLOW_DEEP_LINK_PASSWORD,
    OPTION_ALLOW_DEEP_LINK_SERVER_SETTINGS,
    OPTION_ONE_WAY_FILE_TRANSFER,
    OPTION_ALLOW_HTTPS_21114,
    OPTION_ALLOW_HOSTNAME_AS_ID,
    OPTION_REGISTER_DEVICE,
    OPTION_HIDE_POWERED_BY_ME,
    OPTION_MAIN_WINDOW_ALWAYS_ON_TOP,
    OPTION_FILE_TRANSFER_MAX_FILES,
    OPTION_DISABLE_CHANGE_PERMANENT_PASSWORD,
    OPTION_DISABLE_CHANGE_ID,
    OPTION_DISABLE_UNLOCK_PIN,
    OPTION_USE_RAW_TCP_FOR_API,
    OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW,
    OPTION_ALLOW_COMMAND_LINE_SETTINGS_WHEN_SETTINGS_DISABLED,
];

#[cfg(test)]
mod tests {
    /// The glob above and the constants below share one namespace, and Rust
    /// silently prefers the explicit item over a glob import. A key defined on
    /// both sides would therefore compile, with the client and the server
    /// disagreeing about its string value and nothing to signal it. Keep the
    /// two sets apart.
    #[test]
    fn key_names_do_not_collide_with_hbb_common() {
        fn names(src: &str) -> Vec<&str> {
            src.lines()
                .filter_map(|l| l.trim().strip_prefix("pub const "))
                .filter_map(|l| l.split(':').next())
                .map(str::trim)
                .filter(|n| n.starts_with("OPTION_") || n.starts_with("KEYS_"))
                .collect()
        }

        let here = names(include_str!("keys.rs"));
        let there = names(include_str!("../../../hbb_common/src/config.rs"));
        assert!(
            !here.is_empty() && !there.is_empty(),
            "key parsing found nothing"
        );

        let both: Vec<_> = here.iter().filter(|n| there.contains(n)).collect();
        assert!(
            both.is_empty(),
            "defined in both crates, so the local one shadows hbb_common's \
             with no diagnostic: {:?}",
            both
        );
    }
}
