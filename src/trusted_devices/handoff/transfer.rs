use hbb_common::{
    anyhow::Context,
    bail,
    config::{Config, Config2},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Write,
    process::{Command, Stdio},
};

use super::super::{config, service};

#[derive(Deserialize, Serialize)]
struct Snapshot {
    config: Config,
    options: Config2,
    pending: Option<Vec<u8>>,
}

impl Snapshot {
    fn read() -> ResultType<Self> {
        // A missing identity must not become a default snapshot of another host.
        std::fs::metadata(Config::file()).context("Read the current owner's configuration")?;
        config::snapshot()?;
        Ok(Self {
            config: Config::get(),
            options: Config2::get_for_sync()?,
            pending: service::pending_revocation()?,
        })
    }

    fn apply(self) -> ResultType<()> {
        service::sync_from_service(self.config, self.options)?;
        if self.pending.is_some() {
            Config2::set_trusted_devices_v2(String::new())?;
        }
        // Ownership may change only after both receiving files are durable.
        File::open(Config::file())?.sync_all()?;
        if Config2::file().try_exists()? {
            File::open(Config2::file())?.sync_all()?;
        }
        Ok(())
    }
}

fn as_user(uid: u32, operation: &str, input: Option<&[u8]>) -> ResultType<Vec<u8>> {
    // Open user-owned paths with that user's permissions, never with root privileges.
    let mut child = Command::new("/usr/bin/sudo")
        .args(["-n", "-H", "-u", &format!("#{uid}"), "--"])
        .arg(std::env::current_exe()?)
        .args(["--trusted-device-handoff", operation])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let sent = match input {
        Some(input) => child
            .stdin
            .take()
            .context("Open handoff input")?
            .write_all(input),
        None => Ok(()),
    };
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "Trusted-device handoff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    sent.context("Send trusted-device configuration")?;
    Ok(output.stdout)
}

pub(super) fn to_user(uid: u32) -> ResultType<()> {
    let snapshot = Config2::file()
        .try_exists()?
        .then(Snapshot::read)
        .transpose()?;
    as_user(uid, "import", Some(&serde_json::to_vec(&snapshot)?))?;
    Ok(())
}

pub(super) fn from_user(uid: u32) -> ResultType<()> {
    let mut snapshot: Snapshot = serde_json::from_slice(&as_user(uid, "export", None)?)?;
    snapshot.options.options.remove("stop-service");
    // Once handed off, this user's grants include subsequent enrollments and removals.
    // The old pending marker must not erase those new grants during reinstallation.
    let pending = snapshot.pending.take();
    snapshot.apply()?;
    if let Some(pending) = pending {
        as_user(uid, "acknowledge", Some(&serde_json::to_vec(&pending)?))?;
    }
    Ok(())
}

pub(super) fn command(operation: &str) -> ResultType<()> {
    match operation {
        "export" => {
            serde_json::to_writer(std::io::stdout().lock(), &Snapshot::read()?)?;
        }
        "import" => {
            let incoming: Option<Snapshot> = serde_json::from_reader(std::io::stdin().lock())?;
            let empty_handoff = incoming.is_none();
            // Offline security edits take precedence over the retained root snapshot.
            let mut snapshot = match incoming {
                Some(snapshot) if service::pending_revocation()?.is_none() => snapshot,
                _ => Snapshot::read()?,
            };
            snapshot
                .options
                .options
                .insert("stop-service".into(), "Y".into());
            snapshot.apply()?;
            if empty_handoff {
                // An unused service has no settings to copy, but its empty trust
                // store must still invalidate an inactive user's old credentials.
                Config2::set_trusted_devices_v2(String::new())?;
                File::open(Config2::file())?.sync_all()?;
            }
        }
        "acknowledge" => {
            let pending: Vec<u8> = serde_json::from_reader(std::io::stdin().lock())?;
            service::acknowledge_revocation(&pending)?;
        }
        _ => bail!("Invalid trusted-device configuration transfer"),
    }
    Ok(())
}
