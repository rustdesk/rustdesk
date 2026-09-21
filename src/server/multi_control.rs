//! Primary-first multi-controller arbitration.
//!
//! One connection is the *primary*: its absolute moves drive the real pointer, so it
//! keeps the normal hover, menu traversal, drag and cursor-shape experience. Every
//! other connection is a *helper*: its moves only update its own cursor UI, and it
//! borrows the real pointer around its own operations - automatically for clicks,
//! wheel steps and drags, or for as long as it holds its local "operate" key.
//!
//! The primary always wins: as soon as it has real input, a borrow is revoked, the
//! buttons/keys the borrower injected are released, and the pointer goes back to the
//! primary position. Nothing is queued for later, so a rejected helper action never
//! lands on a desktop that already changed.
//!
//! This module is the pure decision layer: it owns no thread, does no I/O and takes
//! `Instant` as a parameter, so every rule below is unit tested. The thread that runs
//! the returned commands is `server/multi_control_worker.rs`.
//! See `docs/multi-control-master-plan.md`.

use crate::input::{
    MOUSE_BUTTON_BACK, MOUSE_BUTTON_FORWARD, MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT,
    MOUSE_BUTTON_WHEEL, MOUSE_TYPE_DOWN, MOUSE_TYPE_MASK, MOUSE_TYPE_MOVE,
    MOUSE_TYPE_MOVE_RELATIVE, MOUSE_TYPE_TRACKPAD, MOUSE_TYPE_UP, MOUSE_TYPE_WHEEL,
};
use base::{
    config::keys,
    message_proto::{KeyEvent, KeyboardMode, MouseEvent},
};
use hbb_common::config::Config;
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

/// Only this value turns the feature on; unset, empty or unknown keeps the legacy path.
pub const MODE_PRIMARY_FIRST: &str = "primary-first";

/// The primary counts as idle - and a helper may start borrowing - after this long
/// without real input. It never delays a primary preemption.
pub const PRIMARY_IDLE_GRACE: Duration = Duration::from_millis(600);
/// A borrow whose owner sends nothing at all for this long is revoked, so a peer that
/// dies in the middle of a drag cannot freeze the host pointer.
pub const BORROW_LEASE: Duration = Duration::from_secs(5);
/// Grace after the last button of a click, to keep double clicks together.
pub const CLICK_GRACE: Duration = Duration::from_millis(500);
/// Grace after the last wheel step, so a scroll gesture is not cut in half.
pub const SCROLL_GRACE: Duration = Duration::from_millis(200);
/// How often the worker calls [`on_tick`].
pub const TICK: Duration = Duration::from_millis(50);

/// The buttons this module can press and has to release again.
const BUTTONS: [i32; 5] = [
    MOUSE_BUTTON_LEFT,
    MOUSE_BUTTON_RIGHT,
    MOUSE_BUTTON_WHEEL,
    MOUSE_BUTTON_BACK,
    MOUSE_BUTTON_FORWARD,
];

/// Identity of an injected physical key. Text/IME events have no identity because they
/// are committed in one step instead of being held down.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyId(pub i32);

/// The key a physical `Map`-mode event refers to, if it is one.
pub fn key_id(evt: &KeyEvent) -> Option<KeyId> {
    match evt.mode.enum_value() {
        Ok(KeyboardMode::Map) => Some(KeyId(evt.chr() as i32)),
        _ => None,
    }
}

/// A release event for a key this module injected as down.
pub fn key_release_event(key: KeyId) -> KeyEvent {
    let mut evt = KeyEvent::new();
    evt.mode = KeyboardMode::Map.into();
    evt.set_chr(key.0 as u32);
    evt
}

/// What the caller has to do with the event it just handed in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Move the pointer and inject nothing else.
    Locate { x: i32, y: i32 },
    /// Inject the event where the pointer already is.
    Inject,
    /// Move the pointer first, then inject the event.
    LocateThenInject { x: i32, y: i32 },
    /// Do not inject: only the peer cursor UI uses this event.
    Track,
    /// Nothing was injected and nothing is queued.
    Drop(Reject),
}

/// Why an event was not injected. The worker turns these into peer notices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reject {
    /// The peer has no keyboard/mouse permission.
    NotPermitted,
    /// That peer does not speak this protocol, so it may only watch.
    NotSupported,
    /// The peer has no absolute position yet.
    NoPosition,
    /// Nobody may inject right now.
    NoPrimary,
    /// The primary has recent input or still holds buttons/keys.
    PrimaryBusy,
    /// Another helper holds the real pointer.
    OtherBusy,
    /// Keyboard input needs a confirmed target first.
    KeyboardNeedsTarget,
    /// Arbitration is paused; input stays off until it is resolved.
    Suspended,
    /// Event kind that this mode does not support.
    UnsupportedEvent,
    /// Swallowed on purpose, without telling the user (late release of a dropped key).
    Suppressed,
}

/// Side effects the worker has to perform, in order.
#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    /// Inject this event, which was already arbitrated.
    Mouse {
        conn: i32,
        evt: MouseEvent,
        simulate: bool,
    },
    /// Release a key this connection injected as down.
    KeyRelease { conn: i32, key: KeyId },
    /// Send the current state to these connections.
    Notify { conns: Vec<i32> },
}

/// Commands plus the decision for the event the caller handed in.
#[derive(Clone, PartialEq, Debug)]
pub struct Planned {
    pub commands: Vec<Command>,
    pub action: Action,
}

impl Planned {
    fn with(commands: Vec<Command>, action: Action) -> Self {
        Self { commands, action }
    }

    fn drop_with(commands: Vec<Command>, reject: Reject) -> Self {
        Self::with(commands, Action::Drop(reject))
    }
}

/// Which kind of operation is currently borrowing the real pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BorrowKind {
    /// Started by a pointer press; lasts until the double-click grace ends.
    Click,
    /// Started by a wheel step; lasts until the scroll grace ends.
    Scroll,
    /// A click that is still held: the borrower keeps the pointer until release.
    Drag,
    /// The peer holds its local "operate" key.
    Continuous,
}

/// Requests a peer can make about the real pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BorrowRequest {
    /// The "operate" key went down.
    Begin,
    /// The "operate" key went up, or the peer lost focus.
    End,
    /// Liveness of the borrowing carrying this epoch.
    Heartbeat,
}

#[derive(Clone, Copy, Debug)]
struct Borrow {
    peer: i32,
    epoch: u64,
    kind: BorrowKind,
    last_seen: Instant,
    /// When a borrow without held buttons gives the pointer back.
    grace_until: Option<Instant>,
    /// A completed, non-drag click happened inside this borrow.
    clicked: bool,
    /// The borrower moved while holding a button: the gesture is a drag, not a click.
    dragged: bool,
}

#[derive(Clone, Default, Debug)]
struct Peer {
    can_inject: bool,
    supported: bool,
    pos: Option<(i32, i32)>,
    pos_layout: u64,
    /// Buttons this module injected as down for this peer, and did not release yet.
    buttons: i32,
    /// Keys this module injected as down for this peer.
    keys: HashSet<KeyId>,
    /// Keys whose down was dropped: repeats stay dropped until the peer releases them.
    quarantined: HashSet<KeyId>,
    /// The keyboard target of this peer is known to hold its input.
    focus_confirmed: bool,
    /// Button pressed by this peer's current borrow, used to tell a click from a drag.
    pending_click: Option<i32>,
    last_user_input: Option<Instant>,
}

/// Why arbitration is paused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Suspend {
    /// The screen layout changed; positions must be re-established.
    LayoutResync,
    /// Injection could not be completed or ordered.
    InjectionFailed,
    /// The local user paused remote input.
    LocalPause,
}

#[derive(Default)]
struct State {
    peers: HashMap<i32, Peer>,
    /// Connection order, used to pick a primary deterministically.
    order: Vec<i32>,
    primary: Option<i32>,
    borrow: Option<Borrow>,
    epoch: u64,
    /// Bumped whenever the screen layout changes: positions of older layouts are void.
    layout: u64,
    suspended: Option<Suspend>,
    notice: HashMap<i32, &'static str>,
}

lazy_static::lazy_static! {
    static ref STATE: Mutex<State> = Mutex::new(State::default());
}

/// The host option: only `primary-first` enables this module.
pub fn enabled() -> bool {
    Config::get_option(keys::OPTION_MULTI_CONTROL_MODE) == MODE_PRIMARY_FIRST
}

/// Whether this connection is the primary controller right now, used by the worker to
/// route its events to the priority queue.
pub fn is_primary(conn: i32) -> bool {
    STATE.lock().unwrap().primary == Some(conn)
}

/// Hooks the mode into the older option: enabling this one turns the other off, so the
/// two arbitration models can never run at the same time.
pub fn enforce_exclusive_mode() {
    let mode = Config::get_option(keys::OPTION_MULTI_CONTROL_MODE);
    if mode != MODE_PRIMARY_FIRST {
        return;
    }
    if Config::get_option(keys::OPTION_INDEPENDENT_MOUSE) == "Y" {
        Config::set_option(keys::OPTION_INDEPENDENT_MOUSE.to_owned(), "N".to_owned());
    }
}

/// State of one connection, for the host UI and for the peer's own status line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StateOut {
    pub enabled: bool,
    pub is_primary: bool,
    pub borrowed_by_other: bool,
    pub borrowed_by_me: bool,
    pub borrower: Option<i32>,
    pub keyboard_target_confirmed: bool,
    pub epoch: u64,
    pub notice: String,
}

/// One connection as the host UI shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PeerOut {
    pub conn: i32,
    pub is_primary: bool,
    pub can_inject: bool,
    pub supported: bool,
    pub has_position: bool,
    pub borrowed: bool,
}

/// Registers a connection. `supported` is the negotiated capability, `can_inject` its
/// current keyboard/mouse permission.
pub fn register(conn: i32, supported: bool, can_inject: bool) -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    register_in(&mut st, conn, supported, can_inject)
}

fn register_in(st: &mut State, conn: i32, supported: bool, can_inject: bool) -> Vec<Command> {
    if !st.order.contains(&conn) {
        st.order.push(conn);
    }
    let entry = st.peers.entry(conn).or_default();
    entry.supported = supported;
    entry.can_inject = can_inject;
    // A connection that was authorized before the option was turned on still holds a
    // valid keyboard target until the first borrow invalidates it.
    entry.focus_confirmed = true;
    ensure_primary(st);
    notify_all(st)
}

/// Forgets a closed connection and releases what it still holds.
pub fn unregister(conn: i32) -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    unregister_in(&mut st, conn)
}

fn unregister_in(st: &mut State, conn: i32) -> Vec<Command> {
    st.order.retain(|id| *id != conn);
    st.notice.remove(&conn);
    let mut commands = if revoking_conn(st, conn) {
        revoke(st, RevokeReason::PeerClosed)
    } else {
        Vec::new()
    };
    commands.extend(release_peer(st, conn));
    st.peers.remove(&conn);
    if st.primary == Some(conn) {
        st.primary = None;
    }
    ensure_primary(st);
    commands.extend(notify_all(st));
    commands
}

/// Applies a keyboard/mouse permission change.
pub fn set_can_inject(conn: i32, can_inject: bool) -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    set_can_inject_in(&mut st, conn, can_inject)
}

fn set_can_inject_in(st: &mut State, conn: i32, can_inject: bool) -> Vec<Command> {
    let Some(peer) = st.peers.get_mut(&conn) else {
        return Vec::new();
    };
    if peer.can_inject == can_inject {
        return Vec::new();
    }
    peer.can_inject = can_inject;
    let mut commands = Vec::new();
    if !can_inject {
        // The peer cannot send releases anymore, so nothing of its own may stay down.
        if revoking_conn(st, conn) {
            commands.extend(revoke(st, RevokeReason::PermissionLost));
        }
        commands.extend(release_peer(st, conn));
    }
    ensure_primary(st);
    commands.extend(notify_all(st));
    commands
}

/// Makes `conn` the primary controller. The caller is the local user.
pub fn set_primary(conn: i32) -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    set_primary_in(&mut st, conn)
}

fn set_primary_in(st: &mut State, conn: i32) -> Vec<Command> {
    match st.peers.get(&conn) {
        Some(peer) if peer.can_inject && peer.supported => {}
        _ => return Vec::new(),
    }
    if st.primary == Some(conn) {
        return Vec::new();
    }
    st.primary = Some(conn);
    if let Some(peer) = st.peers.get_mut(&conn) {
        peer.focus_confirmed = true;
    }
    let mut commands = if revoking_conn(st, conn) {
        revoke(st, RevokeReason::PrimaryChanged)
    } else {
        Vec::new()
    };
    commands.extend(notify_all(st));
    commands
}

/// The screen layout, resolution or display count changed.
pub fn on_layout_changed() -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    layout_changed_in(&mut st)
}

fn layout_changed_in(st: &mut State) -> Vec<Command> {
    st.layout += 1;
    for peer in st.peers.values_mut() {
        if peer.pos_layout != st.layout {
            peer.pos = None;
        }
    }
    let mut commands = if st.borrow.is_some() {
        revoke(st, RevokeReason::LayoutChanged)
    } else {
        Vec::new()
    };
    st.suspended = Some(Suspend::LayoutResync);
    commands.extend(notify_all(st));
    commands
}

/// The local user paused or resumed remote input.
pub fn set_local_pause(paused: bool) -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    set_local_pause_in(&mut st, paused)
}

fn set_local_pause_in(st: &mut State, paused: bool) -> Vec<Command> {
    let mut commands = if paused && st.borrow.is_some() {
        revoke(st, RevokeReason::LocalPause)
    } else {
        Vec::new()
    };
    st.suspended = if paused {
        Some(Suspend::LocalPause)
    } else {
        None
    };
    commands.extend(notify_all(st));
    commands
}

/// Reports that injection could not be completed, or that it can be trusted again.
pub fn set_injection_failed(failed: bool) -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    set_injection_failed_in(&mut st, failed)
}

fn set_injection_failed_in(st: &mut State, failed: bool) -> Vec<Command> {
    if failed {
        if st.suspended == Some(Suspend::InjectionFailed) {
            return Vec::new();
        }
        let mut commands = if st.borrow.is_some() {
            revoke(st, RevokeReason::InjectionFailed)
        } else {
            Vec::new()
        };
        st.suspended = Some(Suspend::InjectionFailed);
        commands.extend(notify_all(st));
        return commands;
    }
    if st.suspended == Some(Suspend::InjectionFailed) {
        st.suspended = None;
        return notify_all(st);
    }
    Vec::new()
}

/// Arbitrates a mouse event.
pub fn on_mouse(conn: i32, evt: &MouseEvent, now: Instant) -> Planned {
    let mut st = STATE.lock().unwrap();
    on_mouse_in(&mut st, conn, evt, now)
}

/// Arbitrates a key event. `key` is `None` for text/IME commits, which have no
/// identity and can therefore not be held, quarantined or released.
pub fn on_key(conn: i32, key: Option<KeyId>, press: bool, now: Instant) -> Planned {
    let mut st = STATE.lock().unwrap();
    on_key_in(&mut st, conn, key, press, now)
}

/// Handles a borrow request from a peer that speaks this protocol.
pub fn on_borrow(conn: i32, request: BorrowRequest, epoch: u64, now: Instant) -> Planned {
    let mut st = STATE.lock().unwrap();
    on_borrow_in(&mut st, conn, request, epoch, now)
}

/// Periodic housekeeping: borrow lease, click/scroll grace, primary election.
pub fn on_tick(now: Instant) -> Vec<Command> {
    let mut st = STATE.lock().unwrap();
    tick_in(&mut st, now)
}

fn tick_in(st: &mut State, now: Instant) -> Vec<Command> {
    let mut commands = Vec::new();
    if let Some(borrow) = st.borrow {
        if now.saturating_duration_since(borrow.last_seen) >= BORROW_LEASE {
            commands.extend(revoke(st, RevokeReason::LeaseExpired));
        } else if borrow.kind != BorrowKind::Continuous {
            if let Some(until) = borrow.grace_until {
                if now >= until && st.peers.get(&borrow.peer).map_or(true, |p| p.buttons == 0) {
                    commands.extend(revoke(st, RevokeReason::Finished));
                }
            }
        }
    }
    if ensure_primary(st) {
        commands.extend(notify_all(st));
    }
    commands
}

/// What a peer has to be told about the current arbitration state.
pub fn snapshot(conn: i32) -> StateOut {
    let st = STATE.lock().unwrap();
    snapshot_in(&st, conn)
}

fn snapshot_in(st: &State, conn: i32) -> StateOut {
    let is_primary = st.primary == Some(conn);
    let borrowed_by_me = st.borrow.map_or(false, |b| b.peer == conn);
    let keyboard_target_confirmed = if is_primary {
        st.peers.get(&conn).map_or(false, |p| p.focus_confirmed)
    } else {
        borrowed_by_me && borrow_typing_allowed(&st.borrow)
    };
    StateOut {
        enabled: st.suspended.is_none(),
        is_primary,
        borrowed_by_other: st.borrow.map_or(false, |b| b.peer != conn),
        borrowed_by_me,
        borrower: st.borrow.map(|b| b.peer),
        keyboard_target_confirmed,
        epoch: st.borrow.map_or(0, |b| b.epoch),
        notice: st
            .notice
            .get(&conn)
            .map(|code| (*code).to_owned())
            .unwrap_or_default(),
    }
}

/// Every registered connection, for the host UI.
pub fn peers_status() -> Vec<PeerOut> {
    let st = STATE.lock().unwrap();
    peers_status_in(&st)
}

fn peers_status_in(st: &State) -> Vec<PeerOut> {
    st.order
        .iter()
        .filter_map(|conn| {
            let peer = st.peers.get(conn)?;
            Some(PeerOut {
                conn: *conn,
                is_primary: st.primary == Some(*conn),
                can_inject: peer.can_inject,
                supported: peer.supported,
                has_position: peer.pos.is_some(),
                borrowed: st.borrow.map_or(false, |b| b.peer == *conn),
            })
        })
        .collect()
}

/// The connection that currently owns the real pointer, 0 when there is none.
pub fn primary_conn() -> i32 {
    STATE.lock().unwrap().primary.unwrap_or(0)
}

/// The connections and their role as JSON, so the controlled side UI can show and change
/// the primary controller without knowing anything about the arbitration.
pub fn peers_status_json() -> String {
    let st = STATE.lock().unwrap();
    let peers: Vec<serde_json::Value> = peers_status_in(&st)
        .into_iter()
        .map(|peer| {
            serde_json::json!({
                "conn": peer.conn,
                "isPrimary": peer.is_primary,
                "canInject": peer.can_inject,
                "supported": peer.supported,
                "hasPosition": peer.has_position,
                "borrowed": peer.borrowed,
            })
        })
        .collect();
    serde_json::json!({
        "enabled": enabled(),
        "primary": st.primary.unwrap_or(0),
        "peers": peers,
    })
    .to_string()
}

/// Whether the module currently holds any state, for the worker's idle path.
pub fn is_idle() -> bool {
    let st = STATE.lock().unwrap();
    st.peers.is_empty() && st.borrow.is_none()
}

fn borrow_typing_allowed(borrow: &Option<Borrow>) -> bool {
    match borrow {
        Some(b) => b.kind == BorrowKind::Continuous && b.clicked,
        None => false,
    }
}

fn ensure_primary(st: &mut State) -> bool {
    if let Some(primary) = st.primary {
        if st
            .peers
            .get(&primary)
            .map_or(false, |p| p.can_inject && p.supported)
        {
            return false;
        }
        st.primary = None;
    }
    let candidate = st.order.iter().find_map(|conn| {
        let peer = st.peers.get(conn)?;
        (peer.can_inject && peer.supported).then_some(*conn)
    });
    if candidate.is_some() {
        st.primary = candidate;
        true
    } else {
        false
    }
}

fn notify_all(st: &State) -> Vec<Command> {
    if st.order.is_empty() {
        return Vec::new();
    }
    vec![Command::Notify {
        conns: st.order.clone(),
    }]
}

fn revoking_conn(st: &State, conn: i32) -> bool {
    st.borrow.map_or(false, |b| b.peer == conn)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RevokeReason {
    PrimaryInput,
    PrimaryChanged,
    PermissionLost,
    LayoutChanged,
    LocalPause,
    InjectionFailed,
    LeaseExpired,
    Finished,
    BorrowerEnded,
    PeerClosed,
}

impl RevokeReason {
    fn notice(self) -> &'static str {
        match self {
            RevokeReason::PrimaryInput => "borrow-preempted",
            RevokeReason::PrimaryChanged => "primary-changed",
            RevokeReason::PermissionLost => "permission-lost",
            RevokeReason::LayoutChanged => "layout-changed",
            RevokeReason::LocalPause => "local-pause",
            RevokeReason::InjectionFailed => "injection-failed",
            RevokeReason::LeaseExpired => "borrow-expired",
            RevokeReason::Finished => "borrow-finished",
            RevokeReason::BorrowerEnded => "borrow-finished",
            RevokeReason::PeerClosed => "borrow-finished",
        }
    }
}

/// Ends the current borrow, releasing everything the borrower injected.
fn revoke(st: &mut State, reason: RevokeReason) -> Vec<Command> {
    let Some(borrow) = st.borrow.take() else {
        return Vec::new();
    };
    st.notice.insert(borrow.peer, reason.notice());
    let mut commands = release_peer(st, borrow.peer);
    // The pointer moved away from the primary, so its keyboard target is not known
    // anymore: the primary has to confirm it again before it may type.
    if let Some(primary) = st.primary {
        if let Some(peer) = st.peers.get_mut(&primary) {
            peer.focus_confirmed = false;
        }
    }
    commands.extend(notify_all(st));
    commands
}

/// Releases the buttons and keys this connection still holds, in a fixed order.
fn release_peer(st: &mut State, conn: i32) -> Vec<Command> {
    let Some(peer) = st.peers.get_mut(&conn) else {
        return Vec::new();
    };
    let mut commands = Vec::new();
    for button in BUTTONS.iter() {
        if peer.buttons & *button != 0 {
            peer.buttons &= !*button;
            let mut evt = MouseEvent::new();
            evt.mask = (button << 3) | MOUSE_TYPE_UP;
            commands.push(Command::Mouse {
                conn,
                evt,
                simulate: true,
            });
        }
    }
    let mut keys: Vec<KeyId> = peer.keys.drain().collect();
    keys.sort_by_key(|key| key.0);
    for key in keys {
        commands.push(Command::KeyRelease { conn, key });
    }
    commands
}

fn set_notice(st: &mut State, conn: i32, code: &'static str) {
    st.notice.insert(conn, code);
}

fn reject_notice(reject: Reject) -> &'static str {
    match reject {
        Reject::NotPermitted => "no-permission",
        Reject::NotSupported => "peer-unsupported",
        Reject::NoPosition => "no-position",
        Reject::NoPrimary => "no-primary",
        Reject::PrimaryBusy => "primary-busy",
        Reject::OtherBusy => "other-busy",
        Reject::KeyboardNeedsTarget => "keyboard-needs-target",
        Reject::Suspended => "suspended",
        Reject::UnsupportedEvent => "unsupported-event",
        Reject::Suppressed => "",
    }
}

fn drop_with_notice(st: &mut State, conn: i32, reject: Reject) -> Action {
    let code = reject_notice(reject);
    if !code.is_empty() {
        set_notice(st, conn, code);
    }
    Action::Drop(reject)
}

fn known_buttons() -> i32 {
    BUTTONS.iter().fold(0, |mask, button| mask | button)
}

fn is_pointer_button(button: i32) -> bool {
    button & known_buttons() != 0
}

/// Whether the primary is idle enough to let a helper borrow the pointer.
fn primary_can_yield(st: &State, now: Instant) -> Result<i32, Reject> {
    let primary = st.primary.ok_or(Reject::NoPrimary)?;
    let peer = st.peers.get(&primary).ok_or(Reject::NoPrimary)?;
    if !peer.can_inject {
        return Err(Reject::NoPrimary);
    }
    if peer.pos.is_none() {
        return Err(Reject::NoPrimary);
    }
    if peer.buttons != 0 || !peer.keys.is_empty() {
        return Err(Reject::PrimaryBusy);
    }
    if let Some(last) = peer.last_user_input {
        if now.saturating_duration_since(last) < PRIMARY_IDLE_GRACE {
            return Err(Reject::PrimaryBusy);
        }
    }
    Ok(primary)
}

fn can_start_borrow(st: &State, conn: i32, now: Instant) -> Result<(i32, i32), Reject> {
    if st.suspended.is_some() {
        return Err(Reject::Suspended);
    }
    if st.borrow.is_some() {
        return Err(Reject::OtherBusy);
    }
    let peer = st.peers.get(&conn).ok_or(Reject::NotSupported)?;
    if !peer.can_inject {
        return Err(Reject::NotPermitted);
    }
    if !peer.supported {
        return Err(Reject::NotSupported);
    }
    let (x, y) = peer.pos.ok_or(Reject::NoPosition)?;
    primary_can_yield(st, now)?;
    Ok((x, y))
}

fn start_borrow(st: &mut State, conn: i32, kind: BorrowKind, now: Instant) -> u64 {
    st.epoch += 1;
    let epoch = st.epoch;
    st.borrow = Some(Borrow {
        peer: conn,
        epoch,
        kind,
        last_seen: now,
        grace_until: None,
        clicked: false,
        dragged: false,
    });
    if let Some(peer) = st.peers.get_mut(&conn) {
        peer.pending_click = None;
    }
    epoch
}

fn on_mouse_in(st: &mut State, conn: i32, evt: &MouseEvent, now: Instant) -> Planned {
    let evt_type = evt.mask & MOUSE_TYPE_MASK;
    let buttons = evt.mask >> 3;
    let Some(peer) = st.peers.get(&conn) else {
        return Planned::drop_with(Vec::new(), Reject::NotSupported);
    };
    // Without permission, or from a peer that does not speak this protocol, a move is
    // still forwarded for the cursor UI but nothing may reach the host.
    if !peer.can_inject || !peer.supported {
        let reject = if peer.can_inject {
            Reject::NotSupported
        } else {
            Reject::NotPermitted
        };
        return if evt_type == MOUSE_TYPE_MOVE {
            update_position(st, conn, evt.x, evt.y);
            Planned::with(Vec::new(), Action::Track)
        } else {
            let action = drop_with_notice(st, conn, reject);
            Planned::with(Vec::new(), action)
        };
    }
    if st.suspended.is_some() {
        // A paused mode only resumes when the primary re-establishes a valid position,
        // so its own absolute moves have to go through.
        let resyncing = st.suspended == Some(Suspend::LayoutResync)
            && st.primary == Some(conn)
            && evt_type == MOUSE_TYPE_MOVE;
        if !resyncing {
            let action = drop_with_notice(st, conn, Reject::Suspended);
            return Planned::with(Vec::new(), action);
        }
    }
    let is_primary = st.primary == Some(conn);
    if is_primary {
        on_primary_mouse(st, conn, evt_type, buttons, evt, now)
    } else {
        on_helper_mouse(st, conn, evt_type, buttons, evt, now)
    }
}

fn update_position(st: &mut State, conn: i32, x: i32, y: i32) {
    let layout = st.layout;
    if let Some(peer) = st.peers.get_mut(&conn) {
        peer.pos = Some((x, y));
        peer.pos_layout = layout;
    }
}

fn on_primary_mouse(
    st: &mut State,
    conn: i32,
    evt_type: i32,
    buttons: i32,
    evt: &MouseEvent,
    now: Instant,
) -> Planned {
    let mut commands = Vec::new();
    // Any real input of the primary takes the pointer back, even out of a drag.
    if st.borrow.is_some() {
        commands.extend(revoke(st, RevokeReason::PrimaryInput));
    }
    match evt_type {
        MOUSE_TYPE_MOVE => {
            let moved = st
                .peers
                .get(&conn)
                .and_then(|peer| peer.pos)
                .map_or(true, |pos| pos != (evt.x, evt.y));
            update_position(st, conn, evt.x, evt.y);
            if moved {
                mark_primary_input(st, conn, now);
            }
            resume_after_layout(st);
            Planned::with(commands, Action::Inject)
        }
        MOUSE_TYPE_DOWN | MOUSE_TYPE_UP => {
            mark_primary_input(st, conn, now);
            if let Some(peer) = st.peers.get_mut(&conn) {
                match evt_type {
                    MOUSE_TYPE_DOWN => peer.buttons |= buttons & known_buttons(),
                    _ => peer.buttons &= !(buttons & known_buttons()),
                }
                if evt_type == MOUSE_TYPE_DOWN && is_pointer_button(buttons) {
                    // A completed click is what confirms this peer's keyboard target.
                    peer.focus_confirmed = true;
                }
            }
            Planned::with(commands, Action::Inject)
        }
        MOUSE_TYPE_WHEEL | MOUSE_TYPE_TRACKPAD => {
            mark_primary_input(st, conn, now);
            Planned::with(commands, Action::Inject)
        }
        MOUSE_TYPE_MOVE_RELATIVE => {
            let action = drop_with_notice(st, conn, Reject::UnsupportedEvent);
            Planned::with(commands, action)
        }
        _ => {
            let action = drop_with_notice(st, conn, Reject::UnsupportedEvent);
            Planned::with(commands, action)
        }
    }
}

fn mark_primary_input(st: &mut State, conn: i32, now: Instant) {
    if st.primary == Some(conn) {
        if let Some(peer) = st.peers.get_mut(&conn) {
            peer.last_user_input = Some(now);
        }
    }
}

fn resume_after_layout(st: &mut State) {
    if st.suspended == Some(Suspend::LayoutResync) {
        let ready = st.primary.map_or(false, |primary| {
            st.peers
                .get(&primary)
                .map_or(false, |peer| peer.pos.is_some())
        });
        if ready {
            st.suspended = None;
        }
    }
}

fn on_helper_mouse(
    st: &mut State,
    conn: i32,
    evt_type: i32,
    buttons: i32,
    evt: &MouseEvent,
    now: Instant,
) -> Planned {
    let own = st.borrow.filter(|b| b.peer == conn);
    match evt_type {
        MOUSE_TYPE_MOVE => {
            let held = st.peers.get(&conn).map_or(0, |peer| peer.buttons);
            let continued = own.map_or(false, |b| {
                b.kind == BorrowKind::Continuous
                    || b.kind == BorrowKind::Drag
                    || b.grace_until.is_some()
                    || held != 0
            });
            update_position(st, conn, evt.x, evt.y);
            if !continued {
                return Planned::with(Vec::new(), Action::Track);
            }
            if let Some(borrow) = st.borrow.as_mut() {
                borrow.last_seen = now;
                if held != 0 {
                    borrow.dragged = true;
                    if borrow.kind != BorrowKind::Continuous {
                        borrow.kind = BorrowKind::Drag;
                    }
                }
            }
            Planned::with(Vec::new(), Action::Inject)
        }
        MOUSE_TYPE_DOWN => {
            if own.is_some() {
                if let Some(peer) = st.peers.get_mut(&conn) {
                    let fresh = peer.buttons == 0;
                    peer.buttons |= buttons & known_buttons();
                    // Only a press that starts with no button held is a click candidate.
                    peer.pending_click = fresh.then_some(buttons & known_buttons());
                }
                renew_borrow(st, conn, now);
                return Planned::with(Vec::new(), Action::Inject);
            }
            if buttons & known_buttons() == 0 {
                let action = drop_with_notice(st, conn, Reject::UnsupportedEvent);
                return Planned::with(Vec::new(), action);
            }
            match can_start_borrow(st, conn, now) {
                Ok((x, y)) => {
                    start_borrow(st, conn, BorrowKind::Click, now);
                    if let Some(peer) = st.peers.get_mut(&conn) {
                        peer.buttons |= buttons & known_buttons();
                        peer.pending_click = Some(buttons & known_buttons());
                    }
                    Planned::with(Vec::new(), Action::LocateThenInject { x, y })
                }
                Err(reject) => {
                    let action = drop_with_notice(st, conn, reject);
                    Planned::with(Vec::new(), action)
                }
            }
        }
        MOUSE_TYPE_UP => {
            let Some(borrow) = own else {
                // Nobody may release a button of a borrow it does not hold.
                let action = drop_with_notice(st, conn, Reject::Suppressed);
                return Planned::with(Vec::new(), action);
            };
            let was_pending = st
                .peers
                .get(&conn)
                .and_then(|peer| peer.pending_click)
                .map_or(false, |pending| pending & buttons != 0);
            if let Some(peer) = st.peers.get_mut(&conn) {
                peer.buttons &= !(buttons & known_buttons());
                if was_pending {
                    peer.pending_click = None;
                }
            }
            let held = st.peers.get(&conn).map_or(0, |peer| peer.buttons);
            let completed_click = was_pending && !borrow.dragged;
            if borrow.kind == BorrowKind::Continuous {
                if let Some(borrow) = st.borrow.as_mut() {
                    borrow.last_seen = now;
                    if completed_click {
                        borrow.clicked = true;
                    }
                }
            } else if held == 0 {
                let grace = if borrow.kind == BorrowKind::Scroll {
                    SCROLL_GRACE
                } else {
                    CLICK_GRACE
                };
                if let Some(borrow) = st.borrow.as_mut() {
                    borrow.last_seen = now;
                    borrow.grace_until = Some(now + grace);
                    if completed_click {
                        borrow.clicked = true;
                    }
                }
            } else if let Some(borrow) = st.borrow.as_mut() {
                borrow.last_seen = now;
                borrow.kind = BorrowKind::Drag;
            }
            Planned::with(Vec::new(), Action::Inject)
        }
        MOUSE_TYPE_WHEEL | MOUSE_TYPE_TRACKPAD => {
            if own.is_some() {
                renew_borrow(st, conn, now);
                return Planned::with(Vec::new(), Action::Inject);
            }
            match can_start_borrow(st, conn, now) {
                Ok((x, y)) => {
                    start_borrow(st, conn, BorrowKind::Scroll, now);
                    Planned::with(Vec::new(), Action::LocateThenInject { x, y })
                }
                Err(reject) => {
                    let action = drop_with_notice(st, conn, reject);
                    Planned::with(Vec::new(), action)
                }
            }
        }
        MOUSE_TYPE_MOVE_RELATIVE => {
            let action = drop_with_notice(st, conn, Reject::UnsupportedEvent);
            Planned::with(Vec::new(), action)
        }
        _ => {
            let action = drop_with_notice(st, conn, Reject::UnsupportedEvent);
            Planned::with(Vec::new(), action)
        }
    }
}

fn renew_borrow(st: &mut State, conn: i32, now: Instant) {
    if let Some(borrow) = st.borrow.as_mut() {
        if borrow.peer == conn {
            borrow.last_seen = now;
            borrow.grace_until = None;
        }
    }
}

fn on_key_in(st: &mut State, conn: i32, key: Option<KeyId>, press: bool, now: Instant) -> Planned {
    let Some(peer) = st.peers.get(&conn) else {
        return Planned::drop_with(Vec::new(), Reject::NotSupported);
    };
    if !peer.can_inject || !peer.supported {
        let reject = if peer.can_inject {
            Reject::NotSupported
        } else {
            Reject::NotPermitted
        };
        let action = drop_with_notice(st, conn, reject);
        return Planned::with(Vec::new(), action);
    }
    if st.suspended.is_some() {
        let action = drop_with_notice(st, conn, Reject::Suspended);
        return Planned::with(Vec::new(), action);
    }
    if st.primary == Some(conn) {
        return on_primary_key(st, conn, key, press, now);
    }
    on_helper_key(st, conn, key, press, now)
}

fn on_primary_key(
    st: &mut State,
    conn: i32,
    key: Option<KeyId>,
    press: bool,
    now: Instant,
) -> Planned {
    let mut commands = Vec::new();
    if st.borrow.is_some() {
        commands.extend(revoke(st, RevokeReason::PrimaryInput));
    }
    let confirmed = st
        .peers
        .get(&conn)
        .map_or(false, |peer| peer.focus_confirmed);
    if !press {
        let injected = match key {
            Some(key) => {
                let peer = st.peers.get_mut(&conn);
                match peer {
                    Some(peer) => {
                        peer.quarantined.remove(&key);
                        peer.keys.remove(&key)
                    }
                    None => false,
                }
            }
            None => false,
        };
        return if injected {
            mark_primary_input(st, conn, now);
            Planned::with(commands, Action::Inject)
        } else {
            // Either the press was dropped, or the key was never held down: releasing it
            // again would release a key this module does not own.
            Planned::with(commands, Action::Drop(Reject::Suppressed))
        };
    }
    if !confirmed {
        if let Some(key) = key {
            if let Some(peer) = st.peers.get_mut(&conn) {
                peer.quarantined.insert(key);
            }
        }
        let action = drop_with_notice(st, conn, Reject::KeyboardNeedsTarget);
        return Planned::with(commands, action);
    }
    if let Some(key) = key {
        let quarantined = st
            .peers
            .get(&conn)
            .map_or(false, |peer| peer.quarantined.contains(&key));
        if quarantined {
            return Planned::with(commands, Action::Drop(Reject::Suppressed));
        }
        if let Some(peer) = st.peers.get_mut(&conn) {
            peer.keys.insert(key);
        }
    }
    mark_primary_input(st, conn, now);
    Planned::with(commands, Action::Inject)
}

fn on_helper_key(
    st: &mut State,
    conn: i32,
    key: Option<KeyId>,
    press: bool,
    now: Instant,
) -> Planned {
    let allowed = st.borrow.map_or(false, |b| b.peer == conn) && borrow_typing_allowed(&st.borrow);
    if !allowed {
        if let Some(key) = key {
            if let Some(peer) = st.peers.get_mut(&conn) {
                if press {
                    peer.quarantined.insert(key);
                } else {
                    // The release of a dropped press ends its quarantine and is
                    // swallowed: no key of that gesture was ever injected.
                    peer.quarantined.remove(&key);
                }
            }
        }
        // A held-down key of a peer that may not type is a real, tellable refusal; a
        // release of it is only cleanup.
        let reject = if press {
            Reject::KeyboardNeedsTarget
        } else {
            Reject::Suppressed
        };
        let action = drop_with_notice(st, conn, reject);
        return Planned::with(Vec::new(), action);
    }
    if let Some(key) = key {
        if let Some(peer) = st.peers.get_mut(&conn) {
            if press {
                peer.keys.insert(key);
            } else {
                peer.keys.remove(&key);
            }
        }
    }
    renew_borrow(st, conn, now);
    Planned::with(Vec::new(), Action::Inject)
}

fn on_borrow_in(
    st: &mut State,
    conn: i32,
    request: BorrowRequest,
    epoch: u64,
    now: Instant,
) -> Planned {
    let own = st.borrow.filter(|b| b.peer == conn);
    match request {
        BorrowRequest::Begin => {
            if own.is_some() {
                // Idempotent: a repeated begin only renews the same borrow.
                renew_borrow(st, conn, now);
                return Planned::with(
                    Vec::new(),
                    Action::Locate {
                        x: borrow_peer_pos(st, conn),
                        y: borrow_peer_pos_y(st, conn),
                    },
                );
            }
            match can_start_borrow(st, conn, now) {
                Ok((x, y)) => {
                    start_borrow(st, conn, BorrowKind::Continuous, now);
                    Planned::with(Vec::new(), Action::Locate { x, y })
                }
                Err(reject) => {
                    let action = drop_with_notice(st, conn, reject);
                    Planned::with(Vec::new(), action)
                }
            }
        }
        BorrowRequest::End => {
            let Some(borrow) = own else {
                return Planned::with(Vec::new(), Action::Drop(Reject::Suppressed));
            };
            if epoch != 0 && epoch != borrow.epoch {
                // A late end of an older borrow must not end the current one.
                return Planned::with(Vec::new(), Action::Drop(Reject::Suppressed));
            }
            let commands = revoke(st, RevokeReason::BorrowerEnded);
            Planned::with(commands, Action::Drop(Reject::Suppressed))
        }
        BorrowRequest::Heartbeat => {
            let Some(borrow) = own else {
                return Planned::with(Vec::new(), Action::Drop(Reject::Suppressed));
            };
            if epoch != borrow.epoch {
                return Planned::with(Vec::new(), Action::Drop(Reject::Suppressed));
            }
            renew_borrow(st, conn, now);
            Planned::with(Vec::new(), Action::Drop(Reject::Suppressed))
        }
    }
}

fn borrow_peer_pos(st: &State, conn: i32) -> i32 {
    st.peers
        .get(&conn)
        .and_then(|peer| peer.pos)
        .map_or(0, |pos| pos.0)
}

fn borrow_peer_pos_y(st: &State, conn: i32) -> i32 {
    st.peers
        .get(&conn)
        .and_then(|peer| peer.pos)
        .map_or(0, |pos| pos.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::KeyboardMode;

    const LEFT: i32 = MOUSE_BUTTON_LEFT;
    const RIGHT: i32 = MOUSE_BUTTON_RIGHT;

    fn moved(x: i32, y: i32) -> MouseEvent {
        MouseEvent {
            mask: MOUSE_TYPE_MOVE,
            x,
            y,
            ..Default::default()
        }
    }

    fn button(button: i32, evt_type: i32) -> MouseEvent {
        MouseEvent {
            mask: (button << 3) | evt_type,
            ..Default::default()
        }
    }

    fn wheel() -> MouseEvent {
        MouseEvent {
            mask: MOUSE_TYPE_WHEEL,
            x: 120,
            y: 120,
            ..Default::default()
        }
    }

    fn key(code: i32) -> KeyId {
        KeyId(code)
    }

    /// A host with a primary (`1`) and one helper (`2`), both able to inject.
    fn host(now: Instant) -> State {
        let mut st = State::default();
        register_in(&mut st, 1, true, true);
        register_in(&mut st, 2, true, true);
        st.primary = Some(1);
        // Both peers already sent an absolute move, and the primary is idle.
        on_mouse_in(&mut st, 1, &moved(10, 10), now - Duration::from_secs(1));
        on_mouse_in(&mut st, 2, &moved(20, 20), now);
        st.primary = Some(1);
        st
    }

    fn positions(st: &State) -> (Option<(i32, i32)>, Option<(i32, i32)>) {
        (
            st.peers.get(&1).and_then(|peer| peer.pos),
            st.peers.get(&2).and_then(|peer| peer.pos),
        )
    }

    fn injected(planned: &Planned) -> bool {
        matches!(
            planned.action,
            Action::Inject | Action::LocateThenInject { .. } | Action::Locate { .. }
        )
    }

    #[test]
    fn the_primary_moves_the_real_pointer_and_helpers_only_track() {
        let now = Instant::now();
        let mut st = host(now);
        let primary = on_mouse_in(&mut st, 1, &moved(30, 40), now);
        assert_eq!(primary.action, Action::Inject);
        let helper = on_mouse_in(&mut st, 2, &moved(50, 60), now);
        assert_eq!(helper.action, Action::Track);
        assert_eq!(positions(&st), (Some((30, 40)), Some((50, 60))));
    }

    #[test]
    fn a_helper_click_borrows_the_pointer_at_its_own_position() {
        let now = Instant::now();
        let mut st = host(now);
        let planned = on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), now);
        assert_eq!(planned.action, Action::LocateThenInject { x: 20, y: 20 });
        let borrow = st.borrow.expect("borrow");
        assert_eq!(borrow.peer, 2);
        assert_eq!(borrow.kind, BorrowKind::Click);
        // The primary position is untouched, so the helper click cannot land on it.
        assert_eq!(positions(&st).0, Some((10, 10)));
    }

    #[test]
    fn a_helper_cannot_borrow_while_the_primary_is_busy() {
        let now = Instant::now();
        let mut st = host(now);
        on_mouse_in(&mut st, 1, &moved(11, 11), now);
        let planned = on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), now);
        assert_eq!(planned.action, Action::Drop(Reject::PrimaryBusy));
        assert!(st.borrow.is_none());
        // Once the primary has been idle long enough, the same click goes through.
        let later = now + PRIMARY_IDLE_GRACE + Duration::from_millis(1);
        let planned = on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), later);
        assert!(injected(&planned));
    }

    #[test]
    fn a_helper_needs_a_position_and_a_primary() {
        let now = Instant::now();
        let mut st = State::default();
        register_in(&mut st, 1, true, true);
        register_in(&mut st, 2, true, true);
        st.primary = Some(1);
        on_mouse_in(&mut st, 1, &moved(10, 10), now - Duration::from_secs(1));
        // The helper never sent a position.
        let planned = on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), now);
        assert_eq!(planned.action, Action::Drop(Reject::NoPosition));
        // The primary itself has no position either: nobody may borrow.
        on_mouse_in(&mut st, 2, &moved(20, 20), now);
        let mut st = State::default();
        register_in(&mut st, 1, true, true);
        register_in(&mut st, 2, true, true);
        st.primary = Some(1);
        on_mouse_in(&mut st, 2, &moved(20, 20), now);
        let planned = on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), now);
        assert_eq!(planned.action, Action::Drop(Reject::NoPrimary));
    }

    #[test]
    fn a_helper_drag_injects_every_move_and_blocks_the_other_helper() {
        let now = Instant::now();
        let mut st = host(now);
        register_in(&mut st, 3, true, true);
        on_mouse_in(&mut st, 3, &moved(70, 70), now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        assert_eq!(st.borrow.map(|borrow| borrow.kind), Some(BorrowKind::Click));
        let drag = on_mouse_in(&mut st, 2, &moved(25, 25), now);
        assert_eq!(drag.action, Action::Inject);
        assert_eq!(st.borrow.map(|borrow| borrow.kind), Some(BorrowKind::Drag));
        // The third connection may only move its own cursor.
        let other = on_mouse_in(&mut st, 3, &button(LEFT, MOUSE_TYPE_DOWN), now);
        assert_eq!(other.action, Action::Drop(Reject::OtherBusy));
        assert_eq!(
            on_mouse_in(&mut st, 3, &moved(71, 71), now).action,
            Action::Track
        );
        // A stray release of another connection must not end that drag.
        let stray = on_mouse_in(&mut st, 3, &button(LEFT, MOUSE_TYPE_UP), now);
        assert_eq!(stray.action, Action::Drop(Reject::Suppressed));
        assert!(st.borrow.is_some());
    }

    #[test]
    fn the_primary_preempts_a_helper_drag_and_gets_the_buttons_released() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        assert!(injected(&on_mouse_in(&mut st, 2, &moved(25, 25), now)));
        let primary = on_mouse_in(&mut st, 1, &moved(12, 12), now);
        assert_eq!(primary.action, Action::Inject);
        let released: Vec<&Command> = primary
            .commands
            .iter()
            .filter(|cmd| matches!(cmd, Command::Mouse { .. }))
            .collect();
        assert_eq!(released.len(), 1, "the borrowed button has to be released");
        match released[0] {
            Command::Mouse {
                conn,
                evt,
                simulate,
            } => {
                assert_eq!(*conn, 2);
                assert_eq!(evt.mask & MOUSE_TYPE_MASK, MOUSE_TYPE_UP);
                assert_eq!(evt.mask >> 3, LEFT);
                assert!(*simulate);
            }
            _ => unreachable!(),
        }
        assert!(st.borrow.is_none());
        // What the borrower sends afterwards must not touch the primary operation.
        let late = on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_UP), now);
        assert_eq!(late.action, Action::Drop(Reject::Suppressed));
        let late_move = on_mouse_in(&mut st, 2, &moved(26, 26), now);
        assert_eq!(late_move.action, Action::Track);
    }

    #[test]
    fn the_primary_has_to_confirm_its_keyboard_target_after_a_borrow() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_UP),
            now
        )));
        assert!(injected(&on_mouse_in(&mut st, 1, &moved(13, 13), now)));
        let dropped = on_key_in(&mut st, 1, Some(key(30)), true, now);
        assert_eq!(dropped.action, Action::Drop(Reject::KeyboardNeedsTarget));
        // Repeats of the dropped gesture stay dropped even after the target is confirmed.
        assert!(injected(&on_mouse_in(
            &mut st,
            1,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        assert!(injected(&on_mouse_in(
            &mut st,
            1,
            &button(LEFT, MOUSE_TYPE_UP),
            now
        )));
        let repeat = on_key_in(&mut st, 1, Some(key(30)), true, now);
        assert_eq!(repeat.action, Action::Drop(Reject::Suppressed));
        // Its release is swallowed instead of releasing a key nobody pressed.
        let release = on_key_in(&mut st, 1, Some(key(30)), false, now);
        assert_eq!(release.action, Action::Drop(Reject::Suppressed));
        // A new gesture is accepted once the target is confirmed again.
        let fresh = on_key_in(&mut st, 1, Some(key(31)), true, now);
        assert_eq!(fresh.action, Action::Inject);
        let release = on_key_in(&mut st, 1, Some(key(31)), false, now);
        assert_eq!(release.action, Action::Inject);
    }

    #[test]
    fn a_fresh_session_can_type_without_clicking_first() {
        let now = Instant::now();
        let mut st = host(now);
        let planned = on_key_in(&mut st, 1, Some(key(30)), true, now);
        assert_eq!(planned.action, Action::Inject);
    }

    #[test]
    fn a_helper_may_only_type_inside_a_continuous_borrow_after_a_click() {
        let now = Instant::now();
        let mut st = host(now);
        let denied = on_key_in(&mut st, 2, Some(key(30)), true, now);
        assert_eq!(denied.action, Action::Drop(Reject::KeyboardNeedsTarget));
        let begin = on_borrow_in(&mut st, 2, BorrowRequest::Begin, 0, now);
        assert_eq!(begin.action, Action::Locate { x: 20, y: 20 });
        // Continuous borrow, but no click yet.
        let denied = on_key_in(&mut st, 2, Some(key(30)), true, now);
        assert_eq!(denied.action, Action::Drop(Reject::KeyboardNeedsTarget));
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_UP),
            now
        )));
        let allowed = on_key_in(&mut st, 2, Some(key(30)), true, now);
        assert_eq!(allowed.action, Action::Inject);
    }

    #[test]
    fn a_continuous_borrow_survives_idle_time_but_not_a_lost_heartbeat() {
        let now = Instant::now();
        let mut st = host(now);
        let begin = on_borrow_in(&mut st, 2, BorrowRequest::Begin, 0, now);
        assert_eq!(begin.action, Action::Locate { x: 20, y: 20 });
        let epoch = st.borrow.expect("borrow").epoch;
        // Ten seconds of nothing but heartbeats: the hover stays.
        let later = now + Duration::from_secs(10);
        let beat = on_borrow_in(&mut st, 2, BorrowRequest::Heartbeat, epoch, later);
        assert_eq!(beat.action, Action::Drop(Reject::Suppressed));
        assert!(tick_in(&mut st, later).is_empty());
        assert!(st.borrow.is_some());
        // Without a heartbeat the lease expires and the pointer is given back.
        let lost = later + BORROW_LEASE + Duration::from_millis(1);
        assert!(!tick_in(&mut st, lost).is_empty());
        assert!(st.borrow.is_none());
    }

    #[test]
    fn ending_a_continuous_borrow_releases_what_it_held() {
        let now = Instant::now();
        let mut st = host(now);
        on_borrow_in(&mut st, 2, BorrowRequest::Begin, 0, now);
        let epoch = st.borrow.expect("borrow").epoch;
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        let end = on_borrow_in(&mut st, 2, BorrowRequest::End, epoch, now);
        assert!(end
            .commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Mouse { conn: 2, .. })));
        assert!(st.borrow.is_none());
        // A late end of the borrow that just ended changes nothing.
        let stale = on_borrow_in(&mut st, 2, BorrowRequest::End, epoch, now);
        assert!(stale.commands.is_empty());
        assert!(injected(&on_mouse_in(&mut st, 1, &moved(14, 14), now)));
    }

    #[test]
    fn a_click_grace_keeps_a_double_click_together_and_then_gives_up_the_pointer() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_UP),
            now
        )));
        let borrow = st.borrow.expect("borrow");
        assert!(borrow.clicked);
        assert_eq!(borrow.grace_until, Some(now + CLICK_GRACE));
        // The second click of the double click is still allowed.
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now + Duration::from_millis(100)
        )));
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_UP),
            now + Duration::from_millis(150)
        )));
        // After the grace the pointer returns to the primary.
        assert!(!tick_in(&mut st, now + CLICK_GRACE * 2 + Duration::from_millis(1)).is_empty());
        assert!(st.borrow.is_none());
    }

    #[test]
    fn wheel_values_are_never_read_as_coordinates() {
        let now = Instant::now();
        let mut st = host(now);
        let planned = on_mouse_in(&mut st, 2, &wheel(), now);
        assert_eq!(planned.action, Action::LocateThenInject { x: 20, y: 20 });
        assert_eq!(positions(&st).1, Some((20, 20)));
        assert_eq!(
            st.borrow.map(|borrow| borrow.kind),
            Some(BorrowKind::Scroll)
        );
    }

    #[test]
    fn relative_movement_is_dropped() {
        let now = Instant::now();
        let mut st = host(now);
        let evt = MouseEvent {
            mask: MOUSE_TYPE_MOVE_RELATIVE,
            x: 5,
            y: 5,
            ..Default::default()
        };
        assert_eq!(
            on_mouse_in(&mut st, 1, &evt, now).action,
            Action::Drop(Reject::UnsupportedEvent)
        );
        assert_eq!(
            on_mouse_in(&mut st, 2, &evt, now).action,
            Action::Drop(Reject::UnsupportedEvent)
        );
    }

    #[test]
    fn a_peer_without_permission_only_moves_its_cursor() {
        let now = Instant::now();
        let mut st = host(now);
        let commands = set_can_inject_in(&mut st, 2, false);
        assert!(!commands.is_empty());
        assert_eq!(
            on_mouse_in(&mut st, 2, &moved(21, 21), now).action,
            Action::Track
        );
        assert_eq!(
            on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), now).action,
            Action::Drop(Reject::NotPermitted)
        );
    }

    #[test]
    fn losing_permission_releases_the_buttons_and_the_borrow() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        let commands = set_can_inject_in(&mut st, 2, false);
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Mouse { conn: 2, .. })));
        assert!(st.borrow.is_none());
        assert_eq!(st.peers.get(&2).map(|peer| peer.buttons), Some(0));
    }

    #[test]
    fn closing_a_connection_releases_keys_and_elects_a_new_primary() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        let begin = on_borrow_in(&mut st, 2, BorrowRequest::Begin, 0, now);
        assert_eq!(begin.action, Action::Locate { x: 20, y: 20 });
        assert!(injected(&on_borrow_in(
            &mut st,
            2,
            BorrowRequest::Begin,
            0,
            now
        )));
        let commands = unregister_in(&mut st, 2);
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Mouse { conn: 2, .. })));
        assert!(st.borrow.is_none());
        assert!(st.peers.get(&2).is_none());
        // The remaining connection becomes the primary and may move the real pointer.
        assert_eq!(st.primary, Some(1));
        let commands = unregister_in(&mut st, 1);
        // Nothing of the primary was down, so nothing has to be released here.
        assert!(commands
            .iter()
            .all(|cmd| !matches!(cmd, Command::Mouse { .. })));
        assert_eq!(st.primary, None);
    }

    #[test]
    fn a_layout_change_revokes_the_borrow_and_pauses_until_resync() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        let commands = layout_changed_in(&mut st);
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Mouse { conn: 2, .. })));
        assert!(st.borrow.is_none());
        assert_eq!(st.suspended, Some(Suspend::LayoutResync));
        // Positions of the old layout are gone, and nothing may borrow while paused.
        assert_eq!(positions(&st), (None, None));
        assert_eq!(
            on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), now).action,
            Action::Drop(Reject::Suspended)
        );
        // The primary re-establishes its position and the mode resumes.
        assert!(injected(&on_mouse_in(&mut st, 1, &moved(15, 15), now)));
        assert_eq!(st.suspended, None);
        let later = now + PRIMARY_IDLE_GRACE + Duration::from_millis(1);
        assert_eq!(
            on_mouse_in(&mut st, 2, &moved(22, 22), later).action,
            Action::Track
        );
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            later
        )));
    }

    #[test]
    fn a_peer_that_does_not_speak_this_protocol_may_only_watch() {
        let now = Instant::now();
        let mut st = host(now);
        register_in(&mut st, 3, false, true);
        on_mouse_in(&mut st, 3, &moved(30, 30), now);
        assert_eq!(
            on_mouse_in(&mut st, 3, &moved(31, 31), now).action,
            Action::Track
        );
        assert_eq!(
            on_mouse_in(&mut st, 3, &button(LEFT, MOUSE_TYPE_DOWN), now).action,
            Action::Drop(Reject::NotSupported)
        );
        assert_eq!(
            on_key_in(&mut st, 3, Some(key(30)), true, now).action,
            Action::Drop(Reject::NotSupported)
        );
        // A borrowed pointer can not be requested by a peer that does not speak the
        // protocol, whatever epoch it claims.
        let late = now + PRIMARY_IDLE_GRACE + Duration::from_millis(1);
        assert_eq!(
            on_borrow_in(&mut st, 3, BorrowRequest::Begin, 0, late).action,
            Action::Drop(Reject::NotSupported)
        );
        assert!(st.borrow.is_none());
        assert_eq!(
            on_borrow_in(&mut st, 3, BorrowRequest::Heartbeat, 1, late).action,
            Action::Drop(Reject::Suppressed)
        );
        // It is never elected as primary.
        unregister_in(&mut st, 1);
        assert_eq!(st.primary, Some(2));
    }

    #[test]
    fn only_the_borrower_can_end_or_renew_a_borrow() {
        let now = Instant::now();
        let mut st = host(now);
        register_in(&mut st, 3, true, true);
        on_mouse_in(&mut st, 3, &moved(40, 40), now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        let epoch = st.borrow.expect("borrow").epoch;
        // Another helper guessing the epoch may not end or renew that borrow.
        let end = on_borrow_in(&mut st, 3, BorrowRequest::End, epoch, now);
        assert_eq!(end.action, Action::Drop(Reject::Suppressed));
        assert!(end.commands.is_empty());
        assert!(st.borrow.is_some());
        let heartbeat = on_borrow_in(&mut st, 3, BorrowRequest::Heartbeat, epoch, now);
        assert!(heartbeat.commands.is_empty());
        // The lease is still measured against the real borrower, so it does expire.
        let lost = now + BORROW_LEASE + Duration::from_millis(1);
        assert!(!tick_in(&mut st, lost).is_empty());
        assert!(st.borrow.is_none());
    }

    #[test]
    fn the_host_can_designate_the_primary_and_the_choice_survives_helpers() {
        let now = Instant::now();
        let mut st = host(now);
        register_in(&mut st, 3, true, true);
        let commands = set_primary_in(&mut st, 3);
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Notify { .. })));
        assert_eq!(st.primary, Some(3));
        // The new primary establishes its own position before anyone may borrow from it.
        on_mouse_in(&mut st, 3, &moved(30, 30), now);
        // A new connection never takes over the role.
        register_in(&mut st, 4, true, true);
        on_mouse_in(&mut st, 4, &moved(40, 40), now);
        assert_eq!(st.primary, Some(3));
        // Helpers may borrow from the new primary.
        let later = now + PRIMARY_IDLE_GRACE + Duration::from_millis(1);
        assert!(injected(&on_borrow_in(
            &mut st,
            2,
            BorrowRequest::Begin,
            0,
            later
        )));
        assert_eq!(st.borrow.map(|borrow| borrow.peer), Some(2));
    }

    #[test]
    fn host_pause_and_injection_failure_stop_the_pointer() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        let commands = set_local_pause_in(&mut st, true);
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Mouse { conn: 2, .. })));
        assert_eq!(
            on_mouse_in(&mut st, 1, &moved(16, 16), now).action,
            Action::Drop(Reject::Suspended)
        );
        assert_eq!(
            on_key_in(&mut st, 1, Some(key(30)), true, now).action,
            Action::Drop(Reject::Suspended)
        );
        set_local_pause_in(&mut st, false);
        assert!(injected(&on_mouse_in(&mut st, 1, &moved(16, 16), now)));
        let commands = set_injection_failed_in(&mut st, true);
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Notify { .. })));
        assert!(!snapshot_in(&st, 1).enabled);
        set_injection_failed_in(&mut st, false);
        assert!(snapshot_in(&st, 1).enabled);
    }

    #[test]
    fn the_primary_keeps_its_role_while_helpers_borrow() {
        let now = Instant::now();
        let mut st = host(now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        assert_eq!(st.primary, Some(1));
        assert!(snapshot_in(&st, 1).is_primary);
        assert!(!snapshot_in(&st, 1).borrowed_by_me);
        assert!(snapshot_in(&st, 1).borrowed_by_other);
        assert!(snapshot_in(&st, 2).borrowed_by_me);
        assert_eq!(snapshot_in(&st, 2).borrower, Some(2));
    }

    #[test]
    fn every_decision_carries_a_notice_code_for_the_peer() {
        let now = Instant::now();
        let mut st = host(now);
        on_mouse_in(&mut st, 1, &moved(11, 11), now);
        on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_DOWN), now);
        assert_eq!(snapshot_in(&st, 2).notice, "primary-busy");
        assert_eq!(
            reject_notice(Reject::KeyboardNeedsTarget),
            "keyboard-needs-target"
        );
        assert_eq!(reject_notice(Reject::Suppressed), "");
    }

    #[test]
    fn the_mode_is_off_until_the_option_says_otherwise() {
        // The option is stored in the shared config, so this only checks the default of
        // an unset option instead of assuming a user configuration.
        assert_eq!(MODE_PRIMARY_FIRST, "primary-first");
        let value = Config::get_option(keys::OPTION_MULTI_CONTROL_MODE);
        assert_eq!(enabled(), value == MODE_PRIMARY_FIRST);
    }

    #[test]
    fn a_keystroke_from_a_helper_inside_a_borrow_is_released_on_preemption() {
        let now = Instant::now();
        let mut st = host(now);
        on_borrow_in(&mut st, 2, BorrowRequest::Begin, 0, now);
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(RIGHT, MOUSE_TYPE_DOWN),
            now
        )));
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(RIGHT, MOUSE_TYPE_UP),
            now
        )));
        assert!(injected(&on_key_in(&mut st, 2, Some(key(29)), true, now)));
        assert!(injected(&on_mouse_in(
            &mut st,
            2,
            &button(LEFT, MOUSE_TYPE_DOWN),
            now
        )));
        let primary = on_mouse_in(&mut st, 1, &moved(17, 17), now);
        assert!(primary.commands.iter().any(|cmd| matches!(
            cmd,
            Command::KeyRelease {
                conn: 2,
                key: KeyId(29)
            }
        )));
        assert!(primary
            .commands
            .iter()
            .any(|cmd| matches!(cmd, Command::Mouse { conn: 2, .. })));
        assert_eq!(st.peers.get(&2).map(|peer| peer.keys.len()), Some(0));
    }

    #[test]
    fn key_release_events_target_the_same_key_code() {
        let evt = key_release_event(KeyId(30));
        assert_eq!(evt.mode.enum_value(), Ok(KeyboardMode::Map));
        assert_eq!(evt.chr(), 30);
        assert!(!evt.down);
        assert!(!evt.press);
    }

    /// Deterministic random source, so a failure can be replayed exactly.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// What a real injector would have down right now, derived from the decisions and
    /// the synthetic release commands, never from the arbitration state.
    #[derive(Default)]
    struct Screen {
        buttons: HashMap<i32, i32>,
        keys: HashSet<(i32, KeyId)>,
        /// Releases of buttons nobody pressed. A stray release is a no-op by design,
        /// so this only has to stay zero in the tests that drive down/up pairs.
        stray_releases: usize,
    }

    impl Screen {
        fn apply(&mut self, commands: &[Command]) {
            for command in commands {
                match command {
                    Command::Mouse { conn, evt, .. } => self.apply_event(*conn, evt),
                    Command::KeyRelease { conn, key } => {
                        if !self.keys.remove(&(*conn, *key)) {
                            self.stray_releases += 1;
                        }
                    }
                    Command::Notify { .. } => {}
                }
            }
        }

        /// The event the caller handed in is injected when the decision says so.
        fn apply_planned(&mut self, conn: i32, evt: &MouseEvent, planned: &Planned) {
            self.apply(&planned.commands);
            if matches!(
                planned.action,
                Action::Inject | Action::LocateThenInject { .. }
            ) {
                self.apply_event(conn, evt);
            }
        }

        fn apply_event(&mut self, conn: i32, evt: &MouseEvent) {
            if evt.mask & MOUSE_TYPE_MASK == MOUSE_TYPE_MOVE {
                return;
            }
            let buttons = evt.mask >> 3;
            match evt.mask & MOUSE_TYPE_MASK {
                MOUSE_TYPE_DOWN => *self.buttons.entry(conn).or_default() |= buttons,
                MOUSE_TYPE_UP => {
                    let held = self.buttons.entry(conn).or_default();
                    if *held & buttons == 0 {
                        self.stray_releases += 1;
                    }
                    *held &= !buttons;
                }
                _ => {}
            }
        }

        fn held_buttons(&self) -> i32 {
            self.buttons.values().fold(0, |mask, held| mask | held)
        }
    }

    /// An interleaving of three peers must never leave a button of one peer held by
    /// another, never release a button nobody pressed, and always give everything back
    /// when the peers disconnect.
    #[test]
    fn a_random_interleaving_keeps_the_buttons_accounted_for() {
        let mut rng = Rng(0x5eed);
        let mut st = State::default();
        let mut screen = Screen::default();
        let mut now = Instant::now();
        let conns = [1, 2, 3];
        for conn in conns {
            screen.apply(&register_in(&mut st, conn, true, true));
            screen.apply(&on_mouse_in(&mut st, conn, &moved(10 * conn, 10 * conn), now).commands);
        }
        for step in 0..4000 {
            now += Duration::from_millis(rng.below(120));
            // Occasionally jump past the borrow lease, so timeouts are covered too.
            if rng.below(200) == 0 {
                now += BORROW_LEASE + Duration::from_millis(1);
            }
            let conn = conns[rng.below(3) as usize];
            let evt = |rng: &mut Rng| moved(rng.below(500) as i32, rng.below(500) as i32);
            let (event, planned) = match rng.below(12) {
                0 => {
                    st.primary = None;
                    ensure_primary(&mut st);
                    (
                        MouseEvent::new(),
                        Planned::with(notify_all(&st), Action::Track),
                    )
                }
                1 => {
                    let ev = evt(&mut rng);
                    let planned = on_mouse_in(&mut st, conn, &ev, now);
                    (ev, planned)
                }
                2 => {
                    let ev = button(LEFT, MOUSE_TYPE_DOWN);
                    let planned = on_mouse_in(&mut st, conn, &ev, now);
                    (ev, planned)
                }
                3 => {
                    let ev = button(LEFT, MOUSE_TYPE_UP);
                    let planned = on_mouse_in(&mut st, conn, &ev, now);
                    (ev, planned)
                }
                4 => {
                    let ev = button(RIGHT, MOUSE_TYPE_DOWN);
                    let planned = on_mouse_in(&mut st, conn, &ev, now);
                    (ev, planned)
                }
                5 => {
                    let ev = button(RIGHT, MOUSE_TYPE_UP);
                    let planned = on_mouse_in(&mut st, conn, &ev, now);
                    (ev, planned)
                }
                6 => {
                    let ev = wheel();
                    let planned = on_mouse_in(&mut st, conn, &ev, now);
                    (ev, planned)
                }
                7 => (
                    MouseEvent::new(),
                    on_key_in(
                        &mut st,
                        conn,
                        Some(key(30 + rng.below(4) as i32)),
                        true,
                        now,
                    ),
                ),
                8 => (
                    MouseEvent::new(),
                    on_key_in(
                        &mut st,
                        conn,
                        Some(key(30 + rng.below(4) as i32)),
                        false,
                        now,
                    ),
                ),
                9 => (
                    MouseEvent::new(),
                    on_borrow_in(&mut st, conn, BorrowRequest::Begin, 0, now),
                ),
                10 => {
                    let epoch = st.borrow.map_or(0, |borrow| borrow.epoch);
                    (
                        MouseEvent::new(),
                        on_borrow_in(&mut st, conn, BorrowRequest::Heartbeat, epoch, now),
                    )
                }
                _ => {
                    let mut commands = tick_in(&mut st, now);
                    if rng.below(50) == 0 {
                        commands.extend(layout_changed_in(&mut st));
                        // The primary establishes a new position after a layout change.
                        let primary = st.primary.unwrap_or(1);
                        commands.extend(on_mouse_in(&mut st, primary, &moved(1, 1), now).commands);
                    }
                    (MouseEvent::new(), Planned::with(commands, Action::Track))
                }
            };
            screen.apply_planned(conn, &event, &planned);
            // Invariants after every single event.
            let borrow_owner = st.borrow.map(|borrow| borrow.peer);
            if let Some(owner) = borrow_owner {
                assert!(
                    st.peers.contains_key(&owner),
                    "step {}: the borrower left the host",
                    step
                );
            }
            for (conn, peer) in st.peers.iter() {
                assert_eq!(
                    peer.buttons & !known_buttons(),
                    0,
                    "step {}: #{} tracks an unknown button",
                    step,
                    conn
                );
                // What was really injected as down must be exactly what the arbitration
                // believes is down for that peer, for every role.
                assert_eq!(
                    screen.buttons.get(conn).copied().unwrap_or(0),
                    peer.buttons,
                    "step {}: #{} and the injected state disagree",
                    step,
                    conn
                );
            }
        }
        // A clean shutdown must release everything and leave no borrow behind.
        for conn in conns {
            screen.apply(&unregister_in(&mut st, conn));
        }
        assert_eq!(screen.held_buttons(), 0, "a button stayed down");
        assert!(st.borrow.is_none());
        assert!(st.peers.is_empty());
    }

    /// The account of pressed buttons has to survive a helper that dies in the middle of
    /// a drag, at any point of the drag.
    #[test]
    fn a_drag_broken_at_any_point_gives_every_button_back() {
        for steps in 0..8 {
            let now = Instant::now();
            let mut st = host(now);
            let mut screen = Screen::default();
            let down = button(LEFT, MOUSE_TYPE_DOWN);
            let planned = on_mouse_in(&mut st, 2, &down, now);
            screen.apply_planned(2, &down, &planned);
            for step in 0..steps {
                let delta = Duration::from_millis(10 * step as u64);
                let ev = moved(20 + step, 20 + step);
                let planned = on_mouse_in(&mut st, 2, &ev, now + delta);
                screen.apply_planned(2, &ev, &planned);
            }
            let later = now + Duration::from_millis(10 * steps as u64) + BORROW_LEASE;
            screen.apply(&tick_in(&mut st, later));
            assert!(st.borrow.is_none(), "drag {} was not given up", steps);
            assert_eq!(
                st.peers.get(&2).map(|peer| peer.buttons),
                Some(0),
                "drag {} kept a button",
                steps
            );
            screen.apply(&unregister_in(&mut st, 2));
            assert_eq!(screen.held_buttons(), 0, "drag {} leaked a button", steps);
            assert_eq!(screen.stray_releases, 0);
        }
    }

    /// The primary wins at every point of a helper drag, and the helper cannot come back
    /// with the events of the drag it just lost.
    #[test]
    fn the_primary_wins_at_every_point_of_a_helper_drag() {
        for steps in 0..8 {
            let now = Instant::now();
            let mut st = host(now);
            let mut screen = Screen::default();
            let down = button(LEFT, MOUSE_TYPE_DOWN);
            let planned = on_mouse_in(&mut st, 2, &down, now);
            screen.apply_planned(2, &down, &planned);
            for step in 0..steps {
                let delta = Duration::from_millis(10 * step as u64);
                let ev = moved(30 + step, 30 + step);
                let planned = on_mouse_in(&mut st, 2, &ev, now + delta);
                screen.apply_planned(2, &ev, &planned);
            }
            let at = now + Duration::from_millis(10 * steps as u64 + 1);
            let ev = moved(99, 99);
            let primary = on_mouse_in(&mut st, 1, &ev, at);
            screen.apply_planned(1, &ev, &primary);
            assert_eq!(primary.action, Action::Inject);
            assert!(st.borrow.is_none(), "drag {} survived the primary", steps);
            assert_eq!(screen.held_buttons(), 0, "drag {} kept a button", steps);
            // The rest of the lost drag is inert, even after the idle grace.
            let after = at + PRIMARY_IDLE_GRACE + Duration::from_millis(1);
            let late_move = on_mouse_in(&mut st, 2, &moved(40, 40), after);
            assert_eq!(late_move.action, Action::Track);
            let late_up = on_mouse_in(&mut st, 2, &button(LEFT, MOUSE_TYPE_UP), after);
            assert_eq!(late_up.action, Action::Drop(Reject::Suppressed));
            assert!(late_up.commands.is_empty());
            assert_eq!(screen.held_buttons(), 0);
        }
    }
}
