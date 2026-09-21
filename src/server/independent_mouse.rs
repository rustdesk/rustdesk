//! Independent mouse positions for concurrent remote connections.
//!
//! Several peers can be connected to the same host at the same time. Instead of
//! moving the host pointer on every peer move - which makes the peers steal the
//! pointer from each other - every connection remembers its own position, and the
//! pointer is moved to it right before an event that has to land somewhere, such
//! as a click, a wheel step or a key press.
//!
//! The state lives in the process that dispatches input events (the main process),
//! which is also where the decision is made, so the injection paths stay unchanged
//! and never arbitrate again. See `docs/multi-control-plan.md`.

use crate::input::{
    MOUSE_BUTTON_BACK, MOUSE_BUTTON_FORWARD, MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT,
    MOUSE_BUTTON_WHEEL, MOUSE_TYPE_DOWN, MOUSE_TYPE_MASK, MOUSE_TYPE_MOVE,
    MOUSE_TYPE_MOVE_RELATIVE, MOUSE_TYPE_TRACKPAD, MOUSE_TYPE_UP, MOUSE_TYPE_WHEEL,
};
use base::{config::keys, message_proto::MouseEvent};
use hbb_common::{config::Config, log};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

/// The buttons this module can press and has to release again.
const BUTTONS: [i32; 5] = [
    MOUSE_BUTTON_LEFT,
    MOUSE_BUTTON_RIGHT,
    MOUSE_BUTTON_WHEEL,
    MOUSE_BUTTON_BACK,
    MOUSE_BUTTON_FORWARD,
];

/// A connection that stays silent for this long loses the pointer, so a peer that
/// dies in the middle of a drag cannot freeze the host pointer forever.
const OWNER_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

lazy_static::lazy_static! {
    static ref STATE: Mutex<State> = Default::default();
}

#[derive(Default)]
struct State {
    conns: HashMap<i32, ConnState>,
    /// The connection that currently keeps the host pointer, if it drags.
    owner: Option<i32>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct ConnState {
    /// Last absolute position the peer sent, in peer coordinates.
    pos: Option<(i32, i32)>,
    /// Buttons this peer pressed on the host and did not release yet.
    buttons: i32,
    last_input: Option<Instant>,
}

/// What the caller has to do with an incoming mouse event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    /// Only keep the peer position and the cursor UI up to date.
    Track,
    /// Move the host pointer to this position first, then inject the event.
    Locate { x: i32, y: i32 },
    /// Inject the event where the host pointer already is.
    Inject,
    /// Drop the event without touching the host.
    Drop,
}

/// A move event to the given position, in the coordinates the peer used, so the
/// coordinate conversions of the injection path apply to it as well.
pub fn position_event(x: i32, y: i32) -> MouseEvent {
    MouseEvent {
        mask: MOUSE_TYPE_MOVE,
        x,
        y,
        ..Default::default()
    }
}

/// Only `Y` turns the feature on: unset, empty and `N` all keep the old behavior.
pub fn enabled() -> bool {
    Config::get_option(keys::OPTION_INDEPENDENT_MOUSE) == "Y"
}

/// Records an incoming mouse event and reports what has to happen with it.
///
/// `can_inject` is false when the peer has no keyboard/mouse permission: the
/// position is still tracked for the cursor UI, but nothing may be pressed.
pub fn plan_mouse(conn: i32, can_inject: bool, evt: &MouseEvent) -> Plan {
    let mut state = STATE.lock().unwrap();
    plan_mouse_in(&mut state, conn, can_inject, evt)
}

/// The position to move the host pointer to before a key press, if the peer has
/// one and no other connection is dragging.
pub fn plan_key(conn: i32) -> Option<(i32, i32)> {
    let mut state = STATE.lock().unwrap();
    plan_key_in(&mut state, conn)
}

/// Forgets a closed connection, releasing the buttons it still holds.
pub fn on_conn_closed(conn: i32) -> Vec<(i32, MouseEvent)> {
    let mut state = STATE.lock().unwrap();
    take_conn(&mut state, conn)
}

/// Housekeeping for the input threads: releases a drag whose peer went silent, and
/// forgets all state once the option is turned off.
pub fn sweep() -> Vec<(i32, MouseEvent)> {
    let on = enabled();
    let mut state = STATE.lock().unwrap();
    if !on {
        return take_all(&mut state);
    }
    let Some(conn) = idle_owner(&state, Instant::now()) else {
        return Vec::new();
    };
    log::info!(
        "#{} kept the mouse idle for {}s, releasing it",
        conn,
        OWNER_IDLE_TIMEOUT.as_secs()
    );
    take_conn(&mut state, conn)
}

fn plan_mouse_in(state: &mut State, conn: i32, can_inject: bool, evt: &MouseEvent) -> Plan {
    let evt_type = evt.mask & MOUSE_TYPE_MASK;
    let buttons = evt.mask >> 3;
    let owned_by_self = state.owner.map_or(true, |owner| owner == conn);
    let st = state.conns.entry(conn).or_default();
    let plan = match decide(can_inject, evt_type, buttons, owned_by_self, st) {
        // `decide` only answers `Locate` when there is a position to move to.
        Decision::Locate => match st.pos {
            Some((x, y)) => Plan::Locate { x, y },
            None => Plan::Drop,
        },
        Decision::Track => Plan::Track,
        Decision::Inject => Plan::Inject,
        Decision::Drop => Plan::Drop,
    };
    match plan {
        Plan::Track => {
            st.pos = Some((evt.x, evt.y));
            st.last_input = Some(Instant::now());
        }
        Plan::Locate { .. } | Plan::Inject => {
            match evt_type {
                MOUSE_TYPE_DOWN => st.buttons |= buttons,
                MOUSE_TYPE_UP => st.buttons &= !buttons,
                _ => {}
            }
            st.last_input = Some(Instant::now());
            // Only a connection that really holds a button keeps the pointer.
            state.owner = if st.buttons == 0 { None } else { Some(conn) };
        }
        Plan::Drop => {}
    }
    plan
}

fn plan_key_in(state: &mut State, conn: i32) -> Option<(i32, i32)> {
    if state.owner.map_or(false, |owner| owner != conn) {
        return None;
    }
    let st = state.conns.get_mut(&conn)?;
    let pos = st.pos?;
    st.last_input = Some(Instant::now());
    Some(pos)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Decision {
    Track,
    Locate,
    Inject,
    Drop,
}

/// Pure arbitration: no state, no I/O, so every rule is unit testable.
fn decide(
    can_inject: bool,
    evt_type: i32,
    buttons: i32,
    owned_by_self: bool,
    st: &ConnState,
) -> Decision {
    if !can_inject {
        // Without permission nothing may be injected, but the position is still
        // needed to draw the peer cursor.
        return if evt_type == MOUSE_TYPE_MOVE {
            Decision::Track
        } else {
            Decision::Drop
        };
    }
    match evt_type {
        MOUSE_TYPE_MOVE => Decision::Track,
        // Deltas accumulate on the shared pointer and would move it under another
        // peer's hands, and the recorded position could not follow them.
        MOUSE_TYPE_MOVE_RELATIVE => Decision::Drop,
        MOUSE_TYPE_DOWN => {
            if buttons & known_buttons() != 0 && owned_by_self && st.pos.is_some() {
                Decision::Locate
            } else {
                Decision::Drop
            }
        }
        MOUSE_TYPE_UP => {
            if !owned_by_self {
                // Another connection is dragging: a release must not end it.
                Decision::Drop
            } else if st.pos.is_some() {
                Decision::Locate
            } else {
                // The press may have been accepted before this connection had a
                // position, or before the option was turned on - it has to be
                // released either way. Nobody else holds the pointer here, so this
                // can only ever let go of a button nobody else is dragging with.
                Decision::Inject
            }
        }
        MOUSE_TYPE_WHEEL | MOUSE_TYPE_TRACKPAD => {
            if owned_by_self && st.pos.is_some() {
                Decision::Locate
            } else {
                Decision::Drop
            }
        }
        _ => Decision::Drop,
    }
}

fn known_buttons() -> i32 {
    BUTTONS.iter().fold(0, |mask, button| mask | button)
}

fn idle_owner(state: &State, now: Instant) -> Option<i32> {
    let owner = state.owner?;
    let last_input = state.conns.get(&owner)?.last_input?;
    (now.saturating_duration_since(last_input) >= OWNER_IDLE_TIMEOUT).then_some(owner)
}

fn take_conn(state: &mut State, conn: i32) -> Vec<(i32, MouseEvent)> {
    let Some(st) = state.conns.remove(&conn) else {
        return Vec::new();
    };
    if state.owner == Some(conn) {
        state.owner = None;
    }
    release_events(conn, &st)
}

fn take_all(state: &mut State) -> Vec<(i32, MouseEvent)> {
    let mut events = Vec::new();
    for (conn, st) in state.conns.drain() {
        events.extend(release_events(conn, &st));
    }
    state.owner = None;
    events
}

fn release_events(conn: i32, st: &ConnState) -> Vec<(i32, MouseEvent)> {
    BUTTONS
        .iter()
        .filter(|button| st.buttons & **button != 0)
        .map(|button| {
            (
                conn,
                MouseEvent {
                    mask: (button << 3) | MOUSE_TYPE_UP,
                    ..Default::default()
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A state that holds the pointer since `last_input`.
    fn dragging_since(conn: i32, last_input: Instant) -> State {
        let mut state = State::default();
        state.conns.insert(
            conn,
            ConnState {
                pos: Some((1, 1)),
                buttons: LEFT,
                last_input: Some(last_input),
            },
        );
        state.owner = Some(conn);
        state
    }

    #[test]
    fn a_move_only_tracks_the_position() {
        for owned_by_self in [true, false] {
            for pos in [None, Some((10, 10))] {
                let st = ConnState {
                    pos,
                    ..Default::default()
                };
                assert_eq!(
                    decide(true, MOUSE_TYPE_MOVE, 0, owned_by_self, &st),
                    Decision::Track
                );
            }
        }
    }

    #[test]
    fn a_click_needs_a_position_and_is_located_first() {
        let at = ConnState {
            pos: Some((5, 6)),
            ..Default::default()
        };
        assert_eq!(
            decide(true, MOUSE_TYPE_DOWN, LEFT, true, &at),
            Decision::Locate
        );
        assert_eq!(
            decide(true, MOUSE_TYPE_DOWN, LEFT, true, &ConnState::default()),
            Decision::Drop
        );
        // A press without a known button is not an event we can act on.
        assert_eq!(decide(true, MOUSE_TYPE_DOWN, 0, true, &at), Decision::Drop);
    }

    #[test]
    fn wheel_values_are_never_read_as_coordinates() {
        let at = ConnState {
            pos: Some((1, 2)),
            ..Default::default()
        };
        assert_eq!(
            decide(true, MOUSE_TYPE_WHEEL, 0, true, &at),
            Decision::Locate
        );
        assert_eq!(
            decide(true, MOUSE_TYPE_TRACKPAD, 0, true, &at),
            Decision::Locate
        );
        // Without a position the scroll would land in another peer's window.
        assert_eq!(
            decide(true, MOUSE_TYPE_WHEEL, 0, true, &ConnState::default()),
            Decision::Drop
        );
    }

    #[test]
    fn relative_movement_is_dropped() {
        let at = ConnState {
            pos: Some((1, 2)),
            ..Default::default()
        };
        assert_eq!(
            decide(true, MOUSE_TYPE_MOVE_RELATIVE, 0, true, &at),
            Decision::Drop
        );
    }

    #[test]
    fn another_peers_drag_blocks_clicks_and_wheel() {
        let st = ConnState {
            pos: Some((3, 4)),
            ..Default::default()
        };
        assert_eq!(
            decide(true, MOUSE_TYPE_DOWN, LEFT, false, &st),
            Decision::Drop
        );
        assert_eq!(
            decide(true, MOUSE_TYPE_WHEEL, 0, false, &st),
            Decision::Drop
        );
        assert_eq!(
            decide(true, MOUSE_TYPE_TRACKPAD, 0, false, &st),
            Decision::Drop
        );
        // Its own move keeps being tracked, that is how a peer gets a position back.
        assert_eq!(
            decide(true, MOUSE_TYPE_MOVE, 0, false, &st),
            Decision::Track
        );
    }

    #[test]
    fn a_release_never_ends_another_peers_drag() {
        let dragging = ConnState {
            pos: Some((3, 4)),
            buttons: LEFT,
            last_input: None,
        };
        // Another connection owns the pointer.
        assert_eq!(
            decide(true, MOUSE_TYPE_UP, LEFT, false, &dragging),
            Decision::Drop
        );
        assert_eq!(
            decide(true, MOUSE_TYPE_UP, RIGHT, false, &dragging),
            Decision::Drop
        );
        assert_eq!(
            decide(true, MOUSE_TYPE_UP, LEFT, true, &dragging),
            Decision::Locate
        );
        // A release stays possible even if the press was never tracked.
        let untracked = ConnState {
            pos: None,
            ..Default::default()
        };
        assert_eq!(
            decide(true, MOUSE_TYPE_UP, LEFT, true, &untracked),
            Decision::Inject
        );
    }

    #[test]
    fn without_permission_nothing_is_injected() {
        let at = ConnState {
            pos: Some((1, 1)),
            ..Default::default()
        };
        assert_eq!(
            decide(false, MOUSE_TYPE_MOVE, 0, true, &at),
            Decision::Track
        );
        assert_eq!(
            decide(false, MOUSE_TYPE_DOWN, LEFT, true, &at),
            Decision::Drop
        );
        assert_eq!(
            decide(false, MOUSE_TYPE_UP, LEFT, true, &at),
            Decision::Drop
        );
        assert_eq!(
            decide(false, MOUSE_TYPE_WHEEL, 0, true, &at),
            Decision::Drop
        );
    }

    #[test]
    fn a_drag_keeps_the_pointer_until_the_last_button_is_released() {
        let (a, b) = (7, 8);
        let mut state = State::default();
        // No position yet: the first click is dropped, not sent to another peer.
        assert_eq!(
            plan_mouse_in(&mut state, a, true, &button(LEFT, MOUSE_TYPE_DOWN)),
            Plan::Drop
        );
        assert_eq!(
            plan_mouse_in(&mut state, a, true, &moved(11, 12)),
            Plan::Track
        );
        assert_eq!(
            plan_mouse_in(&mut state, a, true, &button(LEFT, MOUSE_TYPE_DOWN)),
            Plan::Locate { x: 11, y: 12 }
        );
        // A keeps the pointer, B can move but cannot interfere.
        assert_eq!(
            plan_mouse_in(&mut state, b, true, &moved(50, 50)),
            Plan::Track
        );
        assert_eq!(
            plan_mouse_in(&mut state, b, true, &button(LEFT, MOUSE_TYPE_DOWN)),
            Plan::Drop
        );
        assert_eq!(
            plan_mouse_in(&mut state, b, true, &button(LEFT, MOUSE_TYPE_WHEEL)),
            Plan::Drop
        );
        // Its release cannot end A's drag either.
        assert_eq!(
            plan_mouse_in(&mut state, b, true, &button(LEFT, MOUSE_TYPE_UP)),
            Plan::Drop
        );
        // A second button of its own does not let the drag go.
        assert_eq!(
            plan_mouse_in(&mut state, a, true, &button(RIGHT, MOUSE_TYPE_DOWN)),
            Plan::Locate { x: 11, y: 12 }
        );
        assert_eq!(
            plan_mouse_in(&mut state, a, true, &button(LEFT, MOUSE_TYPE_UP)),
            Plan::Locate { x: 11, y: 12 }
        );
        assert_eq!(
            plan_mouse_in(&mut state, b, true, &button(LEFT, MOUSE_TYPE_DOWN)),
            Plan::Drop
        );
        assert_eq!(
            plan_mouse_in(&mut state, a, true, &button(RIGHT, MOUSE_TYPE_UP)),
            Plan::Locate { x: 11, y: 12 }
        );
        // The drag is over, B works in its own position.
        assert_eq!(
            plan_mouse_in(&mut state, b, true, &button(LEFT, MOUSE_TYPE_DOWN)),
            Plan::Locate { x: 50, y: 50 }
        );
    }

    #[test]
    fn stray_releases_do_not_pollute_the_state() {
        let conn = 3;
        let mut state = State::default();
        assert_eq!(
            plan_mouse_in(&mut state, conn, true, &button(LEFT, MOUSE_TYPE_UP)),
            Plan::Inject
        );
        assert_eq!(state.owner, None);
        assert_eq!(take_conn(&mut state, conn).len(), 0);
        assert!(state.conns.is_empty());
    }

    #[test]
    fn a_key_press_only_moves_a_pointer_that_has_a_position() {
        let (a, b) = (4, 5);
        let mut state = State::default();
        assert_eq!(plan_key_in(&mut state, a), None);
        plan_mouse_in(&mut state, a, true, &moved(7, 8));
        assert_eq!(plan_key_in(&mut state, a), Some((7, 8)));
        // While another connection drags, the key must not pull the pointer away.
        plan_mouse_in(&mut state, b, true, &moved(30, 31));
        plan_mouse_in(&mut state, b, true, &button(LEFT, MOUSE_TYPE_DOWN));
        assert_eq!(plan_key_in(&mut state, a), None);
        assert_eq!(plan_key_in(&mut state, b), Some((30, 31)));
    }

    #[test]
    fn closing_a_connection_releases_only_its_own_buttons() {
        let (a, b) = (1, 2);
        let mut state = State::default();
        plan_mouse_in(&mut state, a, true, &moved(20, 21));
        plan_mouse_in(&mut state, b, true, &moved(30, 31));
        plan_mouse_in(&mut state, a, true, &button(LEFT, MOUSE_TYPE_DOWN));
        let releases = take_conn(&mut state, a);
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].0, a);
        assert_eq!(releases[0].1.mask, (LEFT << 3) | MOUSE_TYPE_UP);
        assert_eq!(state.owner, None);
        // B keeps its own position, A is gone.
        assert_eq!(plan_key_in(&mut state, b), Some((30, 31)));
        assert_eq!(plan_key_in(&mut state, a), None);
        assert_eq!(take_conn(&mut state, b).len(), 0);
    }

    #[test]
    fn an_idle_drag_is_given_up() {
        let conn = 6;
        let start = Instant::now();
        let mut state = dragging_since(conn, start);
        assert_eq!(idle_owner(&state, start + OWNER_IDLE_TIMEOUT / 2), None);
        assert_eq!(idle_owner(&state, start + OWNER_IDLE_TIMEOUT), Some(conn));
        let releases = take_conn(&mut state, conn);
        assert_eq!(releases.len(), 1);
        assert_eq!(state.owner, None);
    }

    #[test]
    fn turning_the_option_off_releases_everything() {
        let (a, b) = (9, 10);
        let mut state = State::default();
        plan_mouse_in(&mut state, a, true, &moved(2, 3));
        plan_mouse_in(&mut state, b, true, &moved(4, 5));
        plan_mouse_in(&mut state, a, true, &button(LEFT, MOUSE_TYPE_DOWN));
        plan_mouse_in(&mut state, a, true, &button(RIGHT, MOUSE_TYPE_DOWN));
        let releases = take_all(&mut state);
        assert_eq!(releases.len(), 2);
        assert!(releases.iter().all(|(conn, _)| *conn == a));
        assert!(state.conns.is_empty());
        assert_eq!(state.owner, None);
        assert!(take_all(&mut state).is_empty());
    }

    #[test]
    fn event_to_inject_is_built_from_the_stored_position() {
        let evt = position_event(12, -34);
        assert_eq!(evt.mask, MOUSE_TYPE_MOVE);
        assert_eq!((evt.x, evt.y), (12, -34));
    }
}
