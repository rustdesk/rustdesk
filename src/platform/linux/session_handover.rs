use base::message_proto::{message, misc, Message, Misc};
use hbb_common::{bail, log, protobuf::Message as _, timeout, tokio, ResultType, Stream};

use crate::{
    common::SESSION_HANDOVER_CLOSE_REASON,
    ipc::{self, Data},
    server::{AuthConnType, AUTHED_CONNS},
};

pub(super) async fn prepare(greeter_uid: &str) {
    let Ok(uid) = greeter_uid.parse::<u32>() else {
        return;
    };
    // DRM normally runs the greeter server as its user, with a root fallback.
    // Use the previous uid: seat0 already belongs to the incoming user now.
    for uid in [uid, 0] {
        let result = async {
            let mut conn = ipc::connect_for_uid(300, uid, "").await?;
            conn.send(&Data::SessionHandover).await?;
            if !matches!(conn.next_timeout(3000).await?, Some(Data::SessionHandover)) {
                bail!("Server did not confirm session handover");
            }
            Ok::<_, hbb_common::anyhow::Error>(())
        };
        match timeout(3500, result).await {
            Ok(Ok(())) => return,
            result => log::debug!("Could not hand over greeter server for uid {uid}: {result:?}"),
        }
    }
    log::warn!("Greeter server handover was not confirmed; continuing session switch");
}

pub(crate) async fn notify_clients() -> ResultType<()> {
    let ids: Vec<_> = AUTHED_CONNS
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.conn_type == AuthConnType::Remote)
        .filter_map(|c| match c.sender.send(Data::SessionHandover) {
            Ok(()) => Some(c.conn_id),
            Err(err) => {
                log::trace!(
                    "Session handover to closing connection {}: {err}",
                    c.conn_id
                );
                None
            }
        })
        .collect();
    timeout(2500, async {
        while AUTHED_CONNS
            .lock()
            .unwrap()
            .iter()
            .any(|c| ids.contains(&c.conn_id))
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}

pub(crate) async fn notify_peer(peer: &mut Stream) -> ResultType<()> {
    timeout(2000, async {
        let mut misc = Misc::new();
        misc.set_close_reason(SESSION_HANDOVER_CLOSE_REASON.to_owned());
        let mut message = Message::new();
        message.set_misc(misc);
        peer.send(&message).await?;
        while let Some(bytes) = peer.next().await {
            let message = Message::parse_from_bytes(&bytes?)?;
            if let Some(message::Union::Misc(misc)) = message.union {
                if matches!(misc.union, Some(misc::Union::CloseReason(reason)) if reason == SESSION_HANDOVER_CLOSE_REASON)
                {
                    return Ok(());
                }
            }
        }
        bail!("Peer closed before confirming session handover")
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::{
        tcp::FramedStream,
        tokio::net::{TcpListener, TcpStream},
    };

    #[tokio::test]
    async fn handover_waits_for_peer_confirmation_before_server_exit() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let socket = TcpStream::connect(addr).await.unwrap();
        let (remote, _) = listener.accept().await.unwrap();
        let mut server = Stream::Tcp(FramedStream::from(socket, addr));
        let mut client = Stream::Tcp(FramedStream::from(remote, addr));
        let server = tokio::spawn(async move { notify_peer(&mut server).await });
        let notice = Message::parse_from_bytes(&client.next().await.unwrap().unwrap()).unwrap();
        assert!(
            matches!(notice.misc().union.as_ref(), Some(misc::Union::CloseReason(reason)) if reason == SESSION_HANDOVER_CLOSE_REASON)
        );
        assert!(!server.is_finished());
        client.send(&notice).await.unwrap();
        assert!(server.await.unwrap().is_ok());
    }
}
