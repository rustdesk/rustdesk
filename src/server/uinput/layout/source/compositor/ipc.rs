use hbb_common::{anyhow::anyhow, bail, libc, ResultType};
use serde_json::Value;
use std::{
    env, fs,
    io::{self, Read, Write},
    mem::size_of,
    os::{fd::AsRawFd, unix::net::UnixStream},
    path::PathBuf,
};
use wayland_client::Connection;

const SWAY_MAGIC: &[u8] = b"i3-ipc";
const SWAY_GET_INPUTS: u32 = 100;
const EMPTY_PAYLOAD: u32 = 0;
const MAX_REPLY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Sway,
    Hyprland,
}

pub(super) fn read() -> ResultType<Option<(Kind, Value)>> {
    if env::var_os("WAYLAND_DISPLAY").is_none() && env::var_os("WAYLAND_SOCKET").is_none() {
        return Ok(None);
    }
    let wayland = Connection::connect_to_env()?;
    let peer = credentials(wayland.backend().poll_fd().as_raw_fd())?;
    let process = fs::read_to_string(format!("/proc/{}/comm", peer.pid))?;
    let kind = match process.trim() {
        "sway" => Kind::Sway,
        "Hyprland" => Kind::Hyprland,
        _ => return Ok(None),
    };
    let path = socket_path(kind, &peer)?;
    let mut stream = UnixStream::connect(path)?;
    let ipc_peer = credentials(stream.as_raw_fd())?;
    // Service-launched servers may lack compositor env vars or inherit another session's.
    if (ipc_peer.pid, ipc_peer.uid) != (peer.pid, peer.uid) {
        bail!(
            "{:?} IPC does not belong to the current Wayland compositor",
            kind
        );
    }
    let timeout = Some(super::super::DBUS_TIMEOUT);
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    let reply = match kind {
        Kind::Sway => sway(&mut stream),
        Kind::Hyprland => hyprland(&mut stream),
    }
    .map_err(|error| anyhow!("{:?} keyboard layout query failed: {}", kind, error))?;
    Ok(Some((kind, reply)))
}

fn credentials(fd: libc::c_int) -> ResultType<libc::ucred> {
    let mut peer = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut size = size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut size,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error().into());
    }
    if size as usize != size_of::<libc::ucred>() || peer.pid <= 0 {
        bail!("Cannot identify the Wayland compositor process");
    }
    Ok(peer)
}

fn socket_path(kind: Kind, peer: &libc::ucred) -> ResultType<PathBuf> {
    let runtime = PathBuf::from(
        env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| anyhow!("XDG_RUNTIME_DIR is unavailable"))?,
    );
    match kind {
        Kind::Sway => Ok(env::var_os("SWAYSOCK")
            .map(PathBuf::from)
            .unwrap_or_else(|| runtime.join(format!("sway-ipc.{}.{}.sock", peer.uid, peer.pid)))),
        Kind::Hyprland => {
            let pid = peer.pid.to_string();
            for entry in fs::read_dir(runtime.join("hypr"))? {
                let directory = entry?.path();
                let lock = match fs::read_to_string(directory.join("hyprland.lock")) {
                    Ok(lock) => lock,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                if lock.lines().next() == Some(pid.as_str()) {
                    return Ok(directory.join(".socket.sock"));
                }
            }
            bail!("Cannot locate IPC for the current Hyprland instance");
        }
    }
}

fn sway(stream: &mut UnixStream) -> ResultType<Value> {
    let mut request = SWAY_MAGIC.to_vec();
    request.extend_from_slice(&EMPTY_PAYLOAD.to_ne_bytes());
    request.extend_from_slice(&SWAY_GET_INPUTS.to_ne_bytes());
    stream.write_all(&request)?;
    let mut magic = [0; SWAY_MAGIC.len()];
    stream.read_exact(&mut magic)?;
    if magic != SWAY_MAGIC {
        bail!("Invalid Sway IPC response header");
    }
    let length = usize::try_from(read_u32(stream)?)?;
    if read_u32(stream)? != SWAY_GET_INPUTS || length > MAX_REPLY_BYTES {
        bail!("Unexpected or oversized Sway keyboard response");
    }
    let mut reply = vec![0; length];
    stream.read_exact(&mut reply)?;
    Ok(serde_json::from_slice(&reply)?)
}

fn read_u32(stream: &mut UnixStream) -> ResultType<u32> {
    let mut bytes = [0; size_of::<u32>()];
    stream.read_exact(&mut bytes)?;
    Ok(u32::from_ne_bytes(bytes))
}

fn hyprland(stream: &mut UnixStream) -> ResultType<Value> {
    stream.write_all(b"j/devices")?;
    let mut reply = Vec::new();
    stream
        .take((MAX_REPLY_BYTES + 1) as u64)
        .read_to_end(&mut reply)?;
    if reply.len() > MAX_REPLY_BYTES {
        bail!("Oversized Hyprland keyboard response");
    }
    Ok(serde_json::from_slice(&reply)?)
}
