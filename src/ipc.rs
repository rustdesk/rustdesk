#[path = "ipc/auth.rs"]
mod ipc_auth;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "ipc/fs.rs"]
mod ipc_fs;

#[cfg(all(feature = "flutter", feature = "plugin_framework"))]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::plugin::ipc::Plugin;
use crate::{
    common::{is_server, CheckTestNatType},
    privacy_mode,
    privacy_mode::PrivacyModeState,
    rendezvous_mediator::RendezvousMediator,
    ui_interface::{get_local_option, set_local_option},
};
use bytes::Bytes;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use clipboard::ClipboardFile;
#[cfg(target_os = "linux")]
use hbb_common::anyhow;
use hbb_common::{
    allow_err, bail, bytes,
    bytes_codec::BytesCodec,
    config::{self, keys::OPTION_ALLOW_WEBSOCKET, Config, Config2},
    futures::StreamExt as _,
    futures_util::sink::SinkExt,
    log, password_security as password, timeout,
    tokio::{
        self,
        io::{AsyncRead, AsyncWrite},
    },
    tokio_util::codec::Framed,
    ResultType,
};
#[cfg(windows)]
pub(crate) use ipc_auth::authorize_windows_portable_service_ipc_connection;
#[cfg(windows)]
pub(crate) use ipc_auth::ensure_peer_executable_matches_current_by_pid_opt;
#[cfg(windows)]
pub(crate) use ipc_auth::log_rejected_windows_ipc_connection;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use ipc_auth::{active_uid, authorize_service_scoped_ipc_connection};
#[cfg(target_os = "macos")]
use ipc_auth::authorize_user_server_process;
#[cfg(windows)]
use ipc_auth::{
    authorize_windows_main_ipc_connection, portable_service_listener_security_attributes,
    should_allow_everyone_create_on_windows,
};
#[cfg(target_os = "linux")]
pub(crate) use ipc_auth::{
    ensure_peer_executable_matches_current_by_fd, is_allowed_service_peer_uid,
    log_rejected_uinput_connection, peer_uid_from_fd,
};
#[cfg(target_os = "linux")]
use ipc_fs::terminal_count_candidate_uids;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use ipc_fs::{
    check_pid, ensure_secure_ipc_parent_dir, scrub_secure_ipc_parent_dir,
    should_scrub_parent_entries_after_check_pid, write_pid,
};
use parity_tokio_ipc::{
    Connection as Conn, ConnectionClient as ConnClient, Endpoint, Incoming, SecurityAttributes,
};
use serde_derive::{Deserialize, Serialize};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::cell::Cell;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::PermissionsExt;
#[cfg(all(target_os = "linux", feature = "drm"))]
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

// IPC actions here.
pub const IPC_ACTION_CLOSE: &str = "close";
#[cfg(target_os = "windows")]
const PORTABLE_SERVICE_IPC_HANDSHAKE_TIMEOUT_MS: u64 = 3_000;
#[cfg(target_os = "windows")]
pub(crate) const IPC_TOKEN_LEN: usize = 64;
#[cfg(target_os = "windows")]
const IPC_TOKEN_RANDOM_BYTES: usize = IPC_TOKEN_LEN / 2;
#[cfg(target_os = "windows")]
const _: () = assert!(IPC_TOKEN_LEN % 2 == 0);
pub static EXIT_RECV_CLOSE: AtomicBool = AtomicBool::new(true);

#[cfg(any(target_os = "linux", target_os = "macos"))]
thread_local! {
    static USE_USER_MAIN_IPC: Cell<bool> = Cell::new(false);
}

#[must_use = "bind this guard to a local variable to keep the IPC scope active"]
/// Thread-local guard for routing root main IPC to the active user on Linux/macOS.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) struct UserMainIpcScope {
    previous: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl UserMainIpcScope {
    pub(crate) fn new() -> Self {
        let previous = USE_USER_MAIN_IPC.with(|use_user_main| {
            let previous = use_user_main.get();
            use_user_main.set(true);
            previous
        });
        Self { previous }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for UserMainIpcScope {
    fn drop(&mut self) {
        USE_USER_MAIN_IPC.with(|use_user_main| use_user_main.set(self.previous));
    }
}

#[inline]
pub async fn connect_service(ms_timeout: u64) -> ResultType<ConnectionTmpl<ConnClient>> {
    connect(ms_timeout, crate::POSTFIX_SERVICE).await
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum FS {
    ReadEmptyDirs {
        dir: String,
        include_hidden: bool,
    },
    ReadDir {
        dir: String,
        include_hidden: bool,
    },
    RemoveDir {
        path: String,
        id: i32,
        recursive: bool,
    },
    RemoveFile {
        path: String,
        id: i32,
        file_num: i32,
    },
    CreateDir {
        path: String,
        id: i32,
    },
    NewWrite {
        path: String,
        id: i32,
        file_num: i32,
        files: Vec<(String, u64)>,
        overwrite_detection: bool,
        total_size: u64,
        conn_id: i32,
    },
    CancelWrite {
        id: i32,
    },
    WriteBlock {
        id: i32,
        file_num: i32,
        data: Bytes,
        compressed: bool,
    },
    WriteDone {
        id: i32,
        file_num: i32,
    },
    WriteError {
        id: i32,
        file_num: i32,
        err: String,
    },
    WriteOffset {
        id: i32,
        file_num: i32,
        offset_blk: u32,
    },
    CheckDigest {
        id: i32,
        file_num: i32,
        file_size: u64,
        last_modified: u64,
        is_upload: bool,
        is_resume: bool,
    },
    SendConfirm(Vec<u8>),
    Rename {
        id: i32,
        path: String,
        new_name: String,
    },
    // CM-side file reading operations (Windows only)
    // These enable Connection Manager to read files and stream them back to Connection
    ReadFile {
        path: String,
        id: i32,
        file_num: i32,
        include_hidden: bool,
        conn_id: i32,
        overwrite_detection: bool,
    },
    CancelRead {
        id: i32,
        conn_id: i32,
    },
    SendConfirmForRead {
        id: i32,
        file_num: i32,
        skip: bool,
        offset_blk: u32,
        conn_id: i32,
    },
    ReadAllFiles {
        path: String,
        id: i32,
        include_hidden: bool,
        conn_id: i32,
    },
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t")]
pub struct ClipboardNonFile {
    pub compress: bool,
    pub content: bytes::Bytes,
    pub content_len: usize,
    pub next_raw: bool,
    pub width: i32,
    pub height: i32,
    // message.proto: ClipboardFormat
    pub format: i32,
    pub special_name: String,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataKeyboard {
    Sequence(String),
    KeyDown(enigo::Key),
    KeyUp(enigo::Key),
    KeyClick(enigo::Key),
    GetKeyState(enigo::Key),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataKeyboardResponse {
    GetKeyState(bool),
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataMouse {
    MoveTo(i32, i32),
    MoveRelative(i32, i32),
    Down(enigo::MouseButton),
    Up(enigo::MouseButton),
    Click(enigo::MouseButton),
    ScrollX(i32),
    ScrollY(i32),
    Refresh,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataControl {
    Resolution {
        minx: i32,
        maxx: i32,
        miny: i32,
        maxy: i32,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataPortableService {
    Ping,
    Pong,
    AuthToken(String),
    AuthResult(bool),
    ConnCount(Option<usize>),
    Mouse((Vec<u8>, i32, String, u32, bool, bool)),
    Pointer((Vec<u8>, i32)),
    Key(Vec<u8>),
    RequestStart,
    WillClose,
    CmShowElevation(bool),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum Data {
    Login {
        id: i32,
        is_file_transfer: bool,
        is_view_camera: bool,
        is_terminal: bool,
        peer_id: String,
        name: String,
        avatar: String,
        authorized: bool,
        port_forward: String,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        file_transfer_enabled: bool,
        restart: bool,
        recording: bool,
        block_input: bool,
        privacy_mode: bool,
        from_switch: bool,
    },
    ChatMessage {
        text: String,
    },
    SwitchPermission {
        name: String,
        enabled: bool,
    },
    SystemInfo(Option<String>),
    ClickTime(i64),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    MouseMoveTime(i64),
    Authorize,
    Close,
    #[cfg(windows)]
    SAS,
    UserSid(Option<u32>),
    OnlineStatus(Option<(i64, bool)>),
    Config((String, Option<String>)),
    Options(Option<HashMap<String, String>>),
    NatType(Option<i32>),
    ConfirmedKey(Option<(Vec<u8>, Vec<u8>)>),
    RawMessage(Vec<u8>),
    Socks(Option<config::Socks5Server>),
    FS(FS),
    Test,
    SyncConfig(Option<Box<(Config, Config2)>>),
    #[cfg(target_os = "windows")]
    ClipboardFile(ClipboardFile),
    ClipboardFileEnabled(bool),
    #[cfg(target_os = "windows")]
    ClipboardNonFile(Option<(String, Vec<ClipboardNonFile>)>),
    PrivacyModeState((i32, PrivacyModeState, String)),
    TestRendezvousServer,
    Deployed,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Keyboard(DataKeyboard),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    KeyboardResponse(DataKeyboardResponse),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Mouse(DataMouse),
    Control(DataControl),
    Theme(String),
    Language(String),
    Empty,
    Disconnected,
    DataPortableService(DataPortableService),
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    SwitchSidesRequest(String),
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    SwitchSidesUuid(String, String, Option<bool>),
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    SwitchSidesBack,
    UrlLink(String),
    VoiceCallIncoming,
    StartVoiceCall,
    VoiceCallResponse(bool),
    CloseVoiceCall(String),
    #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Plugin(Plugin),
    #[cfg(windows)]
    SyncWinCpuUsage(Option<f64>),
    FileTransferLog((String, String)),
    #[cfg(windows)]
    ControlledSessionCount(usize),
    CmErr(String),
    // CM-side file reading responses (Windows only)
    // These are sent from CM back to Connection when CM handles file reading
    /// Response to ReadFile: contains initial file list or error
    ReadJobInitResult {
        id: i32,
        file_num: i32,
        include_hidden: bool,
        conn_id: i32,
        /// Serialized protobuf bytes of FileDirectory, or error string
        result: Result<Vec<u8>, String>,
    },
    /// File data block read by CM.
    ///
    /// The actual data is sent separately via `send_raw()` after this message to avoid
    /// JSON encoding overhead for large binary data. This mirrors the `WriteBlock` pattern.
    ///
    /// **Protocol:**
    /// - Sender: `send(FileBlockFromCM{...})` then `send_raw(data)`
    /// - Receiver: `next()` returns `FileBlockFromCM`, then `next_raw()` returns data bytes
    ///
    /// **Note on empty data (e.g., empty files):**
    /// Empty data is supported. The IPC connection uses `BytesCodec` with `raw=false` (default),
    /// which prefixes each frame with a length header. So `send_raw(Bytes::new())` sends a
    /// 1-byte frame (length=0), and `next_raw()` correctly returns an empty `BytesMut`.
    /// See `libs/hbb_common/src/bytes_codec.rs` test `test_codec2` for verification.
    FileBlockFromCM {
        id: i32,
        file_num: i32,
        /// Data is sent separately via `send_raw()` to avoid JSON encoding overhead.
        /// This field is skipped during serialization; sender must call `send_raw()` after sending.
        /// Receiver must call `next_raw()` and populate this field manually.
        #[serde(skip)]
        data: bytes::Bytes,
        compressed: bool,
        conn_id: i32,
    },
    /// File read completed successfully
    FileReadDone {
        id: i32,
        file_num: i32,
        conn_id: i32,
    },
    /// File read failed with error
    FileReadError {
        id: i32,
        file_num: i32,
        err: String,
        conn_id: i32,
    },
    /// Digest info from CM for overwrite detection
    FileDigestFromCM {
        id: i32,
        file_num: i32,
        last_modified: u64,
        file_size: u64,
        is_resume: bool,
        conn_id: i32,
    },
    /// Response to ReadAllFiles: recursive directory listing
    AllFilesResult {
        id: i32,
        conn_id: i32,
        path: String,
        /// Serialized protobuf bytes of FileDirectory, or error string
        result: Result<Vec<u8>, String>,
    },
    CheckHwcodec,
    #[cfg(feature = "flutter")]
    VideoConnCount(Option<usize>),
    // Although the key is not necessary, it is used to avoid hardcoding the key.
    WaylandScreencastRestoreToken((String, String)),
    HwCodecConfig(Option<String>),
    RemoveTrustedDevices(Vec<Bytes>),
    ClearTrustedDevices,
    #[cfg(all(target_os = "windows", feature = "flutter"))]
    PrinterData(Vec<u8>),
    InstallOption(Option<(String, String)>),
    #[cfg(all(
        feature = "flutter",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    ControllingSessionCount(usize),
    #[cfg(target_os = "linux")]
    TerminalSessionCount(usize),
    #[cfg(target_os = "windows")]
    PortForwardSessionCount(Option<usize>),
    SocksWs(Option<Box<(Option<config::Socks5Server>, String)>>),
    #[cfg(target_os = "macos")]
    HasNoActiveConns(Option<bool>),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Whiteboard((String, crate::whiteboard::CustomEvent)),
    ControlPermissionsRemoteModify(Option<bool>),
    #[cfg(target_os = "windows")]
    FileTransferEnabledState(Option<bool>),
    // --- DRM/KMS capture (opt-in `drm` feature) over the `_drm` service-scoped channel ---
    // All of the following are `cfg(all(linux, drm))`, so the drm-off IPC wire is byte-identical
    // to upstream. Protocol on `_drm`: on connect the root service sends `DrmDisplayList`, the
    // client replies `DrmStart{display}`, then the service streams `DrmFrame` + send_raw(BGRA) and
    // `DrmCursor` + send_raw(RGBA). A frame/cursor header is ALWAYS immediately followed by exactly
    // one `send_raw()` payload (the same header-then-raw pairing as `FileBlockFromCM`). This keeps
    // the header extensible. The zero-copy `DrmFrameDmabuf(DmabufDesc)` sibling below carries only a
    // small JSON metadata descriptor; the scanout dma-buf fd rides an SCM_RIGHTS ancillary message on
    // the same `DrmConn` send (see `DrmConn::send_msg`), so it has NO trailing `send_raw()` body.
    /// Client -> service: begin streaming the chosen display.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    // `need_cpu` is set by an unprivileged consumer that could not open a render-node convert context
    // (drmtap_open_render failed, or an old .so lacks the split symbols). The service then streams the
    // CPU-converted `DrmFrame` path for this connection instead of a dma-buf fd the consumer cannot
    // detile, so a render-node-less seat still captures instead of losing the stream.
    DrmStart { display: i32, need_cpu: bool },
    /// Service -> client: the enumerated DRM displays (sent once, before frames).
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmDisplayList(Vec<DrmDisplayInfo>),
    /// Service -> client: the connector topology changed mid-stream (a monitor hotplug/unplug/modeset,
    /// observed by the service's udev DRM-uevent listener). Carries the freshly-enumerated list so the
    /// consumer can swap its sticky positive availability cache off the hot path, WITHOUT re-probing
    /// `_drm` (which would trip the enumeration restart loop). Interleaved with frames on the same
    /// stream; carries no `send_raw()` body and no fd.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmDisplaysChanged(Vec<DrmDisplayInfo>),
    /// Service -> client: a frame header; the packed BGRA pixels follow via `send_raw()`.
    /// CPU-fallback path (old .so, no render node): pixels cross the wire.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmFrame { width: u32, height: u32 },
    /// Service -> client: a zero-copy dma-buf frame descriptor. The scanout fd is NOT a field; when
    /// `desc.has_fd` it rides an SCM_RIGHTS ancillary message on the same `DrmConn::send_msg`, and
    /// there is NO trailing `send_raw()` body. The unprivileged `--server` imports the fd and does
    /// the EGL detile/convert itself (see `DmabufDesc`).
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmFrameDmabuf(DmabufDesc),
    /// Service -> client: a hardware-cursor header; the RGBA pixels follow via `send_raw()`.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmCursor {
        id: u64,
        width: u32,
        height: u32,
        hotx: i32,
        hoty: i32,
    },
}

/// One enumerated DRM display shipped over `_drm` (physical geometry). The serializable IPC
/// form of `scrap::drm_reader::DisplaySnapshot`; the server augments it with the Wayland
/// logical geometry/scale, which needs the user session.
#[cfg(all(target_os = "linux", feature = "drm"))]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DrmDisplayInfo {
    pub name: String,
    pub crtc_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub active: bool,
}

/// Serializable metadata descriptor of a scanout dma-buf, shipped over `_drm` as the JSON payload of
/// `Data::DrmFrameDmabuf`. It mirrors `scrap::drm_reader::drmtap_dmabuf_desc` field-for-field EXCEPT
/// the process-local `dma_buf_fd` (which never serializes — it rides SCM_RIGHTS ancillary), and adds
/// `buffer_id` (the producer's stable pool key) and `has_fd` (whether this message's `send_msg`
/// carries the fd, vs an import-once cache hit that omits it). The converter rebuilds a
/// `drmtap_dmabuf_desc` from these fields and overwrites its `dma_buf_fd` with the received fd.
#[cfg(all(target_os = "linux", feature = "drm"))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DmabufDesc {
    /// Producer-side stable pool key (e.g. fb_id + a connection epoch). Distinct from `fb_id`, which
    /// is libdrmtap's import-once cache key.
    pub buffer_id: u64,
    pub width: u32,
    pub height: u32,
    /// DRM fourcc of the scanout.
    pub format: u32,
    /// DRM format modifier (tiling/compression).
    pub modifier: u64,
    /// KMS framebuffer id — libdrmtap's import-once cache key. 0 disables caching for this frame.
    pub fb_id: u32,
    /// Used entries in `offsets`/`pitches` (1..4); 0 is treated as 1.
    pub num_planes: u32,
    /// Per-plane byte offsets into the dma-buf (CCS main + aux + clear-color).
    pub offsets: [u32; 4],
    /// Per-plane strides in bytes; `pitches[0]` is the main-surface stride.
    pub pitches: [u32; 4],
    /// DRMTAP_EOTF_* (SDR=0, PQ=2, HLG=3). PQ triggers the HDR->SDR tone-map on convert.
    pub hdr_eotf: u32,
    /// Content/mastering peak luminance (cd/m2); 0 = unknown.
    pub hdr_max_nits: u32,
    /// True: this message's `send_msg` attaches the dma-buf fd in an SCM_RIGHTS cmsg. False: an
    /// import-once cache hit for `fb_id` — no fd attached, converter reuses its cached EGLImage.
    pub has_fd: bool,
}

#[tokio::main(flavor = "current_thread")]
pub async fn start(postfix: &str) -> ResultType<()> {
    let mut incoming = new_listener(postfix).await?;
    loop {
        if let Some(result) = incoming.next().await {
            match result {
                Ok(stream) => {
                    let mut stream = Connection::new(stream);
                    let postfix = postfix.to_owned();
                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    if config::is_service_ipc_postfix(&postfix) {
                        if !authorize_service_scoped_ipc_connection(&stream, &postfix) {
                            continue;
                        }
                    }
                    #[cfg(windows)]
                    if postfix.is_empty() {
                        // Windows main IPC (`postfix == ""`) is authorized here.
                        // Other security-sensitive channels use dedicated authorization paths:
                        // - `_portable_service`: portable-service listener + handshake policy
                        // - service-scoped postfixes: service-specific listener/authorization
                        if !authorize_windows_main_ipc_connection(&stream, &postfix) {
                            continue;
                        }
                    }
                    tokio::spawn(async move {
                        loop {
                            match stream.next().await {
                                Err(err) => {
                                    log::trace!("ipc '{}' connection closed: {}", postfix, err);
                                    break;
                                }
                                Ok(Some(data)) => {
                                    // On Linux/macOS, the protected `_service` channel is used only for
                                    // syncing config between root service and the active user process.
                                    //
                                    // NOTE: `is_service_ipc_postfix()` also includes `_uinput_*`, but those
                                    // channels are handled by the dedicated uinput listener/protocol in
                                    // `src/server/uinput.rs` and therefore do not share this Data enum
                                    // allowlist. The SyncConfig allowlist here is intentionally scoped to the
                                    // `_service` channel only.
                                    //
                                    // Keep this explicit branch to avoid policy drift between `_service` and
                                    // uinput IPC paths while still minimizing exposed message surface here.
                                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                                    if postfix == crate::POSTFIX_SERVICE {
                                        if matches!(&data, Data::SyncConfig(_)) {
                                            handle(data, &mut stream).await;
                                        } else {
                                            log::warn!(
                                                "Rejected non-sync data on protected _service IPC channel: postfix={}, data_kind={:?}, peer_uid={:?}",
                                                postfix,
                                                std::mem::discriminant(&data),
                                                stream.peer_uid()
                                            );
                                            // Close the connection to avoid keeping a protected channel
                                            // alive while repeatedly receiving invalid traffic.
                                            break;
                                        }
                                        continue;
                                    }
                                    handle(data, &mut stream).await;
                                }
                                Ok(None) => {
                                    // `Ok(None)` means a complete frame arrived but did not
                                    // deserialize into `Data`. Peer close/reset is returned as
                                    // `Err` by `ConnectionTmpl::next()`. Keep the historical
                                    // ignore behavior except on the protected `_service` channel.
                                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                                    {
                                        if postfix == crate::POSTFIX_SERVICE {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                Err(err) => {
                    log::error!("Couldn't get client: {:?}", err);
                }
            }
        }
    }
}

pub async fn new_listener(postfix: &str) -> ResultType<Incoming> {
    let path = Config::ipc_path(postfix);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let should_scrub_parent_entries = ensure_secure_ipc_parent_dir(&path, postfix)?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let existing_listener_alive = check_pid(postfix).await;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if should_scrub_parent_entries_after_check_pid(
        should_scrub_parent_entries,
        existing_listener_alive,
    ) {
        scrub_secure_ipc_parent_dir(&path, postfix)?;
    }
    let mut endpoint = Endpoint::new(path.clone());
    let security_attrs = {
        #[cfg(windows)]
        {
            if postfix == "_portable_service" {
                portable_service_listener_security_attributes()
            } else if should_allow_everyone_create_on_windows(postfix) {
                SecurityAttributes::allow_everyone_create()
            } else {
                Ok(SecurityAttributes::empty())
            }
        }
        #[cfg(not(windows))]
        {
            SecurityAttributes::allow_everyone_create()
        }
    };
    match security_attrs {
        Ok(attr) => endpoint.set_security_attributes(attr),
        Err(err) => {
            log::error!("Failed to set ipc{} security: {}", postfix, err);
            #[cfg(windows)]
            if postfix == "_portable_service" {
                // Fail closed for `_portable_service` when SDDL construction fails.
                // This endpoint is security-critical and must not start with default ACLs.
                return Err(err.into());
            }
        }
    };
    match endpoint.incoming() {
        Ok(incoming) => {
            if postfix == crate::POSTFIX_SERVICE {
                log::info!("Started protected ipc service server: postfix={}", postfix);
            } else {
                log::info!("Started ipc{} server at path: {}", postfix, &path);
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                // NOTE: On Linux/macOS, some IPC sockets are intentionally world-connectable
                // (0666) so the active (non-root) user process can connect. Authorization is
                // enforced at accept-time for these channels, and the protected `_service`
                // channel is further restricted by an explicit message allowlist (SyncConfig
                // only).
                let socket_mode = if config::is_service_ipc_postfix(postfix) {
                    0o0666
                } else {
                    0o0600
                };
                if let Err(err) =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(socket_mode))
                {
                    log::error!(
                        "Failed to set permissions on ipc{} socket at path {}: {}",
                        postfix,
                        &path,
                        err
                    );
                    std::fs::remove_file(&path).ok();
                    return Err(err.into());
                }
                write_pid(postfix);
            }
            Ok(incoming)
        }
        Err(err) => {
            log::error!(
                "Failed to start ipc{} server at path {}: {}",
                postfix,
                path,
                err
            );
            Err(err.into())
        }
    }
}

pub struct CheckIfRestart {
    stop_service: String,
    rendezvous_servers: Vec<String>,
    audio_input: String,
    voice_call_input: String,
    ws: String,
    disable_udp: String,
    allow_insecure_tls_fallback: String,
    api_server: String,
}

impl CheckIfRestart {
    pub fn new() -> CheckIfRestart {
        CheckIfRestart {
            stop_service: Config::get_option("stop-service"),
            rendezvous_servers: Config::get_rendezvous_servers(),
            audio_input: Config::get_option("audio-input"),
            voice_call_input: Config::get_option("voice-call-input"),
            ws: Config::get_option(OPTION_ALLOW_WEBSOCKET),
            disable_udp: Config::get_option(config::keys::OPTION_DISABLE_UDP),
            allow_insecure_tls_fallback: Config::get_option(
                config::keys::OPTION_ALLOW_INSECURE_TLS_FALLBACK,
            ),
            api_server: Config::get_option("api-server"),
        }
    }
}
impl Drop for CheckIfRestart {
    fn drop(&mut self) {
        // If https proxy is used, we need to restart rendezvous mediator.
        // No need to check if https proxy is used, because this option does not change frequently
        // and restarting mediator is safe even https proxy is not used.
        let allow_insecure_tls_fallback_changed = self.allow_insecure_tls_fallback
            != Config::get_option(config::keys::OPTION_ALLOW_INSECURE_TLS_FALLBACK);
        if allow_insecure_tls_fallback_changed
            || self.stop_service != Config::get_option("stop-service")
            || self.rendezvous_servers != Config::get_rendezvous_servers()
            || self.ws != Config::get_option(OPTION_ALLOW_WEBSOCKET)
            || self.disable_udp != Config::get_option(config::keys::OPTION_DISABLE_UDP)
            || self.api_server != Config::get_option("api-server")
        {
            if allow_insecure_tls_fallback_changed {
                hbb_common::tls::reset_tls_cache();
            }
            RendezvousMediator::restart();
        }
        if self.audio_input != Config::get_option("audio-input") {
            crate::audio_service::restart();
        }
        if self.voice_call_input != Config::get_option("voice-call-input") {
            crate::audio_service::set_voice_call_input_device(
                Some(Config::get_option("voice-call-input")),
                true,
            )
        }
    }
}

async fn handle(data: Data, stream: &mut Connection) {
    match data {
        Data::SystemInfo(_) => {
            let info = format!(
                "log_path: {}, config: {}, username: {}",
                Config::log_path().to_str().unwrap_or(""),
                Config::file().to_str().unwrap_or(""),
                crate::username(),
            );
            allow_err!(stream.send(&Data::SystemInfo(Some(info))).await);
        }
        Data::ClickTime(_) => {
            let t = crate::server::CLICK_TIME.load(Ordering::SeqCst);
            allow_err!(stream.send(&Data::ClickTime(t)).await);
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::MouseMoveTime(_) => {
            let t = crate::server::MOUSE_MOVE_TIME.load(Ordering::SeqCst);
            allow_err!(stream.send(&Data::MouseMoveTime(t)).await);
        }
        Data::Close => {
            log::info!("Receive close message");
            if EXIT_RECV_CLOSE.load(Ordering::SeqCst) {
                #[cfg(not(target_os = "android"))]
                crate::server::input_service::fix_key_down_timeout_at_exit();
                if is_server() {
                    let _ = privacy_mode::turn_off_privacy(0, Some(PrivacyModeState::OffByPeer));
                }
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                if crate::is_main() {
                    // below part is for main windows can be reopen during rustdesk installation and installing service from UI
                    // this make new ipc server (domain socket) can be created.
                    std::fs::remove_file(&Config::ipc_path("")).ok();
                    #[cfg(target_os = "linux")]
                    {
                        hbb_common::sleep((crate::platform::SERVICE_INTERVAL * 2) as f32 / 1000.0)
                            .await;
                        // https://github.com/rustdesk/rustdesk/discussions/9254
                        crate::run_me::<&str>(vec!["--no-server"]).ok();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        // our launchagent interval is 1 second
                        hbb_common::sleep(1.5).await;
                        std::process::Command::new("open")
                            .arg("-n")
                            .arg(&format!("/Applications/{}.app", crate::get_app_name()))
                            .spawn()
                            .ok();
                    }
                    // leave above open a little time
                    hbb_common::sleep(0.3).await;
                    // in case below exit failed
                    crate::platform::quit_gui();
                }
                std::process::exit(-1); // to make sure --server luauchagent process can restart because SuccessfulExit used
            }
        }
        Data::OnlineStatus(_) => {
            let x = config::get_online_state();
            let confirmed = Config::get_key_confirmed();
            allow_err!(stream.send(&Data::OnlineStatus(Some((x, confirmed)))).await);
        }
        Data::ConfirmedKey(None) => {
            let out = if Config::get_key_confirmed() {
                Some(Config::get_key_pair())
            } else {
                None
            };
            allow_err!(stream.send(&Data::ConfirmedKey(out)).await);
        }
        Data::Socks(s) => match s {
            None => {
                allow_err!(stream.send(&Data::Socks(Config::get_socks())).await);
            }
            Some(data) => {
                let _nat = CheckTestNatType::new();
                if data.proxy.is_empty() {
                    Config::set_socks(None);
                } else {
                    Config::set_socks(Some(data));
                }
                RendezvousMediator::restart();
                log::info!("socks updated");
            }
        },
        Data::SocksWs(s) => match s {
            None => {
                allow_err!(
                    stream
                        .send(&Data::SocksWs(Some(Box::new((
                            Config::get_socks(),
                            Config::get_option(OPTION_ALLOW_WEBSOCKET)
                        )))))
                        .await
                );
            }
            _ => {}
        },
        #[cfg(feature = "flutter")]
        Data::VideoConnCount(None) => {
            let n = crate::server::AUTHED_CONNS
                .lock()
                .unwrap()
                .iter()
                .filter(|x| x.conn_type == crate::server::AuthConnType::Remote)
                .count();
            allow_err!(stream.send(&Data::VideoConnCount(Some(n))).await);
        }
        Data::Config((name, value)) => match value {
            None => {
                let value;
                if name == "id" {
                    value = Some(Config::get_id());
                } else if name == "temporary-password" {
                    value = Some(password::temporary_password());
                } else if name == "permanent-password-storage-and-salt" {
                    let (storage, salt) = Config::get_local_permanent_password_storage_and_salt();
                    value = Some(storage + "\n" + &salt);
                } else if name == "permanent-password-set" {
                    value = Some(if Config::has_permanent_password() {
                        "Y".to_owned()
                    } else {
                        "N".to_owned()
                    });
                } else if name == "permanent-password-is-preset" {
                    value = Some(if Config::is_using_preset_password() {
                        "Y".to_owned()
                    } else {
                        "N".to_owned()
                    });
                } else if name == "salt" {
                    value = Some(Config::get_salt());
                } else if name == "rendezvous_server" {
                    value = Some(format!(
                        "{},{}",
                        Config::get_rendezvous_server(),
                        Config::get_rendezvous_servers().join(",")
                    ));
                } else if name == "rendezvous_servers" {
                    value = Some(Config::get_rendezvous_servers().join(","));
                } else if name == "fingerprint" {
                    value = if Config::get_key_confirmed() {
                        Some(crate::common::pk_to_fingerprint(Config::get_key_pair().1))
                    } else {
                        None
                    };
                } else if name == "hide_cm" {
                    value = if crate::hbbs_http::sync::is_pro() || crate::common::is_custom_client()
                    {
                        Some(hbb_common::password_security::hide_cm().to_string())
                    } else {
                        None
                    };
                } else if name == "voice-call-input" {
                    value = crate::audio_service::get_voice_call_input_device();
                } else if name == "unlock-pin" {
                    value = Some(Config::get_unlock_pin());
                } else if name == "trusted-devices" {
                    value = Some(Config::get_trusted_devices_json());
                } else {
                    value = None;
                }
                allow_err!(stream.send(&Data::Config((name, value))).await);
            }
            Some(value) => {
                let mut updated = true;
                if name == "id" {
                    // An empty id would wipe the local id and unconfirm the key (cf. #15626).
                    if value.is_empty() {
                        log::warn!("Ignoring empty id write over IPC");
                        updated = false;
                    } else {
                        Config::set_key_confirmed(false);
                        Config::set_id(&value);
                    }
                } else if name == "temporary-password" {
                    password::update_temporary_password();
                } else if name == "permanent-password" {
                    if Config::is_disable_change_permanent_password() {
                        log::warn!("Changing permanent password is disabled");
                        updated = false;
                    } else {
                        updated = Config::set_permanent_password(&value);
                    }
                    // Explicitly ACK/NACK permanent-password writes. This allows UIs/FFI to
                    // distinguish "accepted by daemon" vs "IPC send succeeded" without
                    // reading back any secret.
                    let ack = if updated { "Y" } else { "N" }.to_owned();
                    allow_err!(stream.send(&Data::Config((name.clone(), Some(ack)))).await);
                } else if name == "salt" {
                    Config::set_salt(&value);
                } else if name == "voice-call-input" {
                    crate::audio_service::set_voice_call_input_device(Some(value), true);
                } else if name == "unlock-pin" {
                    Config::set_unlock_pin(&value);
                } else {
                    return;
                }
                if updated {
                    log::info!("{} updated", name);
                }
            }
        },
        Data::Options(value) => match value {
            None => {
                let v = Config::get_options();
                allow_err!(stream.send(&Data::Options(Some(v))).await);
            }
            Some(value) => {
                let _chk = CheckIfRestart::new();
                let _nat = CheckTestNatType::new();
                if let Some(v) = value.get("privacy-mode-impl-key") {
                    crate::privacy_mode::switch(v);
                }
                Config::set_options(value);
                allow_err!(stream.send(&Data::Options(None)).await);
            }
        },
        Data::NatType(_) => {
            let t = Config::get_nat_type();
            allow_err!(stream.send(&Data::NatType(Some(t))).await);
        }
        Data::SyncConfig(Some(configs)) => {
            let (config, config2) = *configs;
            let _chk = CheckIfRestart::new();
            Config::set(config);
            Config2::set(config2);
            allow_err!(stream.send(&Data::SyncConfig(None)).await);
        }
        Data::SyncConfig(None) => {
            allow_err!(
                stream
                    .send(&Data::SyncConfig(Some(
                        (Config::get(), Config2::get()).into()
                    )))
                    .await
            );
        }
        #[cfg(windows)]
        Data::SyncWinCpuUsage(None) => {
            allow_err!(
                stream
                    .send(&Data::SyncWinCpuUsage(
                        hbb_common::platform::windows::cpu_uage_one_minute()
                    ))
                    .await
            );
        }
        Data::TestRendezvousServer => {
            crate::test_rendezvous_server();
        }
        Data::Deployed => {
            crate::rendezvous_mediator::NEEDS_DEPLOY.store(false, Ordering::SeqCst);
            crate::rendezvous_mediator::RendezvousMediator::restart();
        }
        #[cfg(feature = "flutter")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::SwitchSidesRequest(id) => {
            let uuid = uuid::Uuid::new_v4();
            crate::server::insert_switch_sides_uuid(id, uuid.clone());
            allow_err!(
                stream
                    .send(&Data::SwitchSidesRequest(uuid.to_string()))
                    .await
            );
        }
        #[cfg(feature = "flutter")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::SwitchSidesUuid(uuid, id, None) => {
            let allowed = uuid
                .parse::<uuid::Uuid>()
                .map(|uuid| crate::server::remove_pending_switch_sides_uuid(&id, &uuid))
                .unwrap_or(false);
            allow_err!(
                stream
                    .send(&Data::SwitchSidesUuid(uuid, id, Some(allowed)))
                    .await
            );
        }
        #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::Plugin(plugin) => crate::plugin::ipc::handle_plugin(plugin, stream).await,
        #[cfg(windows)]
        Data::ControlledSessionCount(_) => {
            allow_err!(
                stream
                    .send(&Data::ControlledSessionCount(
                        crate::Connection::alive_conns().len()
                    ))
                    .await
            );
        }
        #[cfg(target_os = "macos")]
        Data::HasNoActiveConns(None) => {
            allow_err!(
                stream
                    .send(&Data::HasNoActiveConns(Some(
                        crate::updater::has_no_active_conns()
                    )))
                    .await
            );
        }
        #[cfg(all(
            feature = "flutter",
            not(any(target_os = "android", target_os = "ios"))
        ))]
        Data::ControllingSessionCount(count) => {
            crate::updater::update_controlling_session_count(count);
        }
        #[cfg(target_os = "linux")]
        Data::TerminalSessionCount(_) => {
            let count = crate::terminal_service::get_terminal_session_count(true);
            allow_err!(stream.send(&Data::TerminalSessionCount(count)).await);
        }
        #[cfg(feature = "hwcodec")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::CheckHwcodec => {
            scrap::hwcodec::start_check_process();
        }
        #[cfg(feature = "hwcodec")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::HwCodecConfig(c) => {
            match c {
                None => {
                    let v = match scrap::hwcodec::HwCodecConfig::get_set_value() {
                        Some(v) => Some(serde_json::to_string(&v).unwrap_or_default()),
                        None => None,
                    };
                    allow_err!(stream.send(&Data::HwCodecConfig(v)).await);
                }
                Some(v) => {
                    // --server and portable
                    scrap::hwcodec::HwCodecConfig::set(v);
                }
            }
        }
        Data::WaylandScreencastRestoreToken((key, value)) => {
            let v = if value == "get" {
                let opt = get_local_option(key.clone());
                #[cfg(not(target_os = "linux"))]
                {
                    Some(opt)
                }
                #[cfg(target_os = "linux")]
                {
                    let v = if opt.is_empty() {
                        if scrap::wayland::pipewire::is_rdp_session_hold() {
                            "fake token".to_string()
                        } else {
                            "".to_owned()
                        }
                    } else {
                        opt
                    };
                    Some(v)
                }
            } else if value == "clear" {
                set_local_option(key.clone(), "".to_owned());
                #[cfg(target_os = "linux")]
                scrap::wayland::pipewire::close_session();
                Some("".to_owned())
            } else {
                None
            };
            if let Some(v) = v {
                allow_err!(
                    stream
                        .send(&Data::WaylandScreencastRestoreToken((key, v)))
                        .await
                );
            }
        }
        Data::RemoveTrustedDevices(v) => {
            Config::remove_trusted_devices(&v);
        }
        Data::ClearTrustedDevices => {
            Config::clear_trusted_devices();
        }
        Data::InstallOption(opt) => match opt {
            Some((_k, _v)) => {
                #[cfg(target_os = "windows")]
                if let Err(e) = crate::platform::windows::update_install_option(&_k, &_v) {
                    log::error!(
                        "Failed to update install option \"{}\" to \"{}\", error: {}",
                        &_k,
                        &_v,
                        e
                    );
                }
            }
            None => {
                // `None` is usually used to get values.
                // This branch is left blank for unification and further use.
            }
        },
        #[cfg(target_os = "windows")]
        Data::PortForwardSessionCount(c) => match c {
            None => {
                let count = crate::server::AUTHED_CONNS
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|c| c.conn_type == crate::server::AuthConnType::PortForward)
                    .count();
                allow_err!(
                    stream
                        .send(&Data::PortForwardSessionCount(Some(count)))
                        .await
                );
            }
            _ => {
                // Port forward session count is only a get value.
            }
        },
        Data::ControlPermissionsRemoteModify(_) => {
            use hbb_common::rendezvous_proto::control_permissions::Permission;
            let state =
                crate::server::get_control_permission_state(Permission::remote_modify, true);
            allow_err!(
                stream
                    .send(&Data::ControlPermissionsRemoteModify(state))
                    .await
            );
        }
        #[cfg(target_os = "windows")]
        Data::FileTransferEnabledState(_) => {
            use hbb_common::rendezvous_proto::control_permissions::Permission;
            let state = crate::server::get_control_permission_state(Permission::file, false);
            let enabled = state.unwrap_or_else(|| {
                crate::server::Connection::is_permission_enabled_locally(
                    config::keys::OPTION_ENABLE_FILE_TRANSFER,
                )
            });
            allow_err!(
                stream
                    .send(&Data::FileTransferEnabledState(Some(enabled)))
                    .await
            );
        }
        _ => {}
    };
}

#[cfg(target_os = "windows")]
pub(crate) fn generate_one_time_ipc_token() -> ResultType<String> {
    use hbb_common::rand::{rngs::OsRng, RngCore as _};
    use std::fmt::Write as _;

    let mut random_bytes = [0u8; IPC_TOKEN_RANDOM_BYTES];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut random_bytes).map_err(|err| {
        hbb_common::anyhow::anyhow!(
            "failed to generate portable service ipc token from OsRng: {}",
            err
        )
    })?;

    let mut token = String::with_capacity(IPC_TOKEN_LEN);
    for byte in random_bytes {
        let _ = write!(token, "{:02x}", byte);
    }
    Ok(token)
}

#[cfg(target_os = "windows")]
pub(crate) fn constant_time_ipc_token_eq(expected: &str, candidate: &str) -> bool {
    if expected.len() != IPC_TOKEN_LEN || candidate.len() != IPC_TOKEN_LEN {
        return false;
    }
    expected
        .as_bytes()
        .iter()
        .zip(candidate.as_bytes().iter())
        .fold(0u8, |diff, (left, right)| diff | (*left ^ *right))
        == 0
}

#[cfg(target_os = "windows")]
pub(crate) async fn portable_service_ipc_handshake_as_client<T>(
    stream: &mut ConnectionTmpl<T>,
    token: &str,
) -> ResultType<()>
where
    T: AsyncRead + AsyncWrite + std::marker::Unpin,
{
    stream
        .send(&Data::DataPortableService(DataPortableService::AuthToken(
            token.to_owned(),
        )))
        .await?;
    match stream
        .next_timeout(PORTABLE_SERVICE_IPC_HANDSHAKE_TIMEOUT_MS)
        .await?
    {
        Some(Data::DataPortableService(DataPortableService::AuthResult(true))) => Ok(()),
        Some(Data::DataPortableService(DataPortableService::AuthResult(false))) => {
            bail!("portable service ipc handshake was rejected by server")
        }
        Some(_) | None => bail!("portable service ipc handshake returned an unexpected response"),
    }
}

#[cfg(target_os = "windows")]
pub(crate) async fn portable_service_ipc_handshake_as_server<T, F>(
    stream: &mut ConnectionTmpl<T>,
    mut validate_token: F,
) -> ResultType<()>
where
    T: AsyncRead + AsyncWrite + std::marker::Unpin,
    // Token validators must use `constant_time_ipc_token_eq` or an equivalent
    // fixed-length comparison; this handshake is part of the privilege boundary.
    F: FnMut(&str) -> bool,
{
    let authorized = match stream
        .next_timeout(PORTABLE_SERVICE_IPC_HANDSHAKE_TIMEOUT_MS)
        .await?
    {
        Some(Data::DataPortableService(DataPortableService::AuthToken(token))) => {
            validate_token(&token)
        }
        Some(_) | None => false,
    };
    stream
        .send(&Data::DataPortableService(DataPortableService::AuthResult(
            authorized,
        )))
        .await?;
    if !authorized {
        bail!("portable service ipc handshake failed")
    }
    Ok(())
}

#[inline]
async fn connect_with_path(ms_timeout: u64, path: &str) -> ResultType<ConnectionTmpl<ConnClient>> {
    let client = timeout(ms_timeout, Endpoint::connect(path)).await??;
    Ok(ConnectionTmpl::new(client))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline]
fn select_server_uid_for_user_main_ipc(
    server_uids: &[u32],
    active_uid: Option<u32>,
    prefer_root: bool,
) -> ResultType<u32> {
    let mut server_uids = server_uids.to_vec();
    server_uids.sort_unstable();
    server_uids.dedup();

    match server_uids.as_slice() {
        [] => {
            if let Some(uid) = active_uid {
                // If no `--server` processes are found but the active user is identifiable,
                // try the active user anyway because the main process may also listen on "" IPC.
                return Ok(uid);
            } else {
                bail!("No --server process found for user main IPC")
            }
        }
        [uid] => return Ok(*uid),
        _ => {}
    }

    if prefer_root && server_uids.contains(&0) {
        return Ok(0);
    }
    if let Some(active_uid) = active_uid.filter(|uid| server_uids.contains(uid)) {
        return Ok(active_uid);
    }
    bail!("Multiple --server processes found for user main IPC");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn running_server_uids_for_current_exe() -> ResultType<Vec<u32>> {
    let current_exe = std::env::current_exe()?;
    let current_exe_path = std::fs::canonicalize(&current_exe)?;
    let current_pid = hbb_common::sysinfo::Pid::from_u32(std::process::id());
    let mut sys = hbb_common::sysinfo::System::new();
    sys.refresh_processes();
    let mut server_uids = Vec::new();
    for process in sys.processes().values() {
        if process.pid() == current_pid {
            continue;
        }
        if process.cmd().get(1).map_or(true, |arg| arg != "--server") {
            continue;
        }
        let Ok(process_path) = std::fs::canonicalize(process.exe()) else {
            continue;
        };
        if process_path != current_exe_path {
            continue;
        }
        let Some(uid) = process.user_id().map(|uid| **uid as u32) else {
            // Root CLI management commands need a stable matching `--server` target.
            // If this key process races during enumeration, failing the command is clearer
            // than silently skipping it; `--server` is not expected to exit frequently.
            bail!("Failed to read --server process uid");
        };
        server_uids.push(uid);
    }
    Ok(server_uids)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn user_main_ipc_server_uid() -> ResultType<u32> {
    let server_uids = running_server_uids_for_current_exe()?;
    #[cfg(target_os = "linux")]
    let prefer_root = crate::platform::linux::is_login_screen_wayland();
    #[cfg(target_os = "macos")]
    let prefer_root = false;
    select_server_uid_for_user_main_ipc(&server_uids, active_uid(), prefer_root)
}

pub async fn connect(ms_timeout: u64, postfix: &str) -> ResultType<ConnectionTmpl<ConnClient>> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let use_user_main_ipc = USE_USER_MAIN_IPC.with(|use_user_main| use_user_main.get());
        let is_root_main_ipc =
            unsafe { hbb_common::libc::geteuid() == 0 } && postfix.is_empty() && use_user_main_ipc;
        if is_root_main_ipc {
            let uid = user_main_ipc_server_uid()?;
            let path = Config::ipc_path_for_uid(uid, postfix);
            return connect_with_path(ms_timeout, &path).await;
        }
        let path = Config::ipc_path(postfix);
        return connect_with_path(ms_timeout, &path).await;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let path = Config::ipc_path(postfix);
        connect_with_path(ms_timeout, &path).await
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub async fn connect_for_uid(
    ms_timeout: u64,
    uid: u32,
    postfix: &str,
) -> ResultType<ConnectionTmpl<ConnClient>> {
    let path = Config::ipc_path_for_uid(uid, postfix);
    let conn = connect_with_path(ms_timeout, &path).await?;
    #[cfg(target_os = "macos")]
    if postfix.is_empty()
        && !authorize_user_server_process(conn.peer_uid(), conn.peer_pid(), uid)
    {
        bail!("Rejected user IPC peer for uid {}", uid);
    }
    Ok(conn)
}

#[cfg(target_os = "linux")]
#[tokio::main(flavor = "current_thread")]
pub async fn start_pa() {
    use crate::audio_service::AUDIO_DATA_SIZE_U8;

    match new_listener("_pa").await {
        Ok(mut incoming) => {
            loop {
                if let Some(result) = incoming.next().await {
                    match result {
                        Ok(stream) => {
                            let mut stream = Connection::new(stream);
                            let mut device: String = "".to_owned();
                            if let Some(Ok(Some(Data::Config((_, Some(x)))))) =
                                stream.next_timeout2(1000).await
                            {
                                device = x;
                            }
                            if !device.is_empty() {
                                device = crate::platform::linux::get_pa_source_name(&device);
                            }
                            if device.is_empty() {
                                device = crate::platform::linux::get_pa_monitor();
                            }
                            if device.is_empty() {
                                continue;
                            }
                            let spec = pulse::sample::Spec {
                                format: pulse::sample::Format::F32le,
                                channels: 2,
                                rate: crate::platform::PA_SAMPLE_RATE,
                            };
                            log::info!("pa monitor: {:?}", device);
                            // systemctl --user status pulseaudio.service
                            let mut buf: Vec<u8> = vec![0; AUDIO_DATA_SIZE_U8];
                            match psimple::Simple::new(
                                None,                             // Use the default server
                                &crate::get_app_name(),           // Our application’s name
                                pulse::stream::Direction::Record, // We want a record stream
                                Some(&device),                    // Use the default device
                                "record",                         // Description of our stream
                                &spec,                            // Our sample format
                                None,                             // Use default channel map
                                None, // Use default buffering attributes
                            ) {
                                Ok(s) => loop {
                                    if let Ok(_) = s.read(&mut buf) {
                                        let out =
                                            if buf.iter().filter(|x| **x != 0).next().is_none() {
                                                vec![]
                                            } else {
                                                buf.clone()
                                            };
                                        if let Err(err) = stream.send_raw(out.into()).await {
                                            log::error!("Failed to send audio data:{}", err);
                                            break;
                                        }
                                    }
                                },
                                Err(err) => {
                                    log::error!("Could not create simple pulse: {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("Couldn't get pa client: {:?}", err);
                        }
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start pa ipc server: {}", err);
        }
    }
}

/// Filesystem path of the `_drm` capture socket. It lives beside the hardened `_service` socket in
/// the shared `/tmp/<app>-service` directory (cross-uid, traversable) so the root `--service` and
/// the user `--server` share one uid-independent path. Derived from the real `_service` path so we
/// inherit hbb_common's directory convention WITHOUT teaching hbb_common about a drm-specific
/// postfix (keeps the isolation clean: no shared-lib change). Both ends call this.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) fn drm_ipc_path() -> String {
    let service_path = Config::ipc_path("_service");
    let dir = std::path::Path::new(&service_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"));
    dir.join("ipc_drm").to_string_lossy().into_owned()
}

/// Connect (from the user `--server`) to the root service's `_drm` capture channel. Uses the
/// derived `drm_ipc_path()` rather than `Config::ipc_path` since `_drm` is not a hbb_common
/// service postfix (Option 2 isolation — no shared-lib change). Returns a [`DrmConn`] (bespoke
/// SCM_RIGHTS framing) rather than the `Framed<_, BytesCodec>` `ConnectionTmpl`: the `_drm` channel
/// must carry the scanout dma-buf fd as ancillary data, which the codec cannot do (see `DrmConn`).
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) async fn connect_drm(ms_timeout: u64) -> ResultType<DrmConn> {
    use std::os::fd::AsRawFd;
    let path = drm_ipc_path();
    let stream = timeout(ms_timeout, tokio::net::UnixStream::connect(&path)).await??;
    // The producer MUST be root. DRM/KMS scanout export is a root-service capability, and the DRM
    // path outranks PipeWire (an available DRM stream suppresses the portal consent prompt), so a
    // non-root peer that won a socket-path race must not be trusted to supply the display list,
    // frames and an arbitrary dma-buf fd (review 4.1). The producer direction is authorized in
    // handle_drm_conn; this closes the same gap on the consumer direction.
    if peer_uid_from_fd(stream.as_raw_fd()) != Some(0) {
        bail!("drm: _drm producer is not root; refusing to consume");
    }
    Ok(DrmConn::new(stream))
}

/// Bind the `_drm` listener. Unlike `new_listener`, this does not route through hbb_common's
/// service-postfix machinery — it places the socket in the shared service dir directly, so the
/// drm-off build needs no hbb_common change. The socket is 0666 (world-connectable) so the
/// unprivileged `--server` can reach it; every accepted peer is still authorized in
/// `handle_drm_conn` (root or the active session uid + exe identity), so connectable != authorized.
#[cfg(all(target_os = "linux", feature = "drm"))]
async fn new_drm_listener() -> ResultType<Incoming> {
    let path = drm_ipc_path();
    // Ensure the shared service dir exists at its hardened (0711) mode. Passing the `_service`
    // postfix reuses hbb_common's expected mode for that directory; it only creates/chmods the
    // directory (no pid/socket side effects) and is idempotent with the real `_service` listener.
    let _ = ensure_secure_ipc_parent_dir(&path, "_service")?;
    // Clear any stale socket from a previous run before binding.
    std::fs::remove_file(&path).ok();
    let mut endpoint = Endpoint::new(path.clone());
    endpoint.set_security_attributes(SecurityAttributes::allow_everyone_create()?);
    let incoming = endpoint.incoming()?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).map_err(|err| {
        std::fs::remove_file(&path).ok();
        err
    })?;
    log::info!("Started drm ipc server at path: {}", &path);
    Ok(incoming)
}

/// Message from a per-connection DRM worker thread (which owns the `!Send` `DrmReader`) to its
/// async socket task. The worker does the blocking device I/O; the task only forwards to the wire.
#[cfg(all(target_os = "linux", feature = "drm"))]
enum DrmProducerMsg {
    /// Enumerated displays, sent once before any frame so the task can answer the handshake.
    Displays(Vec<DrmDisplayInfo>),
    /// A captured frame (split/zero-copy path): the serializable dma-buf descriptor plus the (owned)
    /// scanout fd to hand to the peer via SCM_RIGHTS. The worker always produces a real `fd` here; the
    /// async task's `ExportLedger` decides whether to actually attach it (`desc.has_fd`) or elide it as
    /// an import-once cache hit. The `OwnedFd` is closed once the send has dup'd it into the peer (or
    /// immediately, when elided).
    Frame {
        desc: DmabufDesc,
        fd: Option<OwnedFd>,
    },
    /// A captured frame (CPU-mapped fallback path): a full packed-BGRA frame body. Used when the
    /// loaded libdrmtap predates the split API (no `drmtap_grab_desc`) or the seat has no transferable
    /// dma-buf (ENOTSUP). Forwarded as `Data::DrmFrame{width,height}` + `send_raw(BGRA)`, exactly like
    /// the pre-split protocol, so an unprivileged converter is never required.
    FrameCpu {
        width: u32,
        height: u32,
        data: Bytes,
    },
    /// A changed hardware-cursor shape + its packed RGBA pixels.
    Cursor {
        id: u64,
        width: u32,
        height: u32,
        hotx: i32,
        hoty: i32,
        colors: Vec<u8>,
    },
}

/// Sets the shared stop flag when the async task ends (any path), so the blocking worker thread
/// terminates promptly even while it is between channel sends (e.g. spinning on WouldBlock).
#[cfg(all(target_os = "linux", feature = "drm"))]
struct DrmStopGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
#[cfg(all(target_os = "linux", feature = "drm"))]
impl Drop for DrmStopGuard {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Producer-side fd-elision ledger (root `--service`, one per `_drm` connection). Decides, per
/// exported frame, whether the scanout dma-buf fd must ride an SCM_RIGHTS cmsg (`has_fd = true`) or
/// can be elided as an import-once cache hit (`has_fd = false`) because the peer's converter already
/// imported that `fb_id`. Keyed by `fb_id -> (modifier, dims)`; a change in any of those (a resize,
/// a modifier/tiling change, or a recycled fb_id that also changed geometry) forces a real fd, and a
/// modeset/hotplug that invalidates the CRTC ends the connection (so a reconnect starts with a fresh,
/// empty ledger — matching the peer's fresh, empty converter cache).
///
/// SAFETY / CORRECTNESS: eliding relies solely on `(fb_id, modifier, dims)` uniquely identifying a
/// buffer, but the kernel can recycle an `fb_id` onto a *different* buffer with identical geometry
/// and modifier; eliding then would serve a stale EGLImage. libdrmtap's own import cache keys on
/// `fb_id + dma-buf inode` and can re-import ONLY when it is handed a real fd. Because always sending
/// the fd is cheap (the converter still imports once per `fb_id` and closes the surplus fd) and is
/// strictly safe, `DRM_FD_ELISION` defaults to `false` for v1 (always send). The ledger's `epoch`
/// tracks `DRM_DISPLAY_GENERATION` (bumped by the udev listener on a connector-topology change), so a
/// hotplug/modeset invalidates every cached buffer and forces a real fd; but the ledger still cannot
/// see the dma-buf inode, so a recycled fb_id within the SAME generation (identical geometry +
/// modifier) would elide onto a stale EGLImage. Enabling elision needs that inode case validated
/// first.
#[cfg(all(target_os = "linux", feature = "drm"))]
const DRM_FD_ELISION: bool = false;

#[cfg(all(target_os = "linux", feature = "drm"))]
struct SeenBuf {
    modifier: u64,
    dims: (u32, u32),
    epoch: u64,
}

#[cfg(all(target_os = "linux", feature = "drm"))]
struct ExportLedger {
    seen: HashMap<u32, SeenBuf>,
    order: std::collections::VecDeque<u32>, // insertion order, for evict-oldest
    epoch: u64,
}

#[cfg(all(target_os = "linux", feature = "drm"))]
impl ExportLedger {
    // Grow-once, hard-capped (preallocated model): a hostile/buggy peer or a fb_id churn cannot grow
    // this unbounded; oldest keys are evicted so a real fd is simply re-sent for them later.
    const MAX_LEDGER: usize = 32;

    fn new() -> Self {
        Self {
            seen: HashMap::new(),
            order: std::collections::VecDeque::new(),
            epoch: 0,
        }
    }

    /// Returns true if this frame's fd must be attached (new/changed/recycled buffer, caching
    /// disabled, or elision off), false if the converter already holds `fb_id` imported.
    fn should_send_fd(&mut self, desc: &DmabufDesc) -> bool {
        // fb_id == 0 disables caching for that frame; elision-off always sends.
        if !DRM_FD_ELISION || desc.fb_id == 0 {
            return true;
        }
        let ident = SeenBuf {
            modifier: desc.modifier,
            dims: (desc.width, desc.height),
            epoch: self.epoch,
        };
        if let Some(prev) = self.seen.get(&desc.fb_id) {
            if prev.modifier == ident.modifier
                && prev.dims == ident.dims
                && prev.epoch == ident.epoch
            {
                return false; // import-once cache hit: elide the fd
            }
        } else {
            // New key: record insertion order and evict the oldest if at capacity.
            if self.order.len() >= Self::MAX_LEDGER {
                if let Some(old) = self.order.pop_front() {
                    self.seen.remove(&old);
                }
            }
            self.order.push_back(desc.fb_id);
        }
        self.seen.insert(desc.fb_id, ident);
        true
    }
}

/// Build a [`DrmConn`] from an already-authorized `_drm` `Connection` (root `--service` side). The
/// parity `Connection` wraps a tokio `UnixStream` but exposes no way to move it out, so we `dup()`
/// its fd into a fresh, independently-owned tokio `UnixStream` for the bespoke SCM_RIGHTS framing.
/// A dup gives a NEW fd number, which registers as its own epoll entry in tokio's reactor (reusing
/// the same fd number would double-register); the caller drops the parity `Connection` afterwards,
/// closing ITS fd, while the dup keeps the socket alive via the shared open file description.
#[cfg(all(target_os = "linux", feature = "drm"))]
fn dup_to_drm_conn(stream: &Connection) -> ResultType<DrmConn> {
    let raw = stream.inner.get_ref().as_raw_fd();
    let dup = unsafe { hbb_common::libc::dup(raw) };
    if dup < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: `dup` is a freshly dup'd, owned fd for a connected SOCK_STREAM unix socket.
    let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(dup) };
    std_stream.set_nonblocking(true)?;
    let tokio_stream = tokio::net::UnixStream::from_std(std_stream)?;
    Ok(DrmConn::new(tokio_stream))
}

/// Cached DRM display enumeration. The pre-warm populates it and each capture open refreshes it, so
/// a consumer's handshake can send the display list without first paying a DRM enumeration open.
#[cfg(all(target_os = "linux", feature = "drm"))]
static DRM_DISPLAY_CACHE: std::sync::Mutex<Vec<DrmDisplayInfo>> = std::sync::Mutex::new(Vec::new());

/// Monotonic generation bumped by the udev DRM-uevent listener ONLY when a connector-topology change
/// actually altered `DRM_DISPLAY_CACHE` (a monitor hotplug/unplug/modeset). Each live `handle_drm_conn`
/// forward loop watches this (one atomic load per frame) and, on a bump, pushes a `DrmDisplaysChanged`
/// with the fresh list to its consumer — the cheap live-refresh path that avoids a consumer re-probe.
/// `Release`/`Acquire` order it after the cache write so a reader that sees the new generation also sees
/// the new cache (the cache `Mutex` re-synchronizes the contents regardless).
#[cfg(all(target_os = "linux", feature = "drm"))]
static DRM_DISPLAY_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Snapshot a reader's enumerated displays as the IPC `DrmDisplayInfo` form. `displays()` lists all
/// device outputs regardless of the reader's target CRTC, so a capture reader can refresh the cache.
#[cfg(all(target_os = "linux", feature = "drm"))]
fn drm_displays_from_reader(reader: &mut scrap::drm_reader::DrmReader) -> Vec<DrmDisplayInfo> {
    reader
        .displays()
        .into_iter()
        // Only offer outputs actually bound to a CRTC (i.e. scanning out). A
        // CONNECTED-but-unbound connector (e.g. a virtual/dummy HDMI plug the
        // compositor is not driving) enumerates with `crtc_id == 0`. Such an
        // entry has no scanout to capture, yet was still shipped to the client as
        // a selectable monitor; picking it made libdrmtap's `open(crtc=0)`
        // AUTO-SELECT the first active CRTC (the primary) and stream ITS frames at
        // the wrong geometry (e.g. a 3840x2160 frame into a 1280x1024 encoder ->
        // `src rect > dst rect`), which failed every frame and drove a ~1/sec
        // capturer restart loop (the flap that leaked EGL contexts to OOM). Drop
        // these here so they are never offered; the client keeps its real monitors.
        .filter(|d| d.active && d.crtc_id != 0)
        .map(|d| DrmDisplayInfo {
            name: d.name,
            crtc_id: d.crtc_id,
            x: d.x,
            y: d.y,
            width: d.width,
            height: d.height,
            active: d.active,
        })
        .collect()
}

/// True if a kernel uevent datagram is a DRM-subsystem topology change (a connector hotplug/modeset).
/// A uevent is NUL-separated `KEY=value` records; we require `SUBSYSTEM=drm` plus a `change` action or
/// `HOTPLUG=1`, so an `add`/`remove` of an unrelated node (a render device, a fb) does not trigger a
/// re-enumeration. Byte-exact record matching avoids any allocation/UTF-8 handling on the hot recv path.
#[cfg(all(target_os = "linux", feature = "drm"))]
fn uevent_is_drm_change(msg: &[u8]) -> bool {
    let mut is_drm = false;
    let mut is_change = false;
    for rec in msg.split(|&b| b == 0) {
        if rec == b"SUBSYSTEM=drm" {
            is_drm = true;
        } else if rec == b"ACTION=change" || rec == b"HOTPLUG=1" {
            is_change = true;
        }
    }
    is_drm && is_change
}

/// Listen for DRM connector hotplug/modeset uevents and refresh the display cache when the topology
/// actually changes. Uses a raw `NETLINK_KOBJECT_UEVENT` socket (the same hotplug stream udev consumes)
/// so no libudev dependency is added; the root `--service` already runs privileged and joining the
/// kernel-uevent multicast group needs no extra cap. On a real change it re-enumerates (off any hot
/// path — this is a dedicated thread, so the blocking `open`/`displays` is fine), and only when the
/// enumerated set differs does it swap `DRM_DISPLAY_CACHE` and bump `DRM_DISPLAY_GENERATION`; live
/// `handle_drm_conn` loops then push the fresh list to their consumers. Best-effort: if the socket is
/// unavailable it logs and returns, and DRM capture still works (a consumer reconnect re-reads the
/// fresh list) — just without the mid-session live refresh.
#[cfg(all(target_os = "linux", feature = "drm"))]
fn drm_udev_listener() {
    use hbb_common::libc;
    use std::sync::atomic::Ordering;

    let sock = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            libc::NETLINK_KOBJECT_UEVENT,
        )
    };
    if sock < 0 {
        log::info!(
            "drm: udev uevent socket unavailable ({}); hotplug refresh disabled",
            std::io::Error::last_os_error()
        );
        return;
    }
    // Own the fd so it is closed on every return / unwind path.
    let _owned = unsafe { OwnedFd::from_raw_fd(sock) };
    let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    addr.nl_family = libc::AF_NETLINK as u16;
    // Group 1 = kernel-originated uevents (udev re-broadcasts on group 2); pid 0 => kernel assigns.
    addr.nl_groups = 1;
    let rc = unsafe {
        libc::bind(
            sock,
            &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if rc < 0 {
        log::info!(
            "drm: udev uevent bind failed ({}); hotplug refresh disabled",
            std::io::Error::last_os_error()
        );
        return;
    }
    log::info!("drm: udev DRM-uevent listener started");
    // Fixed-size receive buffer (preallocated model): a uevent is well under 8 KiB; a rare larger
    // datagram is truncated by `recv` and simply re-enumerates on the next matching event.
    let mut buf = [0u8; 8192];
    loop {
        // recvmsg (not recv) so the source address is available: bound to the kernel-uevent multicast
        // group, a genuine uevent comes from the kernel (source nl_pid == 0) via a multicast group
        // (nl_groups != 0). A local unprivileged process could otherwise UNICAST a spoofed
        // "change@.../drm/..." datagram to this root listener and drive it to re-enumerate at will;
        // dropping any non-kernel/non-multicast source closes that.
        let mut src: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        };
        let mut mhdr: libc::msghdr = unsafe { std::mem::zeroed() };
        mhdr.msg_name = &mut src as *mut libc::sockaddr_nl as *mut libc::c_void;
        mhdr.msg_namelen = std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        mhdr.msg_iov = &mut iov;
        mhdr.msg_iovlen = 1;
        let n = unsafe { libc::recvmsg(sock, &mut mhdr, 0) };
        if n <= 0 {
            let err = std::io::Error::last_os_error();
            if n < 0 && err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            log::info!("drm: udev uevent recv ended ({err}); hotplug refresh stopped");
            break;
        }
        // Trust only a kernel-originated (nl_pid == 0), multicast-delivered (nl_groups != 0) datagram
        // with a full source address; drop a unicast or user-spoofed message.
        if (mhdr.msg_namelen as usize) < std::mem::size_of::<libc::sockaddr_nl>()
            || src.nl_pid != 0
            || src.nl_groups == 0
        {
            continue;
        }
        if !uevent_is_drm_change(&buf[..n as usize]) {
            continue;
        }
        // Re-enumerate and diff. Only a real change swaps the cache + bumps the generation, so a
        // uevent that does not alter the captured topology stays silent (no consumer churn).
        if let Some(mut r) = scrap::drm_reader::DrmReader::open(None, 0) {
            let fresh = drm_displays_from_reader(&mut r);
            let changed = {
                let mut cache = DRM_DISPLAY_CACHE.lock().unwrap();
                if *cache != fresh {
                    *cache = fresh;
                    true
                } else {
                    false
                }
            };
            if changed {
                DRM_DISPLAY_GENERATION.fetch_add(1, Ordering::Release);
                log::info!("drm: connector topology changed (udev); display cache refreshed");
            }
        }
    }
}

/// Best-effort warm-up at listener start: loads libdrmtap, initializes EGL, enumerates displays into
/// the cache, and maps the first framebuffer once. Moves that one-time cost (which otherwise lands
/// on the first consumer and can push the first frame past the client's initial-frame timeout) off
/// the critical path. Runs on its own thread since `DrmReader` is `!Send` and `open`/`grab` block.
#[cfg(all(target_os = "linux", feature = "drm"))]
fn drm_prewarm() {
    let t = std::time::Instant::now();
    match scrap::drm_reader::DrmReader::open(None, 0) {
        Some(mut r) => {
            let displays = drm_displays_from_reader(&mut r);
            let n = displays.len();
            // Warm the first framebuffer export. On the split path, grab_desc() exports a dma-buf fd
            // WITHOUT loading libEGL/libGLESv2 into the root service (the convert now runs in the
            // unprivileged --server); only an old .so (no grab_desc) still force-maps via grab().
            if r.supports_grab_desc() {
                if let Ok((fd, _desc)) = r.grab_desc() {
                    drop(fd); // close the warm-up fd; we only wanted to prime the device/import path
                }
            } else {
                let _ = r.grab();
            }
            *DRM_DISPLAY_CACHE.lock().unwrap() = displays;
            log::info!("drm: pre-warm ok ({n} displays) in {:?}", t.elapsed());
        }
        None => log::info!("drm: pre-warm skipped (reader unavailable)"),
    }
}

/// DRM/KMS capture producer. Runs in the ROOT `--service` (which holds CAP_SYS_ADMIN, so libdrmtap
/// reads the scanout in-process — no helper, no setcap). One dedicated `current_thread` runtime
/// owns the `_drm` listener and `tokio::spawn`s a task per accepted consumer, so a multi-monitor
/// client (which opens one `_drm` connection per captured display) is served CONCURRENTLY instead
/// of serially. The `!Send` `DrmReader` never runs on this runtime: each connection offloads its
/// blocking `grab()` loop to a private std worker thread (see `handle_drm_conn`), which keeps the
/// connection future `Send` (thus spawnable) and lets the tasks multiplex on the one listener
/// thread while the workers capture in parallel.
#[cfg(all(target_os = "linux", feature = "drm"))]
#[tokio::main(flavor = "current_thread")]
pub async fn start_drm() {
    match new_drm_listener().await {
        Ok(mut incoming) => {
            // Warm libdrmtap/EGL + enumeration off-thread so the first consumer does not pay that
            // one-time cost on its critical path.
            std::thread::spawn(drm_prewarm);
            // Watch for connector hotplug/modeset uevents so a mid-session topology change refreshes
            // the display cache and is pushed to live consumers (best-effort; own thread since it
            // blocks on recv and re-enumeration is a blocking `!Send` open).
            std::thread::spawn(drm_udev_listener);
            loop {
                match incoming.next().await {
                    Some(Ok(stream)) => {
                        tokio::spawn(async move {
                            if let Err(err) = handle_drm_conn(Connection::new(stream)).await {
                                log::info!("drm ipc connection ended: {}", err);
                            }
                        });
                    }
                    Some(Err(err)) => log::error!("Couldn't get drm client: {:?}", err),
                    // Stream exhausted: without this the `if let Some` form would re-poll the dead
                    // stream forever and busy-spin the root service. Stop the producer instead.
                    None => {
                        log::error!("drm ipc listener stream ended; stopping drm producer");
                        break;
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start drm ipc server: {}", err);
        }
    }
}

/// Handle one `_drm` consumer. `DrmReader` is `!Send` and `grab()` is a blocking C call, so it
/// cannot live on the shared listener runtime; this task spawns a private std worker thread that
/// owns the reader (`drm_capture_worker`) and streams `DrmProducerMsg`s back over a bounded channel
/// (capacity 2 = backpressure: a slow consumer throttles capture instead of growing memory). The
/// task itself stays fully async — hence `Send`, hence `tokio::spawn`able — and only forwards
/// messages to the wire. On any error / disconnect it returns; the `DrmStopGuard` plus dropping the
/// channels tears the worker down, and the client falls back to PipeWire/portal.
#[cfg(all(target_os = "linux", feature = "drm"))]
async fn handle_drm_conn(stream: Connection) -> ResultType<()> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    // The `_drm` socket is world-connectable (0666) so the unprivileged `--server` can reach it,
    // so we MUST authorize the peer here — this is a dedicated listener that does not go through
    // the generic `start()` accept loop where service-scoped channels are checked. Same policy as
    // `_service`: peer must be root or the active session uid, with a `/proc/pid/exe` identity
    // match. Without this any local process could connect and receive the screen contents.
    if !authorize_service_scoped_ipc_connection(&stream, "_drm") {
        log::warn!("drm: rejected unauthorized connection to _drm");
        return Ok(());
    }

    // Admission bound: each accepted _drm consumer spawns a worker thread that opens a DRM context.
    // The peer is authorized (root/active-session), but we still cap concurrency so a buggy or
    // compromised --server cannot exhaust root-service threads/memory by opening an unbounded number
    // of streams. One connection per served display is plenty; MAX_DRM_CONNS covers multi-monitor
    // plus a little slack for a reconnect overlapping an old worker still tearing down.
    const MAX_DRM_CONNS: usize = 8;
    static DRM_CONN_COUNT: AtomicUsize = AtomicUsize::new(0);
    struct DrmConnGuard;
    impl Drop for DrmConnGuard {
        fn drop(&mut self) {
            DRM_CONN_COUNT.fetch_sub(1, Ordering::SeqCst);
        }
    }
    if DRM_CONN_COUNT.fetch_add(1, Ordering::SeqCst) >= MAX_DRM_CONNS {
        DRM_CONN_COUNT.fetch_sub(1, Ordering::SeqCst);
        log::warn!("drm: too many concurrent _drm connections (>= {MAX_DRM_CONNS}); rejecting");
        return Ok(());
    }
    let _conn_guard = DrmConnGuard;

    // Capture the peer uid now so the forward loop can RE-authorize every frame. The check above runs
    // once at accept, but DRM/KMS capture is NOT session-scoped: `drm_capture_worker` grabs the
    // physical scanout of a CRTC regardless of which session currently owns the display. So a stream
    // authorized for one session must stop the moment the active session changes, or the outgoing
    // user's --server keeps receiving the incoming user's screen (and the greeter in between) until
    // the socket dies (review 3.3). `peer_uid` is the --server's fixed uid.
    let peer_uid = stream.peer_uid();

    // Move the authorized `_drm` stream onto the bespoke SCM_RIGHTS framing (see `DrmConn`). ALL
    // further traffic — display list, `DrmStart`, frame descriptors + their ancillary fd, and the
    // cursor / CPU-fallback bodies — goes through `conn` so no `Framed` read buffer ever competes with
    // a `recvmsg` for the fd. The parity `Connection` (used only for the authorization above) is
    // dropped here, closing its fd; the dup inside `conn` keeps the socket alive.
    let mut conn = dup_to_drm_conn(&stream)?;
    drop(stream);

    // worker -> task: display list, frames, cursor (bounded = backpressure).
    let (frame_tx, mut frame_rx) = tokio::sync::mpsc::channel::<DrmProducerMsg>(2);
    // task -> worker: the chosen CRTC + whether the consumer needs the CPU path, sent once after the
    // client's DrmStart.
    let (crtc_tx, crtc_rx) = std::sync::mpsc::channel::<(u32, bool)>();
    let stop = Arc::new(AtomicBool::new(false));
    let _stop_guard = DrmStopGuard(stop.clone());
    let worker_stop = stop.clone();
    std::thread::spawn(move || drm_capture_worker(frame_tx, crtc_rx, worker_stop));

    // Handshake: the worker sends the display list (from the pre-warmed cache, or a throwaway
    // enumeration open if the cache is empty). A closed channel (no Displays) means the reader was
    // unavailable, so let the client fall back.
    let displays = match frame_rx.recv().await {
        Some(DrmProducerMsg::Displays(d)) => d,
        _ => {
            log::info!("drm: reader unavailable; closing _drm connection (client falls back)");
            return Ok(());
        }
    };
    conn.send_msg(&Data::DrmDisplayList(displays.clone()), None).await?;

    // Wait for the client to choose a display before streaming. `recv_msg_timeout2` gates only the
    // wait for the first byte, so a timeout leaves the stream at a clean frame boundary.
    let (display_idx, need_cpu) = loop {
        match conn.recv_msg_timeout2(10_000).await {
            Some(Ok((Data::DrmStart { display, need_cpu }, _fd))) => break (display, need_cpu),
            Some(Ok((_, _fd))) => continue, // ignore unexpected messages; drop any stray fd
            Some(Err(e)) => return Err(e),
            None => return Ok(()), // timed out: client never chose a display
        }
    };
    // Resolve the chosen display's CRTC. `displays` here is already filtered to
    // CRTC-bound outputs (see drm_displays_from_reader), so a valid selection
    // always yields a non-zero crtc_id. Reject a 0 (out-of-range index, or an
    // unbound display that somehow slipped through) rather than passing it to
    // `open(crtc=0)`, whose "auto-select the first/primary CRTC" sentinel would
    // silently stream the WRONG monitor at a mismatched geometry and flap the
    // capturer. Closing lets the consumer fall back (PipeWire) for that display.
    let target_crtc = usize::try_from(display_idx)
        .ok()
        .and_then(|i| displays.get(i))
        .map(|d| d.crtc_id)
        .unwrap_or(0);
    if target_crtc == 0 {
        log::warn!(
            "drm: client selected display {display_idx} with no bound CRTC; closing _drm (client falls back)"
        );
        return Ok(());
    }
    // Hand the CRTC + the consumer's CPU-path request to the worker; an error means it already gave up
    // (reader vanished).
    if crtc_tx.send((target_crtc, need_cpu)).is_err() {
        return Ok(());
    }

    // Forward frames + cursor updates until the worker ends or the client disconnects (a wire send
    // error on a dropped client propagates out and tears the worker down via the guard). The
    // per-connection `ExportLedger` decides, for the zero-copy path, whether each frame's fd must ride
    // an SCM_RIGHTS cmsg or can be elided as an import-once cache hit.
    let mut ledger = ExportLedger::new();
    // Live hotplug: the udev listener bumps DRM_DISPLAY_GENERATION when the connector topology changes.
    // Seed from the value current at handshake (the list already sent reflects it) and, whenever it
    // moves, push the fresh list to this consumer. Piggybacked on the frame cadence so it costs only one
    // atomic load per frame; a genuinely idle stream tears down after MAX_STALLED and the consumer
    // reconnects to a fresh list anyway.
    let mut seen_gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
    while let Some(first) = frame_rx.recv().await {
        // Re-authorize per frame (review 3.3): root (0) is always allowed; any other peer must still
        // be the active-session uid. On a session change the outgoing --server no longer matches, so
        // we stop within one frame (~33ms) instead of streaming the new session's screen to it. Fail
        // closed if the peer uid cannot be determined.
        match peer_uid {
            Some(0) => {}
            Some(uid) if Some(uid) == active_uid() => {}
            _ => {
                log::warn!("drm: _drm peer no longer matches the active session; closing");
                break;
            }
        }
        let gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
        // Keep the ledger's epoch at the live generation so a hotplug/modeset (which may recycle an
        // fb_id onto a new buffer) invalidates every cached buffer and forces a real fd on the next
        // frame. Cheap (one field write) and only observable when DRM_FD_ELISION is enabled.
        ledger.epoch = gen;
        if gen != seen_gen {
            seen_gen = gen;
            let fresh = DRM_DISPLAY_CACHE.lock().unwrap().clone();
            if !fresh.is_empty() {
                conn.send_msg(&Data::DrmDisplaysChanged(fresh), None).await?;
            }
        }
        // Coalesce to latest-wins at the source (review 4.8). The `_drm` socket is a FIFO, so a
        // consumer that drains slower than we produce (a 4K convert on a modest GPU) would fall
        // seconds behind stale frames. Drain everything already queued without blocking and forward
        // only the NEWEST frame; each replaced frame drops here, closing its OwnedFd (zero-copy path)
        // and freeing its pixel buffer (CPU path). Cursor updates are latency-insensitive state
        // (latest-wins by id downstream), so they are forwarded in order and never coalesced away.
        let mut latest_frame: Option<DrmProducerMsg> = None;
        let mut msg = Some(first);
        while let Some(m) = msg.take() {
            match m {
                f @ (DrmProducerMsg::Frame { .. } | DrmProducerMsg::FrameCpu { .. }) => {
                    latest_frame = Some(f);
                }
                DrmProducerMsg::Cursor {
                    id,
                    width,
                    height,
                    hotx,
                    hoty,
                    colors,
                } => {
                    conn.send_msg(
                        &Data::DrmCursor {
                            id,
                            width,
                            height,
                            hotx,
                            hoty,
                        },
                        None,
                    )
                    .await?;
                    conn.send_raw(Bytes::from(colors)).await?;
                }
                DrmProducerMsg::Displays(_) => {}
            }
            msg = frame_rx.try_recv().ok();
        }
        match latest_frame {
            Some(DrmProducerMsg::Frame { mut desc, fd }) => {
                // The worker always supplies a real fd; the ledger decides whether to attach it.
                let send_fd = fd.is_some() && ledger.should_send_fd(&desc);
                desc.has_fd = send_fd;
                let borrowed = if send_fd { fd.as_ref().map(|f| f.as_fd()) } else { None };
                conn.send_msg(&Data::DrmFrameDmabuf(desc), borrowed).await?;
                // `fd` (OwnedFd) is closed here whether or not it was attached (the cmsg dup'd it into
                // the peer). Closing immediately bounds our fd usage to ~1 in flight per frame.
            }
            Some(DrmProducerMsg::FrameCpu {
                width,
                height,
                data,
            }) => {
                // CPU-mapped fallback: pixels cross the wire, exactly like the pre-split protocol.
                conn.send_msg(&Data::DrmFrame { width, height }, None).await?;
                conn.send_raw(data).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// The blocking half of a `_drm` connection: owns the `!Send` `DrmReader`(s) on its own thread and
/// streams messages to the async task. Ends (thread exits, reader closes) when the device is
/// unavailable, errors/stalls, or the task drops the channels / sets the stop flag.
#[cfg(all(target_os = "linux", feature = "drm"))]
fn drm_capture_worker(
    frame_tx: tokio::sync::mpsc::Sender<DrmProducerMsg>,
    crtc_rx: std::sync::mpsc::Receiver<(u32, bool)>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    // ~30 fps producer ceiling; the consumer's encoder/QoS sets the effective rate and the bounded
    // channel throttles us further if it is slower. Also avoids a busy-spin when `grab()` returns
    // the same scanout repeatedly.
    const FRAME_INTERVAL: Duration = Duration::from_millis(33);
    // Bound continuous no-frame (WouldBlock) time so a wedged device ends the stream (~5s) instead
    // of freezing forever; the client then falls back.
    const MAX_STALLED: u32 = 150;

    let t_conn = std::time::Instant::now();

    // Send the display list. Prefer the pre-warmed cache (skips a per-connection enumeration open);
    // fall back to a throwaway enumeration reader if the pre-warm has not populated it yet.
    let displays = {
        let cached = DRM_DISPLAY_CACHE.lock().unwrap().clone();
        if !cached.is_empty() {
            cached
        } else {
            let mut enum_reader = match scrap::drm_reader::DrmReader::open(None, 0) {
                Some(r) => r,
                None => return,
            };
            drm_displays_from_reader(&mut enum_reader)
        }
    };
    if frame_tx
        .blocking_send(DrmProducerMsg::Displays(displays))
        .is_err()
    {
        return;
    }

    // Wait for the task to relay the client's chosen CRTC + CPU-path request (Err => the task gave up
    // / disconnected).
    let (target_crtc, need_cpu) = match crtc_rx.recv() {
        Ok(c) => c,
        Err(_) => return,
    };
    let t_open = std::time::Instant::now();
    let mut reader = match scrap::drm_reader::DrmReader::open(None, target_crtc) {
        Some(r) => r,
        None => {
            log::warn!("drm: failed to open crtc {target_crtc}; closing _drm connection");
            // The cached display list handed out a CRTC that no longer opens (a hotplug/modeset
            // likely invalidated it). Drop the cache so the next connection re-enumerates from the
            // live device instead of serving the same stale, unopenable CRTC on every reconnect.
            DRM_DISPLAY_CACHE.lock().unwrap().clear();
            return;
        }
    };
    // Refresh the cache from the live device so the next consumer's handshake uses fresh geometry.
    *DRM_DISPLAY_CACHE.lock().unwrap() = drm_displays_from_reader(&mut reader);
    log::debug!(
        "drm: capture reader for crtc {target_crtc} opened in {:?}",
        t_open.elapsed()
    );

    // A per-connection buffer-pool epoch so `buffer_id` is unique across connections even for the same
    // fb_id (the consumer may key a pool by buffer_id).
    static DRM_CONN_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let conn_epoch = DRM_CONN_EPOCH.fetch_add(1, Ordering::Relaxed);

    // Prefer the zero-copy split export (root does NO EGL / convert / copy). Fall back to the
    // CPU-mapped path for this connection (pixels cross the wire) when: the loaded libdrmtap predates
    // the split API, grab_desc later reports ENOTSUP (no transferable dma-buf on this seat), OR the
    // consumer asked for the CPU path because it has no render-node convert context (need_cpu) — in
    // that last case the dma-buf fd would be useless to it and the stream would be lost.
    let mut use_dmabuf = reader.supports_grab_desc() && !need_cpu;

    let mut last_cursor_id: u64 = 0;
    let mut stalled: u32 = 0;
    let mut logged_first = false;
    while !stop.load(Ordering::Relaxed) {
        // Grab one frame in the current mode, producing an OWNED message (no borrow of `reader`
        // outlives this, so `reader.cursor()` below is free to run). The dma-buf path ships only the
        // descriptor + fd; the CPU path copies the packed BGRA once (Bytes::copy_from_slice).
        let grabbed: std::io::Result<DrmProducerMsg> = if use_dmabuf {
            match reader.grab_desc() {
                Ok((fd, d)) => Ok(DrmProducerMsg::Frame {
                    desc: DmabufDesc {
                        buffer_id: (d.fb_id as u64) | ((conn_epoch as u64) << 32),
                        width: d.width,
                        height: d.height,
                        format: d.format,
                        modifier: d.modifier,
                        fb_id: d.fb_id,
                        num_planes: d.num_planes,
                        offsets: d.offsets,
                        pitches: d.pitches,
                        hdr_eotf: d.hdr_eotf,
                        hdr_max_nits: d.hdr_max_nits,
                        has_fd: true, // the async task's ExportLedger may downgrade this
                    },
                    fd: Some(fd),
                }),
                Err(err) => Err(err),
            }
        } else {
            match reader.grab() {
                Ok((buf, w, h)) => Ok(DrmProducerMsg::FrameCpu {
                    width: w as u32,
                    height: h as u32,
                    data: Bytes::copy_from_slice(buf),
                }),
                Err(err) => Err(err),
            }
        };
        match grabbed {
            Ok(msg) => {
                stalled = 0;
                if !logged_first {
                    logged_first = true;
                    log::debug!(
                        "drm: first frame for crtc {target_crtc} in {:?} ({} path)",
                        t_conn.elapsed(),
                        if use_dmabuf { "dma-buf" } else { "cpu" }
                    );
                }
                if frame_tx.blocking_send(msg).is_err() {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                stalled += 1;
                if stalled > MAX_STALLED {
                    log::info!("drm: capture stalled (no frame); closing _drm connection");
                    break;
                }
                std::thread::sleep(FRAME_INTERVAL);
                continue;
            }
            Err(err) if use_dmabuf && err.kind() == std::io::ErrorKind::Unsupported => {
                // The split export cannot work on this seat/driver (ENOTSUP). Switch this connection
                // to the CPU-mapped fallback (pixels over the wire) instead of tearing down or
                // rebuild-looping; the reader is already open and usable via grab().
                log::warn!(
                    "drm: grab_desc unsupported ({err}); switching to CPU-mapped fallback for this connection"
                );
                use_dmabuf = false;
                logged_first = false;
                continue;
            }
            Err(err) => {
                log::warn!("drm: capture error: {err}; closing _drm connection");
                break;
            }
        }

        // Ship the cursor shape only when it changes (id is a content hash or the hidden sentinel).
        if let Some(c) = reader.cursor() {
            if c.id != last_cursor_id {
                last_cursor_id = c.id;
                if frame_tx
                    .blocking_send(DrmProducerMsg::Cursor {
                        id: c.id,
                        width: c.width,
                        height: c.height,
                        hotx: c.hotx,
                        hoty: c.hoty,
                        colors: c.colors,
                    })
                    .is_err()
                {
                    break;
                }
            }
        }

        std::thread::sleep(FRAME_INTERVAL);
    }
}

pub struct ConnectionTmpl<T> {
    inner: Framed<T, BytesCodec>,
}

pub type Connection = ConnectionTmpl<Conn>;

impl<T> ConnectionTmpl<T>
where
    T: AsyncRead + AsyncWrite + std::marker::Unpin,
{
    pub fn new(conn: T) -> Self {
        Self {
            inner: Framed::new(conn, BytesCodec::new()),
        }
    }

    pub async fn send(&mut self, data: &Data) -> ResultType<()> {
        let v = serde_json::to_vec(data)?;
        self.inner.send(bytes::Bytes::from(v)).await?;
        Ok(())
    }

    async fn send_config(&mut self, name: &str, value: String) -> ResultType<()> {
        self.send(&Data::Config((name.to_owned(), Some(value))))
            .await
    }

    pub async fn next_timeout(&mut self, ms_timeout: u64) -> ResultType<Option<Data>> {
        Ok(timeout(ms_timeout, self.next()).await??)
    }

    pub async fn next_timeout2(&mut self, ms_timeout: u64) -> Option<ResultType<Option<Data>>> {
        if let Ok(x) = timeout(ms_timeout, self.next()).await {
            Some(x)
        } else {
            None
        }
    }

    pub async fn next(&mut self) -> ResultType<Option<Data>> {
        match self.inner.next().await {
            Some(res) => {
                let bytes = res?;
                if let Ok(s) = std::str::from_utf8(&bytes) {
                    if let Ok(data) = serde_json::from_str::<Data>(s) {
                        return Ok(Some(data));
                    }
                }
                return Ok(None);
            }
            _ => {
                bail!("reset by the peer");
            }
        }
    }

    pub async fn send_raw(&mut self, data: Bytes) -> ResultType<()> {
        self.inner.send(data).await?;
        Ok(())
    }

    pub async fn next_raw(&mut self) -> ResultType<bytes::BytesMut> {
        match self.inner.next().await {
            Some(Ok(res)) => Ok(res),
            _ => {
                bail!("reset by the peer");
            }
        }
    }
}

/// Ancillary-fd transport for the `_drm` channel.
///
/// `ConnectionTmpl`'s `Framed<_, BytesCodec>` cannot carry (nor collect) an SCM_RIGHTS control
/// message: tokio's `AsyncRead` never does a `recvmsg` with a control buffer, so a fd sent alongside
/// a `Framed` byte-frame is silently dropped on receive, and interleaving a raw `sendmsg` with the
/// codec desyncs its internal read buffer. So the WHOLE `_drm` channel moves onto this bespoke
/// length-prefixed `sendmsg`/`recvmsg` framing, owning the raw `tokio::net::UnixStream` directly:
/// handshake (`DrmDisplayList`/`DrmStart`), frame descriptors, and the CPU-fallback/cursor bodies all
/// go through it so no `Framed` read buffer ever competes with a `recvmsg`.
///
/// Framing: each frame is a 4-byte big-endian length prefix + payload. `send_msg`/`recv_msg` carry a
/// JSON `Data`; `send_raw`/`next_raw` carry an opaque body. The dma-buf fd (when present) rides an
/// SCM_RIGHTS cmsg bound to the frame's first (prefix) byte, so reading the prefix with a control
/// buffer reliably collects it (`MSG_CTRUNC` is rejected). Reads use exact-length loops so they never
/// cross a frame boundary and thus never discard a following frame's ancillary fd.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub struct DrmConn {
    /// The raw stream. Obtained from `connect_drm` (client) or the accepted `_drm` listener stream
    /// (service). All framing is done by hand on this fd; there is no `Framed` codec.
    stream: tokio::net::UnixStream,
    /// Grow-once accumulation buffer for `recv_msg`/`next_raw` length-prefixed reads (preallocated
    /// model: it grows to the largest frame seen and is then reused, never per-frame reallocated).
    read_buf: Vec<u8>,
}

/// Cap on a JSON `Data` message read by `recv_msg` (headers/handshake are tiny; this only bounds a
/// hostile/oversized length prefix). Distinct from the raw-body cap because a body can be a whole
/// CPU-fallback frame.
#[cfg(all(target_os = "linux", feature = "drm"))]
const MAX_DRM_JSON_BYTES: usize = 8 * 1024 * 1024;
/// Cap on a raw body read by `next_raw` (CPU-fallback BGRA / cursor RGBA). Covers a 256 MiB 8K
/// scanout (`DrmReader` bounds a frame to that) with margin.
#[cfg(all(target_os = "linux", feature = "drm"))]
const MAX_DRM_RAW_BYTES: usize = 512 * 1024 * 1024;
/// Control-buffer capacity for one SCM_RIGHTS cmsg carrying a single fd. `CMSG_SPACE(sizeof(int))` is
/// 24 bytes on our targets; 64 gives headroom and the `align(8)` matches `cmsghdr` alignment.
#[cfg(all(target_os = "linux", feature = "drm"))]
const DRM_CMSG_CAP: usize = 64;

/// Aligned storage for the SCM_RIGHTS control buffer (`msg_control` must be `cmsghdr`-aligned).
#[cfg(all(target_os = "linux", feature = "drm"))]
#[repr(align(8))]
struct DrmCmsgBuf([u8; DRM_CMSG_CAP]);

/// One non-blocking `sendmsg`: writes `buf` and, when `pass_fd` is `Some`, attaches exactly one
/// SCM_RIGHTS cmsg carrying that fd. The cmsg is attached ONLY when a fd is present (a -1 fd in an
/// SCM_RIGHTS cmsg fails the whole call). Returns bytes sent, or a `WouldBlock`/other io error.
///
/// SAFETY: `fd` must be a valid open socket fd; `buf` a valid readable slice; `pass_fd` (if any) a
/// valid open fd. The ancillary data is delivered by the kernel with the first byte of `buf`.
#[cfg(all(target_os = "linux", feature = "drm"))]
unsafe fn drm_sendmsg(fd: RawFd, buf: &[u8], pass_fd: Option<RawFd>) -> std::io::Result<usize> {
    use hbb_common::libc;
    let mut iov = libc::iovec {
        iov_base: buf.as_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    let mut msg: libc::msghdr = std::mem::zeroed();
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    let mut cbuf = DrmCmsgBuf([0u8; DRM_CMSG_CAP]);
    if let Some(sfd) = pass_fd {
        msg.msg_control = cbuf.0.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = libc::CMSG_SPACE(std::mem::size_of::<libc::c_int>() as u32) as _;
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        // Sized above so CMSG_FIRSTHDR is non-null; guard anyway to avoid UB on any platform quirk.
        if cmsg.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "drm: CMSG_FIRSTHDR null",
            ));
        }
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<libc::c_int>() as u32) as _;
        let sfd_c: libc::c_int = sfd;
        std::ptr::copy_nonoverlapping(
            &sfd_c as *const libc::c_int as *const u8,
            libc::CMSG_DATA(cmsg),
            std::mem::size_of::<libc::c_int>(),
        );
    }
    let n = libc::sendmsg(fd, &msg, libc::MSG_NOSIGNAL);
    if n < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(n as usize)
    }
}

/// One non-blocking `recvmsg` into `buf` with a control buffer. Collects at most one SCM_RIGHTS fd
/// (any surplus fds are closed); rejects a truncated cmsg (`MSG_CTRUNC`) as a hard error after closing
/// whatever it parsed. Returns (bytes read, fd). Received fds are `O_CLOEXEC` (`MSG_CMSG_CLOEXEC`).
///
/// SAFETY: `fd` must be a valid open socket fd; `buf` a valid writable slice.
#[cfg(all(target_os = "linux", feature = "drm"))]
unsafe fn drm_recvmsg(fd: RawFd, buf: &mut [u8]) -> std::io::Result<(usize, Option<OwnedFd>)> {
    use hbb_common::libc;
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    let mut cbuf = DrmCmsgBuf([0u8; DRM_CMSG_CAP]);
    let mut msg: libc::msghdr = std::mem::zeroed();
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cbuf.0.as_mut_ptr() as *mut libc::c_void;
    msg.msg_controllen = cbuf.0.len() as _;
    let n = libc::recvmsg(fd, &mut msg, libc::MSG_CMSG_CLOEXEC);
    if n < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Walk the cmsgs; keep the first SCM_RIGHTS fd, close any extras. Each parsed int is wrapped in an
    // OwnedFd immediately so it is always closed on drop (no fd leak on any error path below).
    let mut got: Option<OwnedFd> = None;
    let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
    while !cmsg.is_null() {
        if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
            let data = libc::CMSG_DATA(cmsg);
            let hdr = libc::CMSG_LEN(0) as usize;
            let payload = ((*cmsg).cmsg_len as usize).saturating_sub(hdr);
            let count = payload / std::mem::size_of::<libc::c_int>();
            for i in 0..count {
                let mut rawfd: libc::c_int = -1;
                std::ptr::copy_nonoverlapping(
                    data.add(i * std::mem::size_of::<libc::c_int>()),
                    &mut rawfd as *mut libc::c_int as *mut u8,
                    std::mem::size_of::<libc::c_int>(),
                );
                if rawfd >= 0 {
                    let owned = OwnedFd::from_raw_fd(rawfd);
                    if got.is_none() {
                        got = Some(owned);
                    } // else: surplus fd, dropped here -> closed
                }
            }
        }
        cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
    }
    // A truncated control message means the kernel dropped fd(s) that did not fit: fail rather than
    // proceed with a missing/partial fd (drop `got` so anything parsed is closed first).
    if msg.msg_flags & libc::MSG_CTRUNC != 0 {
        drop(got);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "drm: truncated SCM_RIGHTS control message (MSG_CTRUNC)",
        ));
    }
    Ok((n as usize, got))
}

/// Write all of `buf` to `stream`, attaching `pass_fd` (if any) to the FIRST byte (the kernel binds
/// SCM_RIGHTS ancillary to the first data byte of the `sendmsg` that carried it). Loops on
/// `WouldBlock` via `writable()`; the fd is attached only until the first `sendmsg` sends >= 1 byte.
#[cfg(all(target_os = "linux", feature = "drm"))]
async fn drm_write_all(
    stream: &tokio::net::UnixStream,
    mut buf: &[u8],
    mut pass_fd: Option<RawFd>,
) -> ResultType<()> {
    while !buf.is_empty() {
        stream.writable().await?;
        let raw = stream.as_raw_fd();
        let chunk = buf;
        let fd_now = pass_fd;
        match stream.try_io(tokio::io::Interest::WRITABLE, || unsafe {
            drm_sendmsg(raw, chunk, fd_now)
        }) {
            Ok(0) => bail!("drm: socket write returned 0 (peer closed)"),
            Ok(n) => {
                pass_fd = None; // ancillary delivered with these bytes; do not re-send it
                buf = &buf[n..];
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Write one length-prefixed frame: a 4-byte big-endian length + payload, with `pass_fd` (if any)
/// riding the prefix's first byte.
#[cfg(all(target_os = "linux", feature = "drm"))]
async fn drm_send_frame(
    stream: &tokio::net::UnixStream,
    payload: &[u8],
    pass_fd: Option<RawFd>,
) -> ResultType<()> {
    if payload.len() > u32::MAX as usize {
        bail!("drm: frame too large ({} bytes)", payload.len());
    }
    let prefix = (payload.len() as u32).to_be_bytes();
    // The fd rides the prefix (its first byte); the payload carries no ancillary.
    drm_write_all(stream, &prefix, pass_fd).await?;
    drm_write_all(stream, payload, None).await?;
    Ok(())
}

/// Read exactly `buf.len()` bytes from `stream`. When `want_cmsg` is true, the FIRST read uses a
/// control buffer to collect an SCM_RIGHTS fd (which the sender bound to the frame's first byte);
/// subsequent reads within the same frame are plain. Returns the collected fd, if any.
#[cfg(all(target_os = "linux", feature = "drm"))]
async fn drm_read_full(
    stream: &tokio::net::UnixStream,
    buf: &mut [u8],
    want_cmsg: bool,
) -> ResultType<Option<OwnedFd>> {
    use hbb_common::libc;
    let mut off = 0usize;
    let mut got: Option<OwnedFd> = None;
    while off < buf.len() {
        stream.readable().await?;
        let raw = stream.as_raw_fd();
        // Only the first read of a frame carries the fd (bound to byte 0); after that, plain reads.
        let use_cmsg = want_cmsg && got.is_none();
        let n = {
            let dst: &mut [u8] = &mut buf[off..];
            match stream.try_io(tokio::io::Interest::READABLE, move || unsafe {
                if use_cmsg {
                    drm_recvmsg(raw, dst)
                } else {
                    let m = libc::read(raw, dst.as_mut_ptr() as *mut libc::c_void, dst.len());
                    if m < 0 {
                        Err(std::io::Error::last_os_error())
                    } else {
                        Ok((m as usize, None))
                    }
                }
            }) {
                Ok((0, _fd)) => bail!("drm: socket closed by peer"),
                Ok((m, fd)) => {
                    if let Some(f) = fd {
                        if got.is_none() {
                            got = Some(f);
                        }
                    }
                    m
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
        };
        off += n;
    }
    Ok(got)
}

#[cfg(all(target_os = "linux", feature = "drm"))]
impl DrmConn {
    /// Take ownership of an already-connected/accepted raw `_drm` stream.
    pub fn new(stream: tokio::net::UnixStream) -> Self {
        Self {
            stream,
            read_buf: Vec::new(),
        }
    }

    /// Send one `Data` message (JSON, length-prefixed). When `fd` is `Some`, attach exactly one
    /// SCM_RIGHTS cmsg carrying that fd on the SAME frame as the payload (a -1 in an SCM_RIGHTS cmsg
    /// fails the whole call, so the cmsg is attached ONLY when a fd is present). `fd` is borrowed so
    /// the caller keeps ownership and closes it after the send has dup'd it into the peer.
    pub async fn send_msg(&mut self, data: &Data, fd: Option<BorrowedFd<'_>>) -> ResultType<()> {
        let payload = serde_json::to_vec(data)?;
        let pass_fd = fd.map(|f| f.as_raw_fd());
        drm_send_frame(&self.stream, &payload, pass_fd).await
    }

    /// Receive one `Data` message plus any dma-buf fd delivered via SCM_RIGHTS. Reads the 4-byte
    /// length prefix (with a `CMSG_SPACE(size_of::<c_int>())` control buffer that collects the fd bound
    /// to the frame's first byte, rejecting `MSG_CTRUNC`), then the payload into the reusable
    /// `read_buf`. Returns the decoded `Data` and an `OwnedFd` iff one arrived.
    pub async fn recv_msg(&mut self) -> ResultType<(Data, Option<OwnedFd>)> {
        let mut prefix = [0u8; 4];
        let fd = drm_read_full(&self.stream, &mut prefix, true).await?;
        let len = u32::from_be_bytes(prefix) as usize;
        if len > MAX_DRM_JSON_BYTES {
            // `fd` (if any) is closed on drop.
            bail!("drm: message length {len} exceeds cap {MAX_DRM_JSON_BYTES}");
        }
        if self.read_buf.len() < len {
            self.read_buf.resize(len, 0);
        }
        // Disjoint field borrows: &self.stream (read) + &mut self.read_buf (dest). No fd on the body.
        drm_read_full(&self.stream, &mut self.read_buf[..len], false).await?;
        let data: Data = serde_json::from_slice(&self.read_buf[..len])?;
        Ok((data, fd))
    }

    /// Cancel-safe timeout wrapper around `recv_msg`, mirroring `ConnectionTmpl::next_timeout2`, so a
    /// dropped consumer re-checks its `stop` flag between frames. `None` on timeout. The timeout gates
    /// ONLY the wait for the first byte (`readable()` consumes nothing), so a fired timeout leaves the
    /// stream at a clean frame boundary and never strands a partial frame or its fd.
    pub async fn recv_msg_timeout2(
        &mut self,
        ms_timeout: u64,
    ) -> Option<ResultType<(Data, Option<OwnedFd>)>> {
        // Bind the readiness result to a `let` so the borrowed `readable()` future temporary is dropped
        // at the `;` (releasing `&self.stream`) BEFORE `recv_msg()` takes `&mut self` in an arm.
        let ready = timeout(ms_timeout, self.stream.readable()).await;
        match ready {
            Err(_) => None, // timed out at a frame boundary
            Ok(Err(e)) => Some(Err(e.into())),
            Ok(Ok(())) => Some(self.recv_msg().await),
        }
    }

    /// Send a raw length-prefixed body (cursor pixels, CPU-fallback BGRA). Parity with
    /// `ConnectionTmpl::send_raw`, over the same manual framing (never carries an fd).
    pub async fn send_raw(&mut self, data: Bytes) -> ResultType<()> {
        drm_send_frame(&self.stream, &data, None).await
    }

    /// Receive a raw length-prefixed body. Parity with `ConnectionTmpl::next_raw`. A raw body never
    /// carries an fd; a stray fd (protocol desync) is collected by `drm_read_full` and dropped/closed.
    pub async fn next_raw(&mut self) -> ResultType<bytes::BytesMut> {
        let mut prefix = [0u8; 4];
        if drm_read_full(&self.stream, &mut prefix, true).await?.is_some() {
            log::warn!("drm: unexpected fd on a raw-body frame; dropping");
        }
        let len = u32::from_be_bytes(prefix) as usize;
        if len > MAX_DRM_RAW_BYTES {
            bail!("drm: raw body length {len} exceeds cap {MAX_DRM_RAW_BYTES}");
        }
        let mut out = bytes::BytesMut::new();
        out.resize(len, 0);
        drm_read_full(&self.stream, &mut out[..], false).await?;
        Ok(out)
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_config(name: &str) -> ResultType<Option<String>> {
    get_config_async(name, 1_000).await
}

async fn get_config_async(name: &str, ms_timeout: u64) -> ResultType<Option<String>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Config((name.to_owned(), None))).await?;
    if let Some(Data::Config((name2, value))) = c.next_timeout(ms_timeout).await? {
        if name == name2 {
            return Ok(value);
        }
    }
    return Ok(None);
}

pub async fn set_config_async(name: &str, value: String) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send_config(name, value).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_data(data: &Data) -> ResultType<()> {
    set_data_async(data).await
}

async fn set_data_async(data: &Data) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(data).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_config(name: &str, value: String) -> ResultType<()> {
    set_config_async(name, value).await
}

pub fn update_temporary_password() -> ResultType<()> {
    set_config("temporary-password", "".to_owned())
}

fn apply_permanent_password_storage_and_salt_payload(payload: Option<&str>) -> ResultType<()> {
    let Some(payload) = payload else {
        return Ok(());
    };
    let Some((storage, salt)) = payload.split_once('\n') else {
        bail!("Invalid permanent-password-storage-and-salt payload");
    };

    Config::set_permanent_password_storage_for_sync(storage, salt)?;
    Ok(())
}

pub fn sync_permanent_password_storage_from_daemon() -> ResultType<()> {
    let v = get_config("permanent-password-storage-and-salt")?;
    apply_permanent_password_storage_and_salt_payload(v.as_deref())
}

async fn sync_permanent_password_storage_from_daemon_async() -> ResultType<()> {
    let ms_timeout = 1_000;
    let v = get_config_async("permanent-password-storage-and-salt", ms_timeout).await?;
    apply_permanent_password_storage_and_salt_payload(v.as_deref())
}

pub fn is_permanent_password_set() -> bool {
    match get_config("permanent-password-set") {
        Ok(Some(v)) => {
            let v = v.trim();
            return v == "Y";
        }
        Ok(None) => {
            // No response/value (timeout).
        }
        Err(_) => {
            // Connection error.
        }
    }
    log::warn!("Failed to query permanent password state from daemon");
    false
}

pub fn is_permanent_password_preset() -> bool {
    if let Ok(Some(v)) = get_config("permanent-password-is-preset") {
        let v = v.trim();
        return v == "Y";
    }
    false
}

pub fn get_fingerprint() -> String {
    get_config("fingerprint")
        .unwrap_or_default()
        .unwrap_or_default()
}

pub fn set_permanent_password(v: String) -> ResultType<()> {
    if Config::is_disable_change_permanent_password() {
        bail!("Changing permanent password is disabled");
    }
    if set_permanent_password_with_ack(v)? {
        Ok(())
    } else {
        bail!("Changing permanent password was rejected by daemon");
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_permanent_password_with_ack(v: String) -> ResultType<bool> {
    set_permanent_password_with_ack_async(v).await
}

async fn set_permanent_password_with_ack_async(v: String) -> ResultType<bool> {
    // The daemon ACK/NACK is expected quickly since it applies the config in-process.
    let ms_timeout = 1_000;
    let mut c = connect(ms_timeout, "").await?;
    c.send_config("permanent-password", v).await?;
    if let Some(Data::Config((name2, Some(v)))) = c.next_timeout(ms_timeout).await? {
        if name2 == "permanent-password" {
            let v = v.trim();
            let ok = v == "Y";
            if ok {
                // Ensure the hashed permanent password storage is written to the user config file.
                // This sync must not affect the daemon ACK outcome.
                if let Err(err) = sync_permanent_password_storage_from_daemon_async().await {
                    log::warn!("Failed to sync permanent password storage from daemon: {err}");
                }
            }
            return Ok(ok);
        }
    }
    Ok(false)
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn set_unlock_pin(v: String, translate: bool) -> ResultType<()> {
    let v = v.trim().to_owned();
    let min_len = 4;
    let max_len = crate::ui_interface::max_encrypt_len();
    let len = v.chars().count();
    if !v.is_empty() {
        if len < min_len {
            let err = if translate {
                crate::lang::translate(
                    "Requires at least {".to_string() + &format!("{min_len}") + "} characters",
                )
            } else {
                // Sometimes, translated can't show normally in command line
                format!("Requires at least {} characters", min_len)
            };
            bail!(err);
        }
        if len > max_len {
            bail!("No more than {max_len} characters");
        }
    }
    Config::set_unlock_pin(&v);
    set_config("unlock-pin", v)
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_unlock_pin() -> String {
    if let Ok(Some(v)) = get_config("unlock-pin") {
        Config::set_unlock_pin(&v);
        v
    } else {
        Config::get_unlock_pin()
    }
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_trusted_devices() -> String {
    if let Ok(Some(v)) = get_config("trusted-devices") {
        v
    } else {
        Config::get_trusted_devices_json()
    }
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn remove_trusted_devices(hwids: Vec<Bytes>) {
    Config::remove_trusted_devices(&hwids);
    allow_err!(set_data(&Data::RemoveTrustedDevices(hwids)));
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn clear_trusted_devices() {
    Config::clear_trusted_devices();
    allow_err!(set_data(&Data::ClearTrustedDevices));
}

pub fn get_id() -> String {
    // An empty id may come from a process that took over the main IPC with a
    // config scope that has no id yet (e.g. a user GUI that became the server
    // while the installed service was restarting). Treat it as no answer,
    // otherwise the empty id is adopted below and wipes the local one.
    if let Ok(Some(v)) = get_config("id") {
        if !v.is_empty() {
            // update salt also, so that next time reinstallation not causing first-time auto-login failure
            if let Ok(Some(v2)) = get_config("salt") {
                Config::set_salt(&v2);
            }
            if v != Config::get_id() {
                Config::set_key_confirmed(false);
                Config::set_id(&v);
            }
            return v;
        }
    }
    Config::get_id()
}

pub async fn get_rendezvous_server(ms_timeout: u64) -> (String, Vec<String>) {
    if let Ok(Some(v)) = get_config_async("rendezvous_server", ms_timeout).await {
        let mut urls = v.split(",");
        let a = urls.next().unwrap_or_default().to_owned();
        let b: Vec<String> = urls.map(|x| x.to_owned()).collect();
        (a, b)
    } else {
        (
            Config::get_rendezvous_server(),
            Config::get_rendezvous_servers(),
        )
    }
}

async fn get_options_(ms_timeout: u64) -> ResultType<HashMap<String, String>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Options(None)).await?;
    if let Some(Data::Options(Some(value))) = c.next_timeout(ms_timeout).await? {
        Config::set_options(value.clone());
        Ok(value)
    } else {
        Ok(Config::get_options())
    }
}

pub async fn get_options_async() -> HashMap<String, String> {
    get_options_(1000).await.unwrap_or(Config::get_options())
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_options() -> HashMap<String, String> {
    get_options_async().await
}

pub async fn get_option_async(key: &str) -> String {
    if let Some(v) = get_options_async().await.get(key) {
        v.clone()
    } else {
        "".to_owned()
    }
}

pub fn set_option(key: &str, value: &str) {
    let mut options = get_options();
    if value.is_empty() {
        options.remove(key);
    } else {
        options.insert(key.to_owned(), value.to_owned());
    }
    set_options(options).ok();
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_options(value: HashMap<String, String>) -> ResultType<()> {
    let _nat = CheckTestNatType::new();
    if let Ok(mut c) = connect(1000, "").await {
        c.send(&Data::Options(Some(value.clone()))).await?;
        // do not put below before connect, because we need to check should_exit
        c.next_timeout(1000).await.ok();
    }
    Config::set_options(value);
    Ok(())
}

#[inline]
async fn get_nat_type_(ms_timeout: u64) -> ResultType<i32> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::NatType(None)).await?;
    if let Some(Data::NatType(Some(value))) = c.next_timeout(ms_timeout).await? {
        Config::set_nat_type(value);
        Ok(value)
    } else {
        Ok(Config::get_nat_type())
    }
}

pub async fn get_nat_type(ms_timeout: u64) -> i32 {
    get_nat_type_(ms_timeout)
        .await
        .unwrap_or(Config::get_nat_type())
}

pub async fn get_rendezvous_servers(ms_timeout: u64) -> Vec<String> {
    if let Ok(Some(v)) = get_config_async("rendezvous_servers", ms_timeout).await {
        return v.split(',').map(|x| x.to_owned()).collect();
    }
    return Config::get_rendezvous_servers();
}

#[inline]
async fn get_socks_(ms_timeout: u64) -> ResultType<Option<config::Socks5Server>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Socks(None)).await?;
    if let Some(Data::Socks(value)) = c.next_timeout(ms_timeout).await? {
        Config::set_socks(value.clone());
        Ok(value)
    } else {
        Ok(Config::get_socks())
    }
}

pub async fn get_socks_async(ms_timeout: u64) -> Option<config::Socks5Server> {
    get_socks_(ms_timeout).await.unwrap_or(Config::get_socks())
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_socks() -> Option<config::Socks5Server> {
    get_socks_async(1_000).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_socks(value: config::Socks5Server) -> ResultType<()> {
    let _nat = CheckTestNatType::new();
    Config::set_socks(if value.proxy.is_empty() {
        None
    } else {
        Some(value.clone())
    });
    connect(1_000, "")
        .await?
        .send(&Data::Socks(Some(value)))
        .await?;
    Ok(())
}

async fn get_socks_ws_(ms_timeout: u64) -> ResultType<(Option<config::Socks5Server>, String)> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::SocksWs(None)).await?;
    if let Some(Data::SocksWs(Some(value))) = c.next_timeout(ms_timeout).await? {
        Config::set_socks(value.0.clone());
        Config::set_option(OPTION_ALLOW_WEBSOCKET.to_string(), value.1.clone());
        Ok(*value)
    } else {
        Ok((
            Config::get_socks(),
            Config::get_option(OPTION_ALLOW_WEBSOCKET),
        ))
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_socks_ws() -> (Option<config::Socks5Server>, String) {
    get_socks_ws_(1_000).await.unwrap_or((
        Config::get_socks(),
        Config::get_option(OPTION_ALLOW_WEBSOCKET),
    ))
}

pub fn get_proxy_status() -> bool {
    Config::get_socks().is_some()
}
#[tokio::main(flavor = "current_thread")]
pub async fn test_rendezvous_server() -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::TestRendezvousServer).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn notify_deployed() -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::Deployed).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn send_url_scheme(url: String) -> ResultType<()> {
    connect(1_000, "_url")
        .await?
        .send(&Data::UrlLink(url))
        .await?;
    Ok(())
}

// Emit `close` events to ipc.
pub fn close_all_instances() -> ResultType<bool> {
    match crate::ipc::send_url_scheme(IPC_ACTION_CLOSE.to_owned()) {
        Ok(_) => Ok(true),
        Err(err) => Err(err),
    }
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
pub async fn connect_to_user_session(usid: Option<u32>) -> ResultType<()> {
    let mut stream = crate::ipc::connect_service(1000).await?;
    timeout(1000, stream.send(&crate::ipc::Data::UserSid(usid))).await??;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn notify_server_to_check_hwcodec() -> ResultType<()> {
    connect(1_000, "").await?.send(&&Data::CheckHwcodec).await?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub async fn get_port_forward_session_count(ms_timeout: u64) -> ResultType<usize> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::PortForwardSessionCount(None)).await?;
    if let Some(Data::PortForwardSessionCount(Some(count))) = c.next_timeout(ms_timeout).await? {
        return Ok(count);
    }
    bail!("Failed to get port forward session count");
}

#[cfg(feature = "hwcodec")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
pub async fn get_hwcodec_config_from_server() -> ResultType<()> {
    if !scrap::codec::enable_hwcodec_option() || scrap::hwcodec::HwCodecConfig::already_set() {
        return Ok(());
    }
    let mut c = connect(50, "").await?;
    c.send(&Data::HwCodecConfig(None)).await?;
    if let Some(Data::HwCodecConfig(v)) = c.next_timeout(50).await? {
        match v {
            Some(v) => {
                scrap::hwcodec::HwCodecConfig::set(v);
                return Ok(());
            }
            None => {
                bail!("hwcodec config is none");
            }
        }
    }
    bail!("failed to get hwcodec config");
}

#[cfg(feature = "hwcodec")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn client_get_hwcodec_config_thread(wait_sec: u64) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if !crate::platform::is_installed()
        || !scrap::codec::enable_hwcodec_option()
        || scrap::hwcodec::HwCodecConfig::already_set()
    {
        return;
    }
    ONCE.call_once(move || {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let mut intervals: Vec<u64> = vec![wait_sec, 3, 3, 6, 9];
            for i in intervals.drain(..) {
                if i > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(i));
                }
                if get_hwcodec_config_from_server().is_ok() {
                    break;
                }
            }
        });
    });
}

#[cfg(feature = "hwcodec")]
#[tokio::main(flavor = "current_thread")]
pub async fn hwcodec_process() {
    let s = scrap::hwcodec::check_available_hwcodec();
    for _ in 0..5 {
        match crate::ipc::connect(1000, "").await {
            Ok(mut conn) => {
                match conn
                    .send(&crate::ipc::Data::HwCodecConfig(Some(s.clone())))
                    .await
                {
                    Ok(()) => {
                        log::info!("send ok");
                        break;
                    }
                    Err(e) => {
                        log::error!("send failed: {e:?}");
                    }
                }
            }
            Err(e) => {
                log::error!("connect failed: {e:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_wayland_screencast_restore_token(key: String) -> ResultType<String> {
    let v = handle_wayland_screencast_restore_token(key, "get".to_owned()).await?;
    Ok(v.unwrap_or_default())
}

#[tokio::main(flavor = "current_thread")]
pub async fn clear_wayland_screencast_restore_token(key: String) -> ResultType<bool> {
    if let Some(v) = handle_wayland_screencast_restore_token(key, "clear".to_owned()).await? {
        return Ok(v.is_empty());
    }
    return Ok(false);
}

#[cfg(all(
    feature = "flutter",
    not(any(target_os = "android", target_os = "ios"))
))]
#[tokio::main(flavor = "current_thread")]
pub async fn update_controlling_session_count(count: usize) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::ControllingSessionCount(count)).await?;
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::main(flavor = "current_thread")]
pub async fn get_terminal_session_count() -> ResultType<usize> {
    let timeout_ms = 1_000;
    let effective_uid = unsafe { hbb_common::libc::geteuid() as u32 };
    let candidate_uids = terminal_count_candidate_uids(effective_uid);
    let mut last_err: Option<anyhow::Error> = None;
    for candidate_uid in candidate_uids {
        let socket_path = Config::ipc_path_for_uid(candidate_uid, "");
        let connect_result = timeout(timeout_ms, Endpoint::connect(&socket_path))
            .await
            .map_err(|err| {
                anyhow::anyhow!(
                    "Timeout connecting to terminal ipc at {}: {}",
                    socket_path,
                    err
                )
            });
        let connection = match connect_result {
            Ok(Ok(connection)) => connection,
            Ok(Err(err)) => {
                last_err = Some(anyhow::anyhow!(
                    "Failed to connect to terminal ipc at {}: {}",
                    socket_path,
                    err
                ));
                continue;
            }
            Err(err) => {
                last_err = Some(err);
                continue;
            }
        };
        let mut ipc_conn = ConnectionTmpl::new(connection);
        if let Err(err) = ipc_conn.send(&Data::TerminalSessionCount(0)).await {
            last_err = Some(anyhow::anyhow!(
                "Failed to request terminal session count via ipc at {}: {}",
                socket_path,
                err
            ));
            continue;
        }
        match ipc_conn.next_timeout(timeout_ms).await {
            Ok(Some(Data::TerminalSessionCount(session_count))) => {
                return Ok(session_count);
            }
            Ok(None) => {
                last_err = Some(anyhow::anyhow!(
                    "Invalid response when requesting terminal session count via ipc at {}",
                    socket_path
                ));
            }
            Ok(other) => {
                last_err = Some(anyhow::anyhow!(
                    "Unexpected response when requesting terminal session count via ipc at {}: {:?}",
                    socket_path,
                    other.map(|v| std::mem::discriminant(&v))
                ));
            }
            Err(err) => {
                last_err = Some(anyhow::anyhow!(
                    "Failed to read terminal session count via ipc at {}: {}",
                    socket_path,
                    err
                ));
            }
        }
    }
    if let Some(err) = last_err {
        Err(err.into())
    } else {
        Ok(0)
    }
}

async fn handle_wayland_screencast_restore_token(
    key: String,
    value: String,
) -> ResultType<Option<String>> {
    let ms_timeout = 1_000;
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::WaylandScreencastRestoreToken((key, value)))
        .await?;
    if let Some(Data::WaylandScreencastRestoreToken((_key, v))) = c.next_timeout(ms_timeout).await?
    {
        return Ok(Some(v));
    }
    return Ok(None);
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_install_option(k: String, v: String) -> ResultType<()> {
    if let Ok(mut c) = connect(1000, "").await {
        c.send(&&Data::InstallOption(Some((k, v)))).await?;
        // do not put below before connect, because we need to check should_exit
        c.next_timeout(1000).await.ok();
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_ffi_enum_data_size() {
        println!("{}", std::mem::size_of::<Data>());
        assert!(std::mem::size_of::<Data>() <= 120);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_service_ipc_path_is_shared_across_uids() {
        assert_eq!(
            Config::ipc_path_for_uid(0, crate::POSTFIX_SERVICE),
            Config::ipc_path_for_uid(501, crate::POSTFIX_SERVICE)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_ipc_path_differs_by_uid_for_cm() {
        let effective_uid = unsafe { hbb_common::libc::geteuid() as u32 };
        let other_uid = effective_uid.saturating_add(1);
        let postfix = "_cm";

        // Default connect path targets the current effective uid.
        assert_eq!(
            Config::ipc_path(postfix),
            Config::ipc_path_for_uid(effective_uid, postfix)
        );
        // A different uid yields a different socket path - this is the root cause of the
        // cross-user regression when root spawns a user process but still connects as uid 0.
        assert_ne!(
            Config::ipc_path(postfix),
            Config::ipc_path_for_uid(other_uid, postfix)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_uses_active_uid_when_no_server_found() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[], Some(501), false).unwrap(),
            501
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_uses_single_server_uid() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[501], None, false).unwrap(),
            501
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_prefers_active_uid_with_multiple_servers() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[0, 501], Some(501), false).unwrap(),
            501
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_prefers_root_on_wayland_login_screen() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[0, 501], Some(501), true).unwrap(),
            0
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_fails_when_multiple_servers_are_ambiguous() {
        assert!(select_server_uid_for_user_main_ipc(&[501, 502], None, false).is_err());
    }
}
