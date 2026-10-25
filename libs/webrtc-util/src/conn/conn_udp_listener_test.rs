use std::future::Future;
use std::pin::Pin;

use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::Duration;

use super::conn_udp_listener::*;
use super::*;
use crate::error::{Error, Result};

async fn pipe() -> Result<(
    Arc<dyn Listener + Send + Sync>,
    Arc<dyn Conn + Send + Sync>,
    UdpSocket,
)> {
    // Start listening
    let listener = Arc::new(listen("0.0.0.0:0").await?);

    // Open a connection
    let d_conn = UdpSocket::bind("0.0.0.0:0").await?;
    d_conn.connect(listener.addr().await?).await?;

    // Write to the connection to initiate it
    let handshake = "hello";
    d_conn.send(handshake.as_bytes()).await?;
    let daddr = d_conn.local_addr()?;

    // Accept the connection
    let (l_conn, raddr) = listener.accept().await?;
    assert_eq!(daddr, raddr, "remote address should be match");

    let raddr = l_conn.remote_addr();
    if let Some(raddr) = raddr {
        assert_eq!(daddr, raddr, "remote address should be match");
    } else {
        panic!("expected Some, but got None, for remote_addr()");
    }

    let mut buf = vec![0u8; handshake.len()];
    let n = l_conn.recv(&mut buf).await?;

    let result = String::from_utf8(buf[..n].to_vec())?;
    if handshake != result {
        Err(Error::Other(format!(
            "errHandshakeFailed: {handshake} != {result}"
        )))
    } else {
        Ok((listener, l_conn, d_conn))
    }
}

#[tokio::test]
async fn test_listener_close_timeout() -> Result<()> {
    let (listener, ca, _) = pipe().await?;

    listener.close().await?;

    // Close client after server closes to cleanup
    ca.close().await?;

    Ok(())
}

#[tokio::test]
async fn test_listener_close_unaccepted() -> Result<()> {
    const BACKLOG: usize = 2;

    let listener = ListenConfig {
        backlog: BACKLOG,
        ..Default::default()
    }
    .listen("0.0.0.0:0")
    .await?;

    for i in 0..BACKLOG as u8 {
        let conn = UdpSocket::bind("0.0.0.0:0").await?;
        conn.connect(listener.addr().await?).await?;
        conn.send(&[i]).await?;
        conn.close().await?;
    }

    // Wait all packets being processed by readLoop
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Unaccepted connections must be closed by listener.Close()
    listener.close().await?;

    Ok(())
}

#[tokio::test]
async fn test_listener_accept_filter() -> Result<()> {
    let tests = vec![("CreateConn", &[0xAA], true), ("Discarded", &[0x00], false)];

    for (name, packet, expected) in tests {
        let accept_filter: Option<AcceptFilterFn> = Some(Box::new(
            |pkt: &[u8]| -> Pin<Box<dyn Future<Output = bool> + Send + 'static>> {
                let p0 = pkt[0];
                Box::pin(async move { p0 == 0xAA })
            },
        ));

        let listener = Arc::new(
            ListenConfig {
                accept_filter,
                ..Default::default()
            }
            .listen("0.0.0.0:0")
            .await?,
        );

        let conn = UdpSocket::bind("0.0.0.0:0").await?;
        conn.connect(listener.addr().await?).await?;
        conn.send(packet).await?;

        let (ch_accepted_tx, mut ch_accepted_rx) = mpsc::channel::<()>(1);
        let mut ch_accepted_tx = Some(ch_accepted_tx);
        let listener2 = Arc::clone(&listener);
        tokio::spawn(async move {
            let (c, _raddr) = match listener2.accept().await {
                Ok((c, raddr)) => (c, raddr),
                Err(err) => {
                    assert_eq!(Error::ErrClosedListener, err);
                    return Result::<()>::Ok(());
                }
            };

            ch_accepted_tx.take();
            c.close().await?;

            Result::<()>::Ok(())
        });

        let mut accepted = false;
        let mut timeout = false;
        let timer = tokio::time::sleep(Duration::from_millis(10));
        tokio::pin!(timer);
        tokio::select! {
            _= ch_accepted_rx.recv()=>{
                accepted = true;
            }
            _ = timer.as_mut() => {
                timeout = true;
            }
        }

        assert_eq!(accepted, expected, "{name}: unexpected result");
        assert_eq!(!timeout, expected, "{name}: unexpected result");

        conn.close().await?;
        listener.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn test_listener_concurrent() -> Result<()> {
    const BACKLOG: usize = 2;

    let listener = Arc::new(
        ListenConfig {
            backlog: BACKLOG,
            ..Default::default()
        }
        .listen("0.0.0.0:0")
        .await?,
    );

    for i in 0..BACKLOG as u8 + 1 {
        let conn = UdpSocket::bind("0.0.0.0:0").await?;
        conn.connect(listener.addr().await?).await?;
        conn.send(&[i]).await?;
        conn.close().await?;
    }

    // Wait all packets being processed by readLoop
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut b = vec![0u8; 1];
    for i in 0..BACKLOG as u8 {
        let (conn, _raddr) = listener.accept().await?;
        let n = conn.recv(&mut b).await?;
        assert_eq!(
            &b[..n],
            &[i],
            "Packet from connection {} is wrong, expected: [{}], got: {:?}",
            i,
            i,
            &b[..n]
        );
        conn.close().await?;
    }

    let (done_tx, mut done_rx) = mpsc::channel::<()>(1);
    let mut done_tx = Some(done_tx);
    let listener2 = Arc::clone(&listener);
    tokio::spawn(async move {
        match listener2.accept().await {
            Ok((conn, _raddr)) => {
                conn.close().await?;
            }
            Err(err) => {
                assert!(Error::ErrClosedListener == err || Error::ErrClosedListenerAcceptCh == err);
            }
        }

        done_tx.take();

        Result::<()>::Ok(())
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    listener.close().await?;

    let _ = done_rx.recv().await;

    Ok(())
}
