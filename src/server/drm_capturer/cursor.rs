use super::DrmCursorData;
use dbus::{
    arg::{PropMap, Variant},
    blocking::Connection,
    message::MatchRule,
    Path,
};
use hbb_common::{anyhow::anyhow, bail, log, tokio, ResultType};
use std::{
    sync::{
        mpsc::{self, Receiver, Sender, TryRecvError},
        Arc, Mutex, MutexGuard,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod ffi;
mod metadata;
mod pipewire;
#[cfg(test)]
mod tests;

const BUS: &str = "org.gnome.Mutter.ScreenCast";
const SESSION_INTERFACE: &str = "org.gnome.Mutter.ScreenCast.Session";
const STREAM_INTERFACE: &str = "org.gnome.Mutter.ScreenCast.Stream";
const CURSOR_METADATA_MODE: u32 = 2;
const DBUS_TIMEOUT: Duration = Duration::from_secs(2);
const START_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub(super) type WireCursor = (u64, u32, u32, i32, i32, Vec<u8>);

pub struct Capture {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
    ready: Arc<Mutex<bool>>,
    pub(super) wire_cursor: Option<WireCursor>,
}

impl Capture {
    pub async fn start(display: i32, epoch: u64, connector: String) -> ResultType<Option<Self>> {
        if !tokio::task::spawn_blocking(mutter_available).await?? {
            return Ok(None);
        }
        let (stop, receiver) = mpsc::channel();
        let error = Arc::new(Mutex::new(None));
        let worker_error = error.clone();
        let ready = Arc::new(Mutex::new(false));
        let worker_ready = ready.clone();
        let thread = thread::Builder::new()
            .name("drm-cursor".into())
            .spawn(move || {
                let result = run(connector, receiver, move |cursor| {
                    publish((display, epoch), &worker_ready, cursor);
                });
                if let Err(error) = result {
                    log::error!("drm: Mutter cursor capture failed: {error:#}");
                    *worker_error.lock().unwrap() =
                        Some(format!("Mutter cursor capture: {error:#}"));
                }
            })?;
        Ok(Some(Self {
            stop: Some(stop),
            thread: Some(thread),
            error,
            ready,
            wire_cursor: None,
        }))
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }

    pub fn ready(&self) -> MutexGuard<'_, bool> {
        self.ready.lock().unwrap()
    }

    pub async fn stop(mut self) -> ResultType<()> {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            tokio::task::spawn_blocking(move || {
                thread
                    .join()
                    .map_err(|_| anyhow!("Mutter cursor worker panicked"))
            })
            .await??;
        }
        Ok(())
    }
}

fn mutter_available() -> ResultType<bool> {
    // Service-spawned servers have a session bus, but no XDG_CURRENT_DESKTOP.
    let conn = Connection::new_session()?;
    let (available,): (bool,) = conn
        .with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            DBUS_TIMEOUT,
        )
        .method_call("org.freedesktop.DBus", "NameHasOwner", (BUS,))?;
    Ok(available)
}

fn stopped(receiver: &Receiver<()>) -> bool {
    !matches!(receiver.try_recv(), Err(TryRecvError::Empty))
}

fn publish(target: (i32, u64), ready: &Mutex<bool>, cursor: DrmCursorData) {
    // Serialize source switching with wire publication, including pending replay.
    let mut ready = ready.lock().unwrap();
    if !*ready {
        log::info!("drm: using Mutter cursor metadata for display {}", target.0);
    }
    *ready = true;
    // These sprites already have the monitor's upright orientation and physical scale.
    super::set_drm_cursor(target.0, target.1, cursor);
}

fn run(
    connector: String,
    stop: Receiver<()>,
    publish: impl FnMut(DrmCursorData) + Send + 'static,
) -> ResultType<()> {
    let session = Session::new()?;
    let Some(node) = session.start(&super::normalize_connector(&connector), &stop)? else {
        return Ok(());
    };
    let stream = pipewire::Stream::new(node, publish)?;
    let started = Instant::now();
    while !stopped(&stop) {
        if !stream.received()? && started.elapsed() >= START_TIMEOUT {
            bail!("Timed out waiting for PipeWire cursor metadata");
        }
        session.conn.process(POLL_INTERVAL)?;
    }
    // Drop order closes PipeWire before stopping its Mutter session.
    Ok(())
}

struct Session {
    conn: Connection,
    path: Path<'static>,
}

impl Session {
    fn new() -> ResultType<Self> {
        let conn = Connection::new_session()?;
        let (path,): (Path<'static>,) = conn
            .with_proxy(BUS, "/org/gnome/Mutter/ScreenCast", DBUS_TIMEOUT)
            .method_call(BUS, "CreateSession", (PropMap::new(),))?;
        Ok(Self { conn, path })
    }

    fn start(&self, connector: &str, stop: &Receiver<()>) -> ResultType<Option<u32>> {
        let mut options = PropMap::new();
        options.insert(
            "cursor-mode".into(),
            Variant(Box::new(CURSOR_METADATA_MODE)),
        );
        let proxy = self.conn.with_proxy(BUS, self.path.clone(), DBUS_TIMEOUT);
        let (stream,): (Path<'static>,) =
            proxy.method_call(SESSION_INTERFACE, "RecordMonitor", (connector, options))?;
        let (sender, receiver) = mpsc::channel();
        let rule = MatchRule::new_signal(STREAM_INTERFACE, "PipeWireStreamAdded")
            .with_sender(BUS)
            .with_path(stream);
        self.conn
            .add_match(rule, move |(node,): (u32,), _, _| sender.send(node).is_ok())?;
        proxy.method_call::<(), _, _, _>(SESSION_INTERFACE, "Start", ())?;
        let started = Instant::now();
        while !stopped(stop) {
            match receiver.try_recv() {
                Ok(node) => return Ok(Some(node)),
                Err(TryRecvError::Disconnected) => bail!("Mutter cursor node subscription closed"),
                Err(TryRecvError::Empty) => {}
            }
            if started.elapsed() >= START_TIMEOUT {
                bail!("Timed out waiting for the Mutter cursor node");
            }
            self.conn.process(POLL_INTERVAL)?;
        }
        Ok(None)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let result: Result<(), _> = self
            .conn
            .with_proxy(BUS, self.path.clone(), DBUS_TIMEOUT)
            .method_call(SESSION_INTERFACE, "Stop", ());
        if let Err(error) = result {
            log::error!("drm: could not stop the Mutter cursor session: {error}");
        }
    }
}
