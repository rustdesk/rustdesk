//! Isolated Wayland display probe: enumerates a compositor over a runtime-directory socket when
//! the environment names no endpoint (a greeter's `--server` and the root service are given no
//! compositor variables). Gated behind the `wayland_probe` feature so the base Wayland path is
//! untouched — a consumer that does not build the DRM login-screen backend never compiles this,
//! and `get_wayland_displays` keeps its original behavior of returning the connect error.

use super::{collect_wayland_displays, get_values_of_seat0_with_gdm_wayland, WaylandDisplayInfo};
use hbb_common::{bail, ResultType};
use sctk::reexports::client::Connection;
use std::path::{Path, PathBuf};

const RUNTIME_DIR_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// The argument the consumer binary must dispatch to `wayland_display_probe_child_main` before
/// any other startup work; see that function for why the probe is its own process.
pub const WAYLAND_DISPLAY_PROBE_ARG: &str = "--wayland-display-probe";

/// First stdout line of a probe child. A binary that does not dispatch the arg never prints it.
const WAYLAND_PROBE_MAGIC: &str = "wayland-display-probe-v1";

/// Latched on a failed handshake: a consumer that does not dispatch the probe arg runs its NORMAL
/// startup instead, and this path re-enters every enumeration cycle.
static PROBE_UNSUPPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static RUNTIME_DIR_PROBE_BUSY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Clears the in-flight flag on every exit path of the parent, error arms included.
struct ProbeBusyGuard;

impl Drop for ProbeBusyGuard {
    fn drop(&mut self) {
        RUNTIME_DIR_PROBE_BUSY.store(false, std::sync::atomic::Ordering::Release);
    }
}

/// Entry point of the isolated probe process. The consumer binary dispatches
/// `WAYLAND_DISPLAY_PROBE_ARG` here first, before config, logging or any other startup work.
///
/// Its own process because the release profile builds with panic=abort: sctk panics on malformed
/// protocol bytes, and in-process that abort takes the whole server down. Here it takes down only
/// this child, which the parent reports as a failed probe. The seat0 lookup also runs in here, so
/// the parent's single deadline bounds the loginctl reads too.
pub fn wayland_display_probe_child_main() -> ! {
    use std::io::Write;
    // The handshake first, so the parent can tell this entry point ran and not a consumer binary
    // that fell through to its normal startup.
    println!("{WAYLAND_PROBE_MAGIC}");
    let _ = std::io::stdout().flush();
    let code = match seat0_runtime_dir()
        .and_then(|dir| {
            drop_to_dir_owner(&dir)?;
            probe_runtime_dir(&dir)
        })
        .and_then(|displays| serde_json::to_string(&displays).map_err(anyhow::Error::from))
    {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(err) => {
            eprintln!("{err:#}");
            1
        }
    };
    let _ = std::io::stdout().flush();
    std::process::exit(code)
}

static ENDPOINT_WAS_NAMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Whether the environment ever named a wayland endpoint in this process. Empty is not a name.
///
/// Read before `connect_to_env`, which removes `WAYLAND_SOCKET` from the environment on both its
/// success and its bad-fd path; and latched, so a consumed variable cannot turn a process that WAS
/// pointed at a compositor into one that is free to go looking for another.
pub(super) fn env_names_wayland_endpoint() -> bool {
    use std::sync::atomic::Ordering;
    let named = ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
        .iter()
        .any(|key| std::env::var_os(key).is_some_and(|value| !value.is_empty()));
    if named {
        ENDPOINT_WAS_NAMED.store(true, Ordering::Release);
    }
    ENDPOINT_WAS_NAMED.load(Ordering::Acquire)
}

/// The probe parses compositor-controlled protocol data; a root service must not do that as
/// root. Before touching the socket, become the runtime directory's owner — and refuse to probe
/// at all if the drop fails, since staying root is the one unacceptable outcome.
fn drop_to_dir_owner(dir: &Path) -> ResultType<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Ok(());
    }
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(dir)?;
    let (uid, gid) = (meta.uid(), meta.gid());
    if uid == 0 {
        // Root's own session: there is no boundary to cross and nothing to drop to.
        return Ok(());
    }
    unsafe {
        if libc::setgroups(0, std::ptr::null()) != 0
            || libc::setgid(gid) != 0
            || libc::setuid(uid) != 0
            || libc::setuid(0) == 0
        {
            bail!("could not drop privileges for the socket probe");
        }
    }
    Ok(())
}

/// `/run/user/<uid>` of the active seat0 session, a greeter included.
///
/// Derived from the uid rather than read from `XDG_RUNTIME_DIR`: the root service is given no such
/// variable, and `get_home_dir_trusted` refuses to trust the environment for the same reason.
fn seat0_runtime_dir() -> ResultType<PathBuf> {
    let uid = get_values_of_seat0_with_gdm_wayland(&[1]).remove(0);
    if uid.is_empty() || !uid.bytes().all(|b| b.is_ascii_digit()) {
        bail!("no active seat0 session to take a runtime directory from");
    }
    Ok(PathBuf::from(format!("/run/user/{uid}")))
}

/// The wayland sockets present in `dir`, lowest display number first.
///
/// Scanned rather than guessed: `wl_display_add_socket_auto` takes the first FREE name up to
/// `wayland-32`, and a greeter is where leftovers accumulate across compositor restarts. Only that
/// name pattern, because the same directory holds pipewire and dbus sockets.
fn wayland_sockets_in(dir: &Path) -> Vec<PathBuf> {
    use std::os::unix::fs::FileTypeExt;
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with("wayland-")
                    && !name.ends_with(".lock")
                    && entry.file_type().map(|t| t.is_socket()).unwrap_or(false)
            })
            .map(|entry| entry.path())
            .collect(),
        Err(_) => Vec::new(),
    };
    paths.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("wayland-"))
            .and_then(|number| number.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    paths
}

/// Enumerate through a socket in the seat0 runtime directory, for the case where nothing named an
/// endpoint: a greeter's `--server` and the root service are given no compositor variables, so
/// nothing tells the enumerator where a compositor that IS running lives. An endpoint that WAS
/// named and failed must not silently reattach to a different compositor.
///
/// In a subprocess and bounded, because the caller holds a process-wide lock across the call while
/// `connect(2)` parks on a full backlog and sctk's roundtrip polls without a deadline; and because
/// sctk panics on malformed output events, which the release profile's panic=abort turns into an
/// abort of the whole server. A child dies alone, and on the deadline it is killed instead of
/// leaking a thread. The seat0 lookup runs inside the child, under the same deadline.
pub(super) fn wayland_displays_from_runtime_dir(
    named_endpoint: bool,
) -> ResultType<Vec<WaylandDisplayInfo>> {
    use std::sync::atomic::Ordering;
    if named_endpoint {
        bail!("an explicit wayland endpoint is set and did not connect");
    }
    if PROBE_UNSUPPORTED.load(Ordering::Acquire) {
        bail!("this binary does not dispatch {WAYLAND_DISPLAY_PROBE_ARG}");
    }
    if RUNTIME_DIR_PROBE_BUSY.swap(true, Ordering::AcqRel) {
        bail!("an earlier probe has not returned");
    }
    let _busy = ProbeBusyGuard;
    let exe = std::env::current_exe()?;
    // Its own process group, so the deadline can kill loginctl descendants along with the child,
    // and so no surviving descendant can hold the pipes open past the reads below.
    use std::os::unix::process::CommandExt;
    let mut child = std::process::Command::new(exe)
        .arg(WAYLAND_DISPLAY_PROBE_ARG)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0)
        .spawn()?;
    let probe_pgid = child.id() as libc::pid_t;
    let kill_probe_group = || unsafe {
        let _ = libc::kill(-probe_pgid, libc::SIGKILL);
    };
    let deadline = std::time::Instant::now() + RUNTIME_DIR_PROBE_TIMEOUT;
    let status = loop {
        match child.try_wait()? {
            Some(status) => {
                kill_probe_group();
                break status;
            }
            None if std::time::Instant::now() >= deadline => {
                kill_probe_group();
                // The direct pid too, not only its group: if the child left the group its own
                // kill would miss it, and the wait below would then block on a live child. A
                // pid-targeted SIGKILL is uncatchable, so wait() is bounded either way.
                let _ = child.kill();
                let _ = child.wait();
                // An unwired binary runs its normal startup, and a long-running one (the
                // server itself) lands HERE rather than at the handshake check below — latch
                // on this path too, or every enumeration cycle spawns a full consumer
                // process. Judged by what the child already wrote: a real probe prints the
                // magic line first and flushes, so its absence after a whole deadline means
                // this is not a probe. Only buffered bytes are read — a blocking read could
                // hang on a grandchild that inherited the write end.
                match first_buffered_line(child.stdout.take()) {
                    // The pipe could not be inspected at all: no evidence, no latch.
                    None => {
                        bail!("the wayland socket probe timed out and its output was uninspectable")
                    }
                    Some(head) if head.as_deref() == Some(WAYLAND_PROBE_MAGIC) => {
                        bail!("the wayland socket probe did not answer and was killed");
                    }
                    Some(_) => {
                        PROBE_UNSUPPORTED.store(true, Ordering::Release);
                        bail!("the wayland socket probe timed out without the handshake; probe disabled");
                    }
                }
            }
            None => std::thread::sleep(std::time::Duration::from_millis(25)),
        }
    };
    // Drained non-blocking, not read_to_string: the child exited so its output is already
    // buffered, but a descendant that escaped the process group could still hold a write end open
    // and an EOF-seeking read would then hang here forever.
    let stdout = drain_nonblocking(child.stdout.take()).unwrap_or_default();
    let stderr = drain_nonblocking(child.stderr.take()).unwrap_or_default();
    let mut lines = stdout.lines();
    if lines.next() != Some(WAYLAND_PROBE_MAGIC) {
        // Not a probe: the binary ran its normal startup. Latch, or this path would spawn one
        // full consumer process per enumeration cycle.
        PROBE_UNSUPPORTED.store(true, Ordering::Release);
        bail!("this binary does not dispatch {WAYLAND_DISPLAY_PROBE_ARG}; probe disabled");
    }
    if !status.success() {
        let detail = stderr.trim();
        if detail.is_empty() {
            // panic=abort or a signal leaves stderr empty; the status is then the only cause.
            bail!("wayland socket probe failed: {status}");
        }
        bail!("wayland socket probe failed ({status}): {detail}");
    }
    let displays: Vec<WaylandDisplayInfo> =
        match serde_json::from_str(lines.next().unwrap_or_default()) {
            Ok(displays) => displays,
            Err(err) => bail!("wayland socket probe answered a malformed list: {err}"),
        };
    // The child already refuses an empty list; refuse it here too, so a truncated pipe cannot
    // become a cached-for-life empty enumeration.
    if displays.is_empty() {
        bail!("wayland socket probe returned no outputs");
    }
    log::debug!(
        "wayland: {} output(s) via the probe subprocess",
        displays.len()
    );
    Ok(displays)
}

/// Everything already buffered in the pipe, read strictly non-blocking and capped: a descendant
/// that escaped the probe's process group can hold a write end open, so a blocking read (even
/// after the child exits) could hang the enumeration forever. `None` means the pipe could not be
/// INSPECTED (missing handle or fcntl failure) and must not be read as evidence of anything;
/// `Some` is whatever bytes were buffered, whether or not EOF arrived.
fn drain_nonblocking<R: std::io::Read + std::os::fd::AsRawFd>(pipe: Option<R>) -> Option<String> {
    let mut pipe = pipe?;
    let fd = pipe.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return None;
        }
    }
    // Capped so a descendant that keeps writing cannot spin this read forever.
    const CAP: usize = 64 * 1024;
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break, // EOF: the write end is fully closed
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if out.len() >= CAP {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            // WouldBlock: what is buffered is drained (a descendant may still hold the writer).
            // Any other error: stop with what we have.
            Err(_) => break,
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// The first line the child buffered, for the timeout latch decision. `Some(None)` is an
/// inspected-but-empty buffer (genuine absence of the handshake); outer `None` is uninspectable.
fn first_buffered_line(pipe: Option<std::process::ChildStdout>) -> Option<Option<String>> {
    drain_nonblocking(pipe).map(|s| s.lines().next().map(str::to_owned))
}

fn probe_runtime_dir(dir: &Path) -> ResultType<Vec<WaylandDisplayInfo>> {
    use std::os::unix::net::UnixStream;
    let mut errs = Vec::new();
    for path in wayland_sockets_in(dir) {
        match UnixStream::connect(&path)
            .map_err(anyhow::Error::from)
            .and_then(|s| Connection::from_socket(s).map_err(anyhow::Error::from))
            .and_then(|conn| collect_wayland_displays(&conn))
        {
            // The caller caches an empty list as ground truth for the process lifetime, and a
            // compositor still probing its monitors is exactly what this path connects to.
            Ok(displays) if displays.is_empty() => {
                errs.push(format!("{}: no outputs yet", path.display()))
            }
            Ok(displays) => {
                // Which socket answered, when nothing in the environment named one.
                log::debug!(
                    "wayland: {} output(s) from {}, found by scanning",
                    displays.len(),
                    path.display()
                );
                return Ok(displays);
            }
            Err(err) => errs.push(format!("{}: {err}", path.display())),
        }
    }
    bail!(
        "no usable wayland socket in {} ({})",
        dir.display(),
        if errs.is_empty() {
            "none present".to_owned()
        } else {
            errs.join("; ")
        }
    )
}
