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
