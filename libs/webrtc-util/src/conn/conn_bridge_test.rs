use std::collections::VecDeque;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc;

use super::conn_bridge::*;
use super::*;

static MSG1: Bytes = Bytes::from_static(b"ADC");
static MSG2: Bytes = Bytes::from_static(b"DEFG");

#[tokio::test]
async fn test_bridge_normal() -> Result<()> {
    let (br, conn0, conn1) = Bridge::new(0, None, None);

    let n = conn0.send(&MSG1).await?;
    assert_eq!(n, MSG1.len(), "unexpected length");

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        let n = conn1.recv(&mut buf).await?;
        let _ = tx.send(n).await;
        Result::<()>::Ok(())
    });

    br.process().await;

    let n = rx.recv().await.unwrap();
    assert_eq!(n, MSG1.len(), "unexpected length");

    Ok(())
}

#[tokio::test]
async fn test_bridge_drop_first_packet_from_conn0() -> Result<()> {
    let (br, conn0, conn1) = Bridge::new(0, None, None);

    let n = conn0.send(&MSG1).await?;
    assert_eq!(n, MSG1.len(), "unexpected length");
    let n = conn0.send(&MSG2).await?;
    assert_eq!(n, MSG2.len(), "unexpected length");

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        let n = conn1.recv(&mut buf).await?;
        let _ = tx.send(n).await;
        Result::<()>::Ok(())
    });

    br.drop_offset(0, 0, 1).await;
    br.process().await;

    let n = rx.recv().await.unwrap();
    assert_eq!(n, MSG2.len(), "unexpected length");

    Ok(())
}

#[tokio::test]
async fn test_bridge_drop_second_packet_from_conn0() -> Result<()> {
    let (br, conn0, conn1) = Bridge::new(0, None, None);

    let n = conn0.send(&MSG1).await?;
    assert_eq!(n, MSG1.len(), "unexpected length");
    let n = conn0.send(&MSG2).await?;
    assert_eq!(n, MSG2.len(), "unexpected length");

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        let n = conn1.recv(&mut buf).await?;
        let _ = tx.send(n).await;
        Result::<()>::Ok(())
    });

    br.drop_offset(0, 1, 1).await;
    br.process().await;

    let n = rx.recv().await.unwrap();
    assert_eq!(n, MSG1.len(), "unexpected length");

    Ok(())
}

#[tokio::test]
async fn test_bridge_drop_first_packet_from_conn1() -> Result<()> {
    let (br, conn0, conn1) = Bridge::new(0, None, None);

    let n = conn1.send(&MSG1).await?;
    assert_eq!(n, MSG1.len(), "unexpected length");
    let n = conn1.send(&MSG2).await?;
    assert_eq!(n, MSG2.len(), "unexpected length");

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        let n = conn0.recv(&mut buf).await?;
        let _ = tx.send(n).await;
        Result::<()>::Ok(())
    });

    br.drop_offset(1, 0, 1).await;
    br.process().await;

    let n = rx.recv().await.unwrap();
    assert_eq!(n, MSG2.len(), "unexpected length");

    Ok(())
}

#[tokio::test]
async fn test_bridge_drop_second_packet_from_conn1() -> Result<()> {
    let (br, conn0, conn1) = Bridge::new(0, None, None);

    let n = conn1.send(&MSG1).await?;
    assert_eq!(n, MSG1.len(), "unexpected length");
    let n = conn1.send(&MSG2).await?;
    assert_eq!(n, MSG2.len(), "unexpected length");

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        let n = conn0.recv(&mut buf).await?;
        let _ = tx.send(n).await;
        Result::<()>::Ok(())
    });

    br.drop_offset(1, 1, 1).await;
    br.process().await;

    let n = rx.recv().await.unwrap();
    assert_eq!(n, MSG1.len(), "unexpected length");

    Ok(())
}

#[tokio::test]
async fn test_bridge_reorder_packets_from_conn0() -> Result<()> {
    let (br, conn0, conn1) = Bridge::new(0, None, None);

    let n = conn0.send(&MSG1).await?;
    assert_eq!(n, MSG1.len(), "unexpected length");
    let n = conn0.send(&MSG2).await?;
    assert_eq!(n, MSG2.len(), "unexpected length");

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        let n = conn1.recv(&mut buf).await?;
        assert_eq!(n, MSG2.len(), "unexpected length");
        let n = conn1.recv(&mut buf).await?;
        assert_eq!(n, MSG1.len(), "unexpected length");

        let _ = rx.recv().await;

        Result::<()>::Ok(())
    });

    br.reorder(0).await;
    br.process().await;

    let _ = tx.send(()).await;

    Ok(())
}

#[tokio::test]
async fn test_bridge_reorder_packets_from_conn1() -> Result<()> {
    let (br, conn0, conn1) = Bridge::new(0, None, None);

    let n = conn1.send(&MSG1).await?;
    assert_eq!(n, MSG1.len(), "unexpected length");
    let n = conn1.send(&MSG2).await?;
    assert_eq!(n, MSG2.len(), "unexpected length");

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        let n = conn0.recv(&mut buf).await?;
        assert_eq!(n, MSG2.len(), "unexpected length");
        let n = conn0.recv(&mut buf).await?;
        assert_eq!(n, MSG1.len(), "unexpected length");

        let _ = rx.recv().await;

        Result::<()>::Ok(())
    });

    br.reorder(1).await;
    br.process().await;

    let _ = tx.send(()).await;

    Ok(())
}

#[tokio::test]
async fn test_bridge_inverse_error() -> Result<()> {
    let mut q = VecDeque::new();
    q.push_back(MSG1.clone());
    assert!(!inverse(&mut q));
    Ok(())
}

#[tokio::test]
async fn test_bridge_drop_next_n_packets() -> Result<()> {
    for id in 0..2 {
        let (br, conn0, conn1) = Bridge::new(0, None, None);
        br.drop_next_nwrites(id, 3);
        let conns: Vec<Arc<dyn Conn + Send + Sync>> = vec![Arc::new(conn0), Arc::new(conn1)];
        let src_conn = Arc::clone(&conns[id]);
        let dst_conn = Arc::clone(&conns[1 - id]);

        let (tx, mut rx) = mpsc::channel(5);

        tokio::spawn(async move {
            let mut buf = vec![0u8; 256];
            for _ in 0..2u8 {
                let n = dst_conn.recv(&mut buf).await?;
                let _ = tx.send(buf[..n].to_vec()).await;
            }

            Result::<()>::Ok(())
        });

        let mut msgs = vec![];
        for i in 0..5u8 {
            let msg = format!("msg{i}");
            let n = src_conn.send(msg.as_bytes()).await?;
            assert_eq!(n, msg.len(), "[{id}] unexpected length");
            msgs.push(msg);
            br.process().await;
        }

        for i in 0..2 {
            if let Some(buf) = rx.recv().await {
                assert_eq!(msgs[i + 3].as_bytes(), &buf);
            } else {
                panic!("{id} unexpected number of packets");
            }
        }
    }

    Ok(())
}
