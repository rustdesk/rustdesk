use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_test::assert_ok;

use super::*;
use crate::error::Error;

#[tokio::test]
async fn test_buffer() {
    let buffer = Buffer::new(0, 0);
    let mut packet: Vec<u8> = vec![0; 4];

    // Write once
    let n = assert_ok!(buffer.write(&[0, 1]).await);
    assert_eq!(n, 2, "n must be 2");

    // Read once
    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(&packet[..n], &[0, 1]);

    // Read deadline
    let result = buffer.read(&mut packet, Some(Duration::new(0, 1))).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::ErrTimeout);

    // Write twice
    let n = assert_ok!(buffer.write(&[2, 3, 4]).await);
    assert_eq!(n, 3, "n must be 3");

    let n = assert_ok!(buffer.write(&[5, 6, 7]).await);
    assert_eq!(n, 3, "n must be 3");

    // Read twice
    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 3, "n must be 3");
    assert_eq!(&packet[..n], &[2, 3, 4]);

    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 3, "n must be 3");
    assert_eq!(&packet[..n], &[5, 6, 7]);

    // Write once prior to close.
    let n = assert_ok!(buffer.write(&[3]).await);
    assert_eq!(n, 1, "n must be 1");

    // Close
    buffer.close().await;

    // Future writes will error
    let result = buffer.write(&[4]).await;
    assert!(result.is_err());

    // But we can read the remaining data.
    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 1, "n must be 1");
    assert_eq!(&packet[..n], &[3]);

    // Until EOF
    let result = buffer.read(&mut packet, None).await;
    assert!(result.is_err());
    assert_eq!(Error::ErrBufferClosed, result.unwrap_err());
}

async fn test_wraparound(grow: bool) {
    let buffer = Buffer::new(0, 0);
    {
        let mut b = buffer.buffer.lock().await;
        let result = b.grow();
        assert!(result.is_ok());

        b.head = b.data.len() - 13;
        b.tail = b.head;
    }

    let p1 = vec![1, 2, 3];
    let p2 = vec![4, 5, 6];
    let p3 = vec![7, 8, 9];
    let p4 = vec![10, 11, 12];

    assert_ok!(buffer.write(&p1).await);
    assert_ok!(buffer.write(&p2).await);
    assert_ok!(buffer.write(&p3).await);

    let mut p = vec![0; 10];

    let n = assert_ok!(buffer.read(&mut p, None).await);
    assert_eq!(&p1[..], &p[..n]);

    if grow {
        let mut b = buffer.buffer.lock().await;
        let result = b.grow();
        assert!(result.is_ok());
    }

    let n = assert_ok!(buffer.read(&mut p, None).await);
    assert_eq!(&p2[..], &p[..n]);

    assert_ok!(buffer.write(&p4).await);

    let n = assert_ok!(buffer.read(&mut p, None).await);
    assert_eq!(&p3[..], &p[..n]);
    let n = assert_ok!(buffer.read(&mut p, None).await);
    assert_eq!(&p4[..], &p[..n]);

    {
        let b = buffer.buffer.lock().await;
        if !grow {
            assert_eq!(b.data.len(), MIN_SIZE);
        } else {
            assert_eq!(b.data.len(), 2 * MIN_SIZE);
        }
    }
}

#[tokio::test]
async fn test_buffer_wraparound() {
    test_wraparound(false).await;
}

#[tokio::test]
async fn test_buffer_wraparound_grow() {
    test_wraparound(true).await;
}

#[tokio::test]
async fn test_buffer_async() {
    let buffer = Buffer::new(0, 0);

    let (done_tx, mut done_rx) = mpsc::channel::<()>(1);

    let buffer2 = buffer.clone();
    tokio::spawn(async move {
        let mut packet: Vec<u8> = vec![0; 4];

        let n = assert_ok!(buffer2.read(&mut packet, None).await);
        assert_eq!(n, 2, "n must be 2");
        assert_eq!(&packet[..n], &[0, 1]);

        let result = buffer2.read(&mut packet, None).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::ErrBufferClosed);

        drop(done_tx);
    });

    // Wait for the reader to start reading.
    sleep(Duration::from_micros(1)).await;

    // Write once
    let n = assert_ok!(buffer.write(&[0, 1]).await);
    assert_eq!(n, 2, "n must be 2");

    // Wait for the reader to start reading again.
    sleep(Duration::from_micros(1)).await;

    // Close will unblock the reader.
    buffer.close().await;

    done_rx.recv().await;
}

#[tokio::test]
async fn test_buffer_limit_count() {
    let buffer = Buffer::new(2, 0);

    assert_eq!(buffer.count().await, 0);

    // Write twice
    let n = assert_ok!(buffer.write(&[0, 1]).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(buffer.count().await, 1);

    let n = assert_ok!(buffer.write(&[2, 3]).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(buffer.count().await, 2);

    // Over capacity
    let result = buffer.write(&[4, 5]).await;
    assert!(result.is_err());
    if let Err(err) = result {
        assert_eq!(err, Error::ErrBufferFull);
    }
    assert_eq!(buffer.count().await, 2);

    // Read once
    let mut packet: Vec<u8> = vec![0; 4];
    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(&packet[..n], &[0, 1]);
    assert_eq!(buffer.count().await, 1);

    // Write once
    let n = assert_ok!(buffer.write(&[6, 7]).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(buffer.count().await, 2);

    // Over capacity
    let result = buffer.write(&[8, 9]).await;
    assert!(result.is_err());
    if let Err(err) = result {
        assert_eq!(Error::ErrBufferFull, err);
    }
    assert_eq!(buffer.count().await, 2);

    // Read twice
    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(&packet[..n], &[2, 3]);
    assert_eq!(buffer.count().await, 1);

    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(&packet[..n], &[6, 7]);
    assert_eq!(buffer.count().await, 0);

    // Nothing left.
    buffer.close().await;
}

#[tokio::test]
async fn test_buffer_limit_size() {
    let buffer = Buffer::new(0, 11);

    assert_eq!(buffer.size().await, 0);

    // Write twice
    let n = assert_ok!(buffer.write(&[0, 1]).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(buffer.size().await, 4);

    let n = assert_ok!(buffer.write(&[2, 3]).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(buffer.size().await, 8);

    // Over capacity
    let result = buffer.write(&[4, 5]).await;
    assert!(result.is_err());
    if let Err(err) = result {
        assert_eq!(Error::ErrBufferFull, err);
    }
    assert_eq!(buffer.size().await, 8);

    // Cheeky write at exact size.
    let n = assert_ok!(buffer.write(&[6]).await);
    assert_eq!(n, 1, "n must be 1");
    assert_eq!(buffer.size().await, 11);

    // Read once
    let mut packet: Vec<u8> = vec![0; 4];
    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(&packet[..n], &[0, 1]);
    assert_eq!(buffer.size().await, 7);

    // Write once
    let n = assert_ok!(buffer.write(&[7, 8]).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(buffer.size().await, 11);

    // Over capacity
    let result = buffer.write(&[9, 10]).await;
    assert!(result.is_err());
    if let Err(err) = result {
        assert_eq!(Error::ErrBufferFull, err);
    }
    assert_eq!(buffer.size().await, 11);

    // Read everything
    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(&packet[..n], &[2, 3]);
    assert_eq!(buffer.size().await, 7);

    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 1, "n must be 1");
    assert_eq!(&packet[..n], &[6]);
    assert_eq!(buffer.size().await, 4);

    let n = assert_ok!(buffer.read(&mut packet, None).await);
    assert_eq!(n, 2, "n must be 2");
    assert_eq!(&packet[..n], &[7, 8]);
    assert_eq!(buffer.size().await, 0);

    // Nothing left.
    buffer.close().await;
}

#[tokio::test]
async fn test_buffer_limit_sizes() {
    let sizes = vec![
        128 * 1024,
        1024 * 1024,
        8 * 1024 * 1024,
        0, // default
    ];
    const HEADER_SIZE: usize = 2;
    const PACKET_SIZE: usize = 0x8000;

    for mut size in sizes {
        let mut name = "default".to_owned();
        if size > 0 {
            name = format!("{}kbytes", size / 1024);
        }

        let buffer = Buffer::new(0, 0);
        if size == 0 {
            size = MAX_SIZE;
        } else {
            buffer.set_limit_size(size + HEADER_SIZE).await;
        }

        //assert.NoError(buffer.SetReadDeadline(now.Add(5 * time.Second))) // Set deadline to avoid test deadlock

        let n_packets = size / (PACKET_SIZE + HEADER_SIZE);
        let pkt = vec![0; PACKET_SIZE];
        for _ in 0..n_packets {
            assert_ok!(buffer.write(&pkt).await);
        }

        // Next write is expected to be errored.
        let result = buffer.write(&pkt).await;
        assert!(result.is_err(), "{}", name);
        assert_eq!(result.unwrap_err(), Error::ErrBufferFull, "{name}");

        let mut packet = vec![0; size];
        for _ in 0..n_packets {
            let n = assert_ok!(buffer.read(&mut packet, Some(Duration::new(5, 0))).await);
            assert_eq!(n, PACKET_SIZE, "{name}");
        }
    }
}

#[tokio::test]
async fn test_buffer_misc() {
    let buffer = Buffer::new(0, 0);

    // Write once
    let n = assert_ok!(buffer.write(&[0, 1, 2, 3]).await);
    assert_eq!(n, 4, "n must be 4");

    // Try to read with a short buffer
    let mut packet: Vec<u8> = vec![0; 3];
    let result = buffer.read(&mut packet, None).await;
    assert!(result.is_err());
    if let Err(err) = result {
        assert_eq!(err, Error::ErrBufferShort);
    }

    // Close
    buffer.close().await;

    // check is_close
    assert!(buffer.is_closed().await);

    // Make sure you can Close twice
    buffer.close().await;
}
