use super::authority::{service_selected, Unavailable};
use hbb_common::{anyhow::Context, bail, libc, ResultType};
use serde_derive::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

mod transfer;

const ROOT_UID: u32 = 0;
const OWNER_FILE_MODE: u32 = 0o644;
const WRITABLE_BY_OTHERS: u32 = 0o022;

#[derive(Deserialize, Serialize)]
struct UserOwner {
    uid: u32,
}

fn path() -> PathBuf {
    Path::new("/Library/Preferences").join(format!(
        "{}_trusted_devices_service",
        crate::get_full_name()
    ))
}

pub(super) fn registered() -> ResultType<bool> {
    Path::new("/Library/LaunchDaemons")
        .join(format!("{}_service.plist", crate::get_full_name()))
        .try_exists()
        .context("Inspect trusted-device system service registration")
}

fn open(writable: bool) -> ResultType<Option<File>> {
    let file = match OpenOptions::new()
        .read(true)
        .write(writable)
        .create(writable)
        .mode(OWNER_FILE_MODE)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path())
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("Open trusted-device authority"),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != ROOT_UID
        || metadata.mode() & WRITABLE_BY_OTHERS != 0
    {
        bail!("Invalid trusted-device authority file permissions");
    }
    if writable {
        file.set_permissions(std::fs::Permissions::from_mode(OWNER_FILE_MODE))?;
    }
    let lock = if writable {
        libc::LOCK_EX
    } else {
        libc::LOCK_SH
    };
    if unsafe { libc::flock(file.as_raw_fd(), lock | libc::LOCK_NB) } != 0 {
        return Err(std::io::Error::last_os_error()).context("Trusted-device handoff is busy");
    }
    Ok(Some(file))
}

fn owner(file: &mut File) -> ResultType<Option<u32>> {
    let mut value = String::new();
    file.seek(SeekFrom::Start(0))?;
    file.read_to_string(&mut value)?;
    if value.is_empty() {
        return Ok(None);
    }
    let owner: UserOwner = serde_json::from_str(&value)?;
    if owner.uid == ROOT_UID {
        bail!("Trusted-device local authority must be a user account");
    }
    Ok(Some(owner.uid))
}

pub(super) fn uses_service() -> ResultType<bool> {
    let registered = registered()?;
    let owner = open(false)
        .and_then(|file| file.map(|mut file| owner(&mut file)).transpose())
        .map_err(Unavailable)?;
    Ok(service_selected(registered, owner, unsafe {
        libc::geteuid()
    }))
}

// Hold this descriptor until the local operation finishes, so installation cannot
// export the user store while a grant operation is still changing it.
pub(super) fn lock_authority() -> ResultType<Option<File>> {
    let mut file = open(false).map_err(Unavailable)?;
    let owner = file.as_mut().map(owner).transpose().map_err(Unavailable)?;
    let uid = unsafe { libc::geteuid() };
    let changed = if uid == ROOT_UID && !crate::is_server() {
        owner.flatten().is_some()
    } else {
        service_selected(registered()?, owner, uid)
    };
    if changed {
        return Err(Unavailable(hbb_common::anyhow::anyhow!(
            "Trusted-device authority changed; retry the operation"
        ))
        .into());
    }
    Ok(file)
}

fn set_owner(file: &mut File, uid: Option<u32>) -> ResultType<()> {
    file.seek(SeekFrom::Start(0))?;
    let value = uid
        .map(|uid| serde_json::to_vec(&UserOwner { uid }))
        .transpose()?
        .unwrap_or_default();
    file.write_all(&value)?;
    file.set_len(value.len() as u64)?;
    file.sync_all().context("Persist trusted-device authority")
}

fn privileged_owner() -> ResultType<File> {
    if unsafe { libc::geteuid() } != ROOT_UID {
        bail!("Transferring trusted-device authority requires root privileges");
    }
    open(true)?.context("Open writable trusted-device authority")
}

pub(super) fn claim_service() -> ResultType<bool> {
    let mut file = privileged_owner()?;
    let Some(uid) = owner(&mut file)? else {
        file.sync_all()?;
        return Ok(false);
    };
    transfer::from_user(uid)?;
    set_owner(&mut file, None)?;
    Ok(true)
}

fn to_user(user: &str) -> ResultType<()> {
    let mut file = privileged_owner()?;
    let service = format!("system/{}_service", crate::get_full_name());
    if std::process::Command::new("/bin/launchctl")
        .args(["print", &service])
        .output()?
        .status
        .success()
    {
        bail!("Stop the system service before handing off trusted devices");
    }
    if user.is_empty() || user.starts_with('-') {
        bail!("Invalid trusted-device owner");
    }
    let output = std::process::Command::new("/usr/bin/id")
        .args(["-u", user])
        .output()?;
    if !output.status.success() {
        bail!("Cannot find the trusted-device owner");
    }
    let uid: u32 = std::str::from_utf8(&output.stdout)?.trim().parse()?;
    if uid == ROOT_UID {
        bail!("Select a user account for trusted-device handoff");
    }
    if let Some(current) = owner(&mut file)? {
        if current != uid {
            bail!("Trusted devices belong to another user; reinstall the service before transferring them");
        }
        return Ok(());
    }
    transfer::to_user(uid)?;
    set_owner(&mut file, Some(uid))
}

pub(super) fn command(args: &[String]) -> ResultType<()> {
    match args {
        [operation, user] if operation == "user" => to_user(user),
        [operation] => transfer::command(operation),
        _ => bail!("Invalid trusted-device handoff operation"),
    }
}
