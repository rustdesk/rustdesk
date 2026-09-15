use hbb_common::{
    bail, log,
    tokio::sync::{oneshot, OwnedSemaphorePermit, Semaphore},
    ResultType,
};
use std::{
    collections::HashSet,
    sync::{mpsc, Arc},
    thread,
};
use virtual_display::macos as native;

const MAX_PENDING_OPERATIONS: usize = 8;

enum Command {
    Resize {
        conn_id: i32,
        display_id: u32,
        width: u32,
        height: u32,
        scale: u32,
        reply: oneshot::Sender<ResultType<()>>,
        _permit: OwnedSemaphorePermit,
    },
    Connected(i32),
    Disconnected(i32),
    Toggle {
        conn_id: i32,
        index: i32,
        on: bool,
        reply: oneshot::Sender<ResultType<()>>,
        _permit: OwnedSemaphorePermit,
    },
}

lazy_static::lazy_static! {
    static ref WORKER: Result<mpsc::Sender<Command>, String> = start_worker();
    static ref PENDING_OPERATIONS: Arc<Semaphore> = Arc::new(Semaphore::new(MAX_PENDING_OPERATIONS));
}

// Bound native mutations without blocking connection lifecycle notifications.
fn operation_permit(operations: &Arc<Semaphore>) -> ResultType<OwnedSemaphorePermit> {
    operations
        .clone()
        .try_acquire_owned()
        .map_err(|_| hbb_common::anyhow::anyhow!("Display settings are busy. Try again."))
}

fn start_worker() -> Result<mpsc::Sender<Command>, String> {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("macos-virtual-display".into())
        .spawn(move || {
            run_worker(
                receiver,
                native::toggle,
                |display_id, width, height, scale| {
                    if scale == 0 {
                        native::resize(display_id, width, height)
                    } else {
                        native::configure(display_id, width, height, scale)
                    }
                },
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(sender)
}

// Serialize connection lifetimes with native operations: a delayed disconnect
// must not remove displays created by a new connection, or leave an in-flight
// creation alive after the last connection closes.
fn run_worker(
    receiver: mpsc::Receiver<Command>,
    mut toggle: impl FnMut(i32, bool) -> bool,
    mut resize: impl FnMut(u32, u32, u32, u32) -> bool,
) {
    let mut connections = HashSet::new();
    for command in receiver {
        match command {
            Command::Resize {
                conn_id,
                display_id,
                width,
                height,
                scale,
                reply,
                _permit,
            } => {
                let result = (|| {
                    if !connections.contains(&conn_id) {
                        bail!("Remote connection has closed");
                    }
                    if !resize(display_id, width, height, scale) {
                        bail!("Failed to resize macOS virtual display: {display_id}, {width}x{height}");
                    }
                    Ok(())
                })();
                if reply.send(result).is_err() {
                    log::debug!(
                        "Virtual display resize requester disconnected before receiving the result"
                    );
                }
            }
            Command::Connected(conn_id) => {
                connections.insert(conn_id);
            }
            Command::Disconnected(conn_id) => {
                if connections.remove(&conn_id) && connections.is_empty() && !toggle(-1, false) {
                    log::error!("Failed to remove macOS virtual displays");
                }
            }
            Command::Toggle {
                conn_id,
                index,
                on,
                reply,
                _permit,
            } => {
                let result = (|| {
                    if !connections.contains(&conn_id) {
                        bail!("Remote connection has closed");
                    }
                    if !toggle(index, on) {
                        bail!("Failed to toggle macOS virtual display: {index}");
                    }
                    Ok(())
                })();
                if reply.send(result).is_err() {
                    log::debug!(
                        "Virtual display requester disconnected before receiving the result"
                    );
                }
            }
        }
    }
}

fn send(command: Command) -> ResultType<()> {
    match WORKER.as_ref() {
        Ok(sender) => sender
            .send(command)
            .map_err(|error| hbb_common::anyhow::anyhow!(error.to_string())),
        Err(error) => bail!("Failed to start macOS virtual display worker: {error}"),
    }
}

pub fn on_connection_open(conn_id: i32) {
    if !native::is_supported() {
        return;
    }
    if let Err(error) = send(Command::Connected(conn_id)) {
        log::error!("{error}");
    }
}

pub fn on_connection_close(conn_id: i32) {
    if !native::is_supported() {
        return;
    }
    if let Err(error) = send(Command::Disconnected(conn_id)) {
        log::error!("{error}");
    }
}

pub fn is_virtual_display_supported() -> bool {
    native::is_supported() && WORKER.is_ok()
}

pub fn get_virtual_displays() -> Vec<i32> {
    let mask = native::active_mask();
    (1..=4).filter(|index| mask & (1 << index) != 0).collect()
}

pub fn display_mode(display_id: u32) -> Option<native::DisplayMode> {
    native::display_mode(display_id)
}

pub fn owns_display(name: &str) -> bool {
    name.parse()
        .map(native::owns_display)
        .unwrap_or(false)
}

pub fn configure(
    conn_id: i32,
    name: &str,
    width: i32,
    height: i32,
    scale: u32,
) -> ResultType<oneshot::Receiver<ResultType<()>>> {
    if !(320..=4096).contains(&width) || !(320..=4096).contains(&height) {
        bail!("Virtual display dimensions must be between 320 and 4096");
    }
    if !matches!(scale, 1 | 2) || width % scale as i32 != 0 || height % scale as i32 != 0 {
        bail!("Virtual display scale must be 1 or 2 and divide both pixel dimensions");
    }
    queue_configuration(conn_id, name, width, height, scale)
}

pub fn resize(
    conn_id: i32,
    name: &str,
    width: i32,
    height: i32,
) -> ResultType<oneshot::Receiver<ResultType<()>>> {
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        bail!("Virtual display dimensions must be between 1 and 4096 logical pixels");
    }
    queue_configuration(conn_id, name, width, height, 0)
}

fn queue_configuration(
    conn_id: i32,
    name: &str,
    width: i32,
    height: i32,
    scale: u32,
) -> ResultType<oneshot::Receiver<ResultType<()>>> {
    let (reply, result) = oneshot::channel();
    send(Command::Resize {
        conn_id,
        display_id: name.parse()?,
        width: width as _,
        height: height as _,
        scale,
        reply,
        _permit: operation_permit(&PENDING_OPERATIONS)?,
    })?;
    Ok(result)
}

pub async fn toggle(conn_id: i32, index: i32, on: bool) -> ResultType<()> {
    let (reply, result) = oneshot::channel();
    send(Command::Toggle {
        conn_id,
        index,
        on,
        reply,
        _permit: operation_permit(&PENDING_OPERATIONS)?,
    })?;
    result.await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_resize_validates_logical_dimensions() {
        let pending = resize(i32::MIN, "0", 160, 160)
            .expect("160 logical pixels can represent 320 output pixels at 200%");
        // An unregistered connection is rejected before any native display call.
        assert!(pending.blocking_recv().unwrap().is_err());
        assert!(resize(i32::MIN, "0", 0, 160).is_err());
        assert!(resize(i32::MIN, "0", 4097, 160).is_err());
    }

    fn request(conn_id: i32, index: i32, on: bool) -> (Command, oneshot::Receiver<ResultType<()>>) {
        let (reply, result) = oneshot::channel();
        (
            Command::Toggle {
                conn_id,
                index,
                on,
                reply,
                _permit: test_permit(),
            },
            result,
        )
    }

    fn test_permit() -> OwnedSemaphorePermit {
        operation_permit(&Arc::new(Semaphore::new(1))).unwrap()
    }

    #[test]
    fn saturated_operations_preserve_lifecycle_events_and_release_capacity() {
        use std::cell::Cell;
        let operations = Arc::new(Semaphore::new(MAX_PENDING_OPERATIONS));
        let (sender, receiver) = mpsc::channel();
        let mut results = Vec::new();
        sender.send(Command::Connected(1)).unwrap();
        for index in 0..MAX_PENDING_OPERATIONS {
            let (reply, result) = oneshot::channel();
            let _permit = operation_permit(&operations).unwrap();
            let command = if index % 2 == 0 {
                Command::Resize {
                    conn_id: 1,
                    display_id: 42,
                    width: 1920,
                    height: 1080,
                    scale: 1,
                    reply,
                    _permit,
                }
            } else {
                Command::Toggle {
                    conn_id: 1,
                    index: 1,
                    on: true,
                    reply,
                    _permit,
                }
            };
            sender.send(command).unwrap();
            results.push(result);
        }
        assert_eq!(
            operation_permit(&operations).unwrap_err().to_string(),
            "Display settings are busy. Try again."
        );
        for command in [
            Command::Connected(2),
            Command::Disconnected(1),
            Command::Disconnected(2),
        ] {
            sender.send(command).unwrap();
        }
        drop(sender);
        let completed = Cell::new(0);
        let mut cleaned_up = false;
        run_worker(
            receiver,
            |index, on| {
                assert_eq!(operations.available_permits(), completed.get());
                if index == -1 {
                    assert!(!on);
                    assert_eq!(completed.get(), MAX_PENDING_OPERATIONS);
                    cleaned_up = true;
                    true
                } else {
                    completed.set(completed.get() + 1);
                    false
                }
            },
            |_, _, _, _| {
                assert_eq!(operations.available_permits(), completed.get());
                completed.set(completed.get() + 1);
                true
            },
        );
        for (index, result) in results.into_iter().enumerate() {
            assert_eq!(result.blocking_recv().unwrap().is_ok(), index % 2 == 0);
        }
        assert!(cleaned_up);
        assert_eq!(operations.available_permits(), MAX_PENDING_OPERATIONS);
    }

    #[test]
    fn stopped_worker_releases_operation_capacity() {
        let operations = Arc::new(Semaphore::new(1));
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        assert!(sender
            .send(Command::Toggle {
                conn_id: 1,
                index: 1,
                on: true,
                reply: oneshot::channel().0,
                _permit: operation_permit(&operations).unwrap(),
            })
            .is_err());
        assert_eq!(operations.available_permits(), 1);
    }

    #[test]
    fn disconnect_preserves_other_connections_and_rejects_stale_requests() {
        let (sender, receiver) = mpsc::channel();
        let mut operations = Vec::new();
        let (first, first_result) = request(1, 1, true);
        let (stale, stale_result) = request(1, 2, true);
        let (second, second_result) = request(2, 3, true);
        for command in [
            Command::Connected(1),
            first,
            Command::Connected(2),
            Command::Disconnected(1),
            stale,
            second,
            Command::Disconnected(2),
        ] {
            sender.send(command).unwrap();
        }
        drop(sender);
        run_worker(
            receiver,
            |index, on| {
                operations.push((index, on));
                true
            },
            |_, _, _, _| panic!("Unexpected resize"),
        );
        assert!(first_result.blocking_recv().unwrap().is_ok());
        assert!(stale_result.blocking_recv().unwrap().is_err());
        assert!(second_result.blocking_recv().unwrap().is_ok());
        assert_eq!(operations, [(1, true), (3, true), (-1, false)]);
    }

    #[test]
    fn reconnect_after_last_disconnect_cleans_before_creating() {
        let (sender, receiver) = mpsc::channel();
        let mut operations = Vec::new();
        let (first, first_result) = request(1, 1, true);
        let (second, second_result) = request(2, 1, true);
        // A cancelled waiter must not skip the queued disconnect cleanup.
        drop(first_result);
        for command in [
            Command::Connected(1),
            first,
            Command::Disconnected(1),
            Command::Connected(2),
            second,
            Command::Disconnected(1),
            Command::Disconnected(2),
        ] {
            sender.send(command).unwrap();
        }
        drop(sender);
        run_worker(
            receiver,
            |index, on| {
                operations.push((index, on));
                true
            },
            |_, _, _, _| panic!("Unexpected resize"),
        );
        assert!(second_result.blocking_recv().unwrap().is_ok());
        assert_eq!(operations, [(1, true), (-1, false), (1, true), (-1, false)]);
    }

    #[test]
    fn resize_is_serialized_with_disconnect_and_rejects_closed_connections() {
        use std::cell::RefCell;
        let (sender, receiver) = mpsc::channel();
        let operations = RefCell::new(Vec::new());
        let resize = || Command::Resize {
            conn_id: 1,
            display_id: 42,
            width: 2560,
            height: 1600,
            scale: 2,
            reply: oneshot::channel().0,
            _permit: test_permit(),
        };
        for command in [
            Command::Connected(1),
            resize(),
            Command::Disconnected(1),
            resize(),
        ] {
            sender.send(command).unwrap();
        }
        drop(sender);
        run_worker(
            receiver,
            |_, _| {
                operations.borrow_mut().push("cleanup");
                true
            },
            |id, width, height, scale| {
                assert_eq!(scale, 2);
                assert_eq!((id, width, height), (42, 2560, 1600));
                operations.borrow_mut().push("resize");
                true
            },
        );
        assert_eq!(*operations.borrow(), ["resize", "cleanup"]);
    }

    #[test]
    fn invalid_native_modes_are_rejected_before_queueing() {
        for (width, height, scale) in [
            (0, 1600, 2),
            (4097, 1600, 1),
            (2559, 1600, 2),
            (2560, 1601, 2),
            (2560, 1600, 0),
            (2560, 1600, 3),
        ] {
            assert!(configure(1, "42", width, height, scale).is_err());
        }
    }

    #[test]
    fn native_mode_errors_and_closed_connections_reach_requesters() {
        let (sender, receiver) = mpsc::channel();
        let (first_reply, first_result) = oneshot::channel();
        let (stale_reply, stale_result) = oneshot::channel();
        sender.send(Command::Connected(1)).unwrap();
        sender
            .send(Command::Resize {
                conn_id: 1,
                display_id: 42,
                width: 2560,
                height: 1600,
                scale: 2,
                reply: first_reply,
                _permit: test_permit(),
            })
            .unwrap();
        sender.send(Command::Disconnected(1)).unwrap();
        sender
            .send(Command::Resize {
                conn_id: 1,
                display_id: 42,
                width: 2560,
                height: 1600,
                scale: 2,
                reply: stale_reply,
                _permit: test_permit(),
            })
            .unwrap();
        drop(sender);
        let mut calls = 0;
        run_worker(
            receiver,
            |_, _| true,
            |_, _, _, _| {
                calls += 1;
                false
            },
        );
        assert_eq!(calls, 1);
        assert!(first_result
            .blocking_recv()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("Failed to resize"));
        assert!(stale_result
            .blocking_recv()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("closed"));
    }

    #[test]
    fn native_failure_is_reported_and_cleanup_is_still_attempted() {
        let (sender, receiver) = mpsc::channel();
        let mut operations = Vec::new();
        let (command, result) = request(1, 1, true);
        for command in [Command::Connected(1), command, Command::Disconnected(1)] {
            sender.send(command).unwrap();
        }
        drop(sender);
        run_worker(
            receiver,
            |index, on| {
                operations.push((index, on));
                !on
            },
            |_, _, _, _| panic!("Unexpected resize"),
        );
        assert!(result.blocking_recv().unwrap().is_err());
        assert_eq!(operations, [(1, true), (-1, false)]);
    }
}
