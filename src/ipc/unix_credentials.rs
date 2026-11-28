use super::{
    connect_with_path, ipc_auth::ensure_peer_executable_matches_current_by_pid_opt, Connection,
};
use hbb_common::{
    anyhow::{anyhow, ensure},
    config::Config,
    ResultType,
};

const IPC_TIMEOUT: u64 = 1_000;

pub(super) async fn verified_main(ms_timeout: u64, target: u32) -> ResultType<Connection> {
    let connection = connect_with_path(ms_timeout, &Config::ipc_path_for_uid(target, "")).await?;
    verify_main_peer(&connection, target)?;
    Ok(connection)
}

fn verify_main_peer(connection: &Connection, target: u32) -> ResultType<()> {
    ensure!(
        connection.peer_uid() == Some(target),
        "Unexpected main IPC server UID"
    );
    ensure_peer_executable_matches_current_by_pid_opt(connection.peer_pid(), "")
}

// Full service sync delegates credential changes to the actual user main server.
pub(super) async fn verify_sync_client(connection: &Connection) -> ResultType<()> {
    let uid = connection
        .peer_uid()
        .ok_or_else(|| anyhow!("Missing sync client UID"))?;
    if uid == 0 {
        return Ok(());
    }
    let pid = connection
        .peer_pid()
        .ok_or_else(|| anyhow!("Missing sync client PID"))?;
    let main = connect_with_path(IPC_TIMEOUT, &Config::ipc_path_for_uid(uid, "")).await?;
    ensure!(
        main.peer_uid() == Some(uid) && main.peer_pid() == Some(pid),
        "Expected the user main IPC server"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixListener,
        process::{Child, Command},
    };
    use std::{env, fs, os::unix::fs::DirBuilderExt, path::Path, process::Stdio};

    const ROLE: &str = "RUSTDESK_TEST_SYNC_PEER";
    const TEST_TIMEOUT_MS: u64 = 10_000;
    // libtest also exits successfully when an exact filter selects no tests.
    const OWNER_VERIFIED: i32 = 42;

    #[tokio::test]
    async fn service_sync_requires_main_listener_owner() -> ResultType<()> {
        let mut child = Command::new(env::current_exe()?);
        child
            .arg("--exact")
            .arg("ipc::unix_credentials::tests::service_sync_requires_main_listener_owner")
            .kill_on_drop(true);
        let Some(role) = env::var_os(ROLE) else {
            let result =
                hbb_common::timeout(TEST_TIMEOUT_MS, child.env(ROLE, "owner").output()).await??;
            assert_eq!(
                result.status.code(),
                Some(OWNER_VERIFIED),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            return Ok(());
        };
        // Start both isolated peers before dropping privileges, even in a root-only build directory.
        let other = if role == "owner" {
            Some(child.env(ROLE, "sender").stdin(Stdio::piped()).spawn()?)
        } else {
            None
        };
        if unsafe { hbb_common::libc::geteuid() } == 0 {
            const UNPRIVILEGED_UID: u32 = 65_534;
            if unsafe { hbb_common::libc::setuid(UNPRIVILEGED_UID) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        if let Some(other) = other {
            check_sync_senders(other).await?;
            std::process::exit(OWNER_VERIFIED);
        }
        let mut path = String::new();
        tokio::io::stdin().read_to_string(&mut path).await?;
        let mut socket = tokio::net::UnixStream::connect(path).await?;
        // Keep the sender alive until the verifier closes its side.
        assert_eq!(socket.read(&mut [0]).await?, 0);
        Ok(())
    }

    async fn check_sync_senders(mut other: Child) -> ResultType<()> {
        *hbb_common::config::APP_NAME.write().unwrap() =
            format!("rd-sync-{}", uuid::Uuid::new_v4());
        let main_path = Config::ipc_path("");
        let directory = Path::new(&main_path).parent().unwrap().to_owned();
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let sync_path = format!("{main_path}.sync");
        let _cleanup = crate::SimpleCallOnReturn {
            b: true,
            f: Box::new(move || fs::remove_dir_all(&directory).unwrap()),
        };
        let _main = UnixListener::bind(main_path)?;
        let mut sync = parity_tokio_ipc::Endpoint::new(sync_path.clone()).incoming()?;

        let _owner = tokio::net::UnixStream::connect(&sync_path).await?;
        let owner = Connection::new(sync.next().await.unwrap()?);
        verify_sync_client(&owner).await?;

        other
            .stdin
            .take()
            .unwrap()
            .write_all(sync_path.as_bytes())
            .await?;
        let sender = Connection::new(sync.next().await.unwrap()?);
        assert_eq!(sender.peer_uid(), owner.peer_uid());
        let result = verify_sync_client(&sender).await;
        drop(sender);
        assert!(other.wait().await?.success());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Expected the user main IPC server"
        );
        Ok(())
    }
}
