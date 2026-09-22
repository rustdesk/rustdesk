//! The thread that executes what `server/multi_control.rs` decided.
//!
//! Every remote mouse/key event is first forwarded here instead of being injected on the
//! per-connection input thread. One worker thread then arbitrates and injects, so
//!
//! * arbitration happens in arrival order, not in per-connection thread order,
//! * a `locate` and the event it belongs to are injected back to back, with no other
//!   connection's event in between,
//! * the primary's events are drained before a queue of helper moves, and superseded
//!   helper moves are coalesced, so a helper flood cannot delay a primary preemption by
//!   more than one bounded batch.
//!
//! Injection itself stays the untouched `input_service::handle_mouse`/`handle_key`, so
//! per-platform behavior is unchanged. On Windows the injection is an IPC message to the
//! service/portable process: two batches keep their order because this one thread
//! enqueues them in order, but there is no completion acknowledgement on that path, so
//! "injected" still means "handed to the injector".

use super::{
    connection::Sender,
    input_service,
    multi_control::{self, Action, BorrowRequest, Command, Planned, Reject},
    multi_control_overlay,
};
use crate::input::MOUSE_TYPE_MASK;
use base::message_proto::{
    KeyEvent, Message, Misc, MouseEvent, MultiControlState, PointerDeviceEvent,
};
use hbb_common::log;
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

/// Helper input may queue behind at most this many jobs before the worker re-checks the
/// primary queue and the timers.
const MAX_BATCH: usize = 16;
/// How often the display layout is compared, to notice a resolution or display change.
const LAYOUT_CHECK: Duration = Duration::from_secs(1);
/// How long the worker keeps waking up after the mode is turned off before it stops.
const IDLE_SHUTDOWN: Duration = Duration::from_secs(1);

/// One piece of work for the worker, in arrival order.
enum Job {
    Mouse {
        conn: i32,
        evt: MouseEvent,
        username: String,
        argb: u32,
        simulate: bool,
        show_cursor: bool,
    },
    Key {
        conn: i32,
        evt: KeyEvent,
        press: bool,
    },
    /// Touch and pen: no per-event arbitration of their own, so the worker decides here
    /// whether this connection may send them at all.
    Pointer {
        conn: i32,
        evt: PointerDeviceEvent,
    },
    Borrow {
        conn: i32,
        request: BorrowRequest,
        epoch: u64,
    },
    Register {
        conn: i32,
        supported: bool,
        can_inject: bool,
    },
    Unregister {
        conn: i32,
    },
    CanInject {
        conn: i32,
        can_inject: bool,
    },
    Designate {
        conn: i32,
    },
}

/// What the worker needs about a connection beyond the arbitration state.
struct Peer {
    sender: Sender,
    /// The connection's message channel to the connection manager, so a role or borrow
    /// change can update the window the local user is looking at.
    cm: Option<hbb_common::tokio::sync::mpsc::UnboundedSender<crate::ipc::Data>>,
    username: String,
    argb: u32,
    show_cursor: bool,
    /// Kept here, not only in the arbitration state, so a mode that is turned on after the
    /// connection was established can be told about it without reconnecting.
    supported: bool,
    can_inject: bool,
    /// The last state sent to this peer, so a refusal that repeats does not send the same
    /// message again.
    last_out: Option<multi_control::StateOut>,
}

struct Queues {
    primary: mpsc::Sender<Job>,
    helper: mpsc::Sender<Job>,
}

lazy_static::lazy_static! {
    static ref QUEUES: Mutex<Option<Queues>> = Mutex::new(None);
    static ref PEERS: Mutex<HashMap<i32, Peer>> = Mutex::new(HashMap::new());
}

/// Starts the worker on first use and returns both queues.
///
/// A start also tells the arbitration about every connection that is already known, so a
/// mode turned on after the connections were established sees them without a reconnect.
fn queues() -> Option<(mpsc::Sender<Job>, mpsc::Sender<Job>)> {
    let queues = {
        let mut guard = QUEUES.lock().unwrap();
        if let Some(queues) = guard.as_ref() {
            return Some((queues.primary.clone(), queues.helper.clone()));
        }
        let (tx_primary, rx_primary) = mpsc::channel::<Job>();
        let (tx_helper, rx_helper) = mpsc::channel::<Job>();
        if let Err(err) = std::thread::Builder::new()
            .name("multi-control".to_owned())
            .spawn(move || run(rx_primary, rx_helper))
        {
            log::error!("failed to start the multi-control worker: {}", err);
            return None;
        }
        *guard = Some(Queues {
            primary: tx_primary.clone(),
            helper: tx_helper.clone(),
        });
        (tx_primary, tx_helper)
    };
    let seeds: Vec<(i32, bool, bool)> = PEERS
        .lock()
        .unwrap()
        .iter()
        .map(|(conn, peer)| (*conn, peer.supported, peer.can_inject))
        .collect();
    for (conn, supported, can_inject) in seeds {
        let _ = queues.0.send(Job::Register {
            conn,
            supported,
            can_inject,
        });
    }
    Some(queues)
}

/// Queues a job; the primary's events and every control job jump the helper queue.
fn enqueue(job: Job, priority: bool) {
    let Some((primary, helper)) = queues() else {
        return;
    };
    let result = if priority {
        primary.send(job)
    } else {
        helper.send(job)
    };
    if let Err(err) = result {
        log::error!("failed to queue a multi-control job: {}", err);
    }
}

/// Registers a connection, the channel that pushes state back to it, and the channel that
/// updates its row in the connection manager window.
pub fn register(
    conn: i32,
    supported: bool,
    can_inject: bool,
    sender: Sender,
    cm: hbb_common::tokio::sync::mpsc::UnboundedSender<crate::ipc::Data>,
    username: String,
) {
    PEERS.lock().unwrap().insert(
        conn,
        Peer {
            sender,
            cm: Some(cm),
            username,
            argb: 0,
            show_cursor: false,
            supported,
            can_inject,
            last_out: None,
        },
    );
    // While the mode is off nothing has to arbitrate, so this only records the connection:
    // the worker starts with the mode, seeded from here.
    if multi_control::enabled() {
        enqueue(
            Job::Register {
                conn,
                supported,
                can_inject,
            },
            true,
        );
    }
}

/// Forgets a connection and releases everything it still held.
pub fn unregister(conn: i32) {
    let known = PEERS.lock().unwrap().remove(&conn).is_some();
    // A connection that never took part may not start the worker just to tell it so.
    if known && QUEUES.lock().unwrap().is_some() {
        enqueue(Job::Unregister { conn }, true);
    }
}

/// Applies a keyboard/mouse permission change of a connection.
pub fn set_can_inject(conn: i32, can_inject: bool) {
    let known = match PEERS.lock().unwrap().get_mut(&conn) {
        Some(peer) => {
            peer.can_inject = can_inject;
            true
        }
        None => false,
    };
    if known && QUEUES.lock().unwrap().is_some() {
        enqueue(Job::CanInject { conn, can_inject }, true);
    }
}

/// Makes `conn` the primary controller, as asked by the local user.
pub fn designate_primary(conn: i32) {
    enqueue(Job::Designate { conn }, true);
}

/// Forwards a remote mouse event to the arbitration worker.
pub fn send_mouse(
    conn: i32,
    evt: MouseEvent,
    username: String,
    argb: u32,
    simulate: bool,
    show_cursor: bool,
) {
    if let Some(peer) = PEERS.lock().unwrap().get_mut(&conn) {
        peer.username = username.clone();
        peer.argb = argb;
        peer.show_cursor = show_cursor;
    }
    let priority = multi_control::is_primary(conn);
    enqueue(
        Job::Mouse {
            conn,
            evt,
            username,
            argb,
            simulate,
            show_cursor,
        },
        priority,
    );
}

/// Forwards a remote key event to the arbitration worker.
pub fn send_key(conn: i32, evt: KeyEvent, press: bool) {
    let priority = multi_control::is_primary(conn);
    enqueue(Job::Key { conn, evt, press }, priority);
}

/// Forwards a touch or pen event; the worker decides whether this connection may send it.
pub fn send_pointer(conn: i32, evt: PointerDeviceEvent) {
    enqueue(Job::Pointer { conn, evt }, true);
}

/// Forwards a borrow request of a peer that speaks this protocol.
pub fn send_borrow(conn: i32, request: BorrowRequest, epoch: u64) {
    enqueue(
        Job::Borrow {
            conn,
            request,
            epoch,
        },
        true,
    );
}

/// The host option, so the connection code has a single entry point.
pub fn enabled() -> bool {
    multi_control::enabled()
}

fn run(rx_primary: mpsc::Receiver<Job>, rx_helper: mpsc::Receiver<Job>) {
    let mut last_layout = layout_signature();
    let mut last_layout_check = Instant::now();
    let mut idle_since: Option<Instant> = None;
    loop {
        while let Ok(job) = rx_primary.try_recv() {
            handle(job);
        }
        match rx_helper.recv_timeout(multi_control::TICK) {
            Ok(job) => handle_batch(job, &rx_helper),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            // The queue senders live in `QUEUES`, which this thread keeps alive, so a
            // disconnect means the process is going down.
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if !multi_control::enabled() {
            if !idle_tick(&mut idle_since) {
                return;
            }
            continue;
        }
        idle_since = None;
        execute(multi_control::on_tick(Instant::now()));
        if last_layout_check.elapsed() >= LAYOUT_CHECK {
            last_layout_check = Instant::now();
            let signature = layout_signature();
            if signature != last_layout {
                last_layout = signature;
                execute(multi_control::on_layout_changed());
            }
        }
        publish_overlay();
    }
}

/// The loop body of a mode that is off: forget what the arbitration still held once, and
/// stop the thread when nothing needs it, so a disabled feature costs nothing.
fn idle_tick(idle_since: &mut Option<Instant>) -> bool {
    execute(multi_control::reset_if_disabled());
    let since = idle_since.get_or_insert_with(Instant::now);
    if since.elapsed() < IDLE_SHUTDOWN {
        return true;
    }
    *QUEUES.lock().unwrap() = None;
    false
}

/// Keeps the overlay of the local desktop in step with where the controllers are. Cheap
/// when the option is off, which is its default.
fn publish_overlay() {
    if !multi_control_overlay::enabled() {
        if multi_control_overlay::is_active() {
            multi_control_overlay::stop();
        }
        return;
    }
    let Some(desktop) = multi_control_overlay::desktop_rect() else {
        return;
    };
    let peers = multi_control::cursor_snapshot();
    let cursors = multi_control::cursors_to_draw(&peers, desktop, multi_control::primary_conn());
    multi_control_overlay::update(&cursors);
}

fn handle_batch(first: Job, rx_helper: &mpsc::Receiver<Job>) {
    let mut batch = vec![first];
    while batch.len() < MAX_BATCH {
        match rx_helper.try_recv() {
            Ok(job) => batch.push(job),
            Err(_) => break,
        }
    }
    // Absolute moves that a later move of the same connection supersedes are skipped
    // instead of injected: this bounds how long a move flood delays everything else.
    let entries: Vec<(i32, bool)> = batch.iter().map(job_entry).collect();
    let keep: Vec<bool> = multi_control::superseded_moves(&entries)
        .into_iter()
        .map(|superseded| !superseded)
        .collect();
    for (job, keep) in batch.drain(..).zip(keep) {
        if keep {
            handle(job);
        }
    }
}

/// The coalescing view of a job: which connection it belongs to and whether it is an
/// absolute move, the only kind that may be merged away.
fn job_entry(job: &Job) -> (i32, bool) {
    match job {
        Job::Mouse { conn, evt, .. } => (
            *conn,
            evt.mask & MOUSE_TYPE_MASK == crate::input::MOUSE_TYPE_MOVE,
        ),
        _ => (0, false),
    }
}

fn handle(job: Job) {
    match job {
        Job::Mouse {
            conn,
            evt,
            username,
            argb,
            simulate,
            show_cursor,
        } => handle_mouse(conn, evt, username, argb, simulate, show_cursor),
        Job::Key { conn, evt, press } => handle_key(conn, evt, press),
        Job::Pointer { conn, evt } => {
            // Touch and pen have no per-event arbitration, so the only question is whether
            // this connection owns the pointer right now. Asking here keeps the answer in
            // the same order as the pointer it depends on.
            if multi_control::can_inject_pointer(conn) {
                input_service::handle_pointer(&evt, conn);
            } else {
                refuse(conn, Reject::NotOwner);
            }
        }
        Job::Borrow {
            conn,
            request,
            epoch,
        } => {
            let planned = multi_control::on_borrow(conn, request, epoch, Instant::now());
            execute(planned.commands);
            if let Action::Locate { x, y } = planned.action {
                locate(conn, x, y);
            }
            if let Action::Drop(reject) = planned.action {
                note_reject(conn, reject);
            }
        }
        Job::Register {
            conn,
            supported,
            can_inject,
        } => execute(multi_control::register(conn, supported, can_inject)),
        Job::Unregister { conn } => execute(multi_control::unregister(conn)),
        Job::CanInject { conn, can_inject } => {
            execute(multi_control::set_can_inject(conn, can_inject))
        }
        Job::Designate { conn } => execute(multi_control::set_primary(conn)),
    }
}

fn handle_mouse(
    conn: i32,
    evt: MouseEvent,
    username: String,
    argb: u32,
    simulate: bool,
    show_cursor: bool,
) {
    if !simulate {
        // The peer may not inject: keep its cursor UI up to date and stop here.
        input_service::handle_mouse(&evt, conn, username, argb, false, show_cursor);
        return;
    }
    let planned: Planned = multi_control::on_mouse(conn, &evt, Instant::now());
    execute(planned.commands);
    match planned.action {
        Action::Inject => {
            input_service::handle_mouse(&evt, conn, username, argb, true, show_cursor)
        }
        Action::Locate { x, y } => locate(conn, x, y),
        Action::LocateThenInject { x, y } => {
            locate(conn, x, y);
            input_service::handle_mouse(&evt, conn, username, argb, true, show_cursor);
        }
        Action::Track => {
            input_service::handle_mouse(&evt, conn, username, argb, false, show_cursor)
        }
        Action::Drop(reject) => note_reject(conn, reject),
    }
}

fn handle_key(conn: i32, evt: KeyEvent, press: bool) {
    // `press` is the connection's atomic-press flag, so it can not stand in for `down`:
    // in `Map` mode a key down and its release arrive with the flag clear.
    let input = multi_control::KeyInput::from_event(&evt, press);
    let planned = multi_control::on_key(conn, input, Instant::now());
    execute(planned.commands);
    match planned.action {
        Action::Inject | Action::LocateThenInject { .. } | Action::Locate { .. } => {
            input_service::handle_key_event(evt, press);
        }
        Action::Track => {}
        Action::Drop(reject) => note_reject(conn, reject),
    }
}

/// The event path of a connection stays the one `input_service` uses for keys, so a key
/// injected by the arbitration behaves exactly like one injected by the input thread.
fn locate(conn: i32, x: i32, y: i32) {
    let evt = MouseEvent {
        mask: crate::input::MOUSE_TYPE_MOVE,
        x,
        y,
        ..Default::default()
    };
    input_service::handle_mouse(&evt, conn, String::new(), 0, true, false);
}

/// Tells a connection that the input it just sent was refused, so its session UI can
/// explain why nothing happened.
pub fn refuse(conn: i32, reject: Reject) {
    note_reject(conn, reject);
}

fn note_reject(conn: i32, reject: Reject) {
    if reject == Reject::Suppressed {
        return;
    }
    log::debug!("#{} input refused: {:?}", conn, reject);
    notify(conn);
}

fn execute(commands: Vec<Command>) {
    for command in commands {
        match command {
            Command::Mouse {
                conn,
                evt,
                simulate,
            } => {
                let (username, argb, show_cursor) = identity(conn);
                input_service::handle_mouse(&evt, conn, username, argb, simulate, show_cursor);
            }
            Command::KeyRelease { key, .. } => {
                input_service::handle_key(&multi_control::key_release_event(key));
            }
            Command::Notify { conns } => {
                for conn in conns {
                    notify(conn);
                }
                notify_cm();
            }
        }
    }
}

fn identity(conn: i32) -> (String, u32, bool) {
    PEERS
        .lock()
        .unwrap()
        .get(&conn)
        .map(|peer| (peer.username.clone(), peer.argb, peer.show_cursor))
        .unwrap_or_default()
}

fn peer_name(conn: i32) -> String {
    PEERS
        .lock()
        .unwrap()
        .get(&conn)
        .map(|peer| peer.username.clone())
        .unwrap_or_default()
}

/// Tells the connection manager who owns the real pointer and who borrows it. The ids are
/// global, so one message per window is enough; a duplicate just sets the same value again.
fn notify_cm() {
    let primary = multi_control::primary_conn();
    let borrower = multi_control::borrower_conn();
    let mut sent: Vec<hbb_common::tokio::sync::mpsc::UnboundedSender<crate::ipc::Data>> =
        Vec::new();
    for sender in PEERS
        .lock()
        .unwrap()
        .values()
        .filter_map(|peer| peer.cm.clone())
    {
        // The window and the server are different processes; several connections of the
        // same window share one channel, and it only has to be told once.
        if sent.iter().any(|sent| sent.same_channel(&sender)) {
            continue;
        }
        sender
            .send(crate::ipc::Data::MultiControlRole { primary, borrower })
            .ok();
        sent.push(sender);
    }
}

/// Pushes the current arbitration state to a peer, so it can show its role and why its
/// last input was not applied.
fn notify(conn: i32) {
    let Some(sender) = PEERS
        .lock()
        .unwrap()
        .get(&conn)
        .map(|peer| peer.sender.clone())
    else {
        return;
    };
    let out = multi_control::snapshot(conn);
    // A refusal that repeats is the same state: sending it again would only add IPC
    // traffic for a flood of refused events, and the peer shows a notice only when it
    // changes anyway.
    if PEERS
        .lock()
        .unwrap()
        .get(&conn)
        .map_or(false, |peer| peer.last_out.as_ref() == Some(&out))
    {
        return;
    }
    let mut state = MultiControlState::new();
    state.enabled = out.enabled;
    state.is_primary = out.is_primary;
    state.borrowed_by_other = out.borrowed_by_other;
    state.borrowed_by_me = out.borrowed_by_me;
    state.borrower_name = out.borrower.map(peer_name).unwrap_or_default();
    state.keyboard_target_confirmed = out.keyboard_target_confirmed;
    state.epoch = out.epoch;
    state.notice = out.notice.clone();
    let mut misc = Misc::new();
    misc.set_multi_control_state(state);
    let mut msg = Message::new();
    msg.set_misc(misc);
    if sender
        .send((hbb_common::tokio::time::Instant::now(), Arc::new(msg)))
        .is_ok()
    {
        if let Some(peer) = PEERS.lock().unwrap().get_mut(&conn) {
            peer.last_out = Some(out);
        }
    }
}

/// A signature of the current display layout, used to notice a resolution or display
/// change without polling the displays from the arbitration module itself.
fn layout_signature() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match crate::server::display_service::try_get_displays() {
        Ok(displays) => {
            for display in displays.iter() {
                let (x, y) = display.origin();
                x.hash(&mut hasher);
                y.hash(&mut hasher);
                display.width().hash(&mut hasher);
                display.height().hash(&mut hasher);
            }
        }
        Err(err) => log::debug!("failed to read the displays: {}", err),
    }
    hasher.finish()
}
