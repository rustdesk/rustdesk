use super::*;
use hbb_common::tcp::FramedStream;

#[cfg(feature = "flutter")]
use crate::flutter::FlutterHandler as TestUi;
#[cfg(not(feature = "flutter"))]
use crate::ui::remote::SciterHandler as TestUi;

fn remote(displays: &[usize]) -> Remote<TestUi> {
    let (sender, receiver) = mpsc::unbounded_channel();
    let handler = Session::<TestUi>::default();
    handler.lc.write().unwrap().version = hbb_common::get_version_number("1.5.0");
    *handler.sender.write().unwrap() = Some(sender.clone());
    let mut remote = Remote::new(handler, receiver, sender);
    remote.first_frame = true;
    for &display in displays {
        let queue = ArrayQueue::new(2);
        queue.push(VideoFrame::new()).unwrap();
        queue.push(VideoFrame::new()).unwrap();
        remote.video_threads.insert(
            display,
            VideoThread {
                video_queue: Arc::new(RwLock::new(queue)),
                video_sender: std::sync::mpsc::channel().0,
                decode_fps: Arc::new(RwLock::new(Some(1))),
                frame_count: Default::default(),
                discard_queue: Default::default(),
                fps_control: Default::default(),
            },
        );
    }
    remote
}

fn streams() -> (Stream, Stream) {
    let (local, peer) = tokio::io::duplex(4096);
    let addr = ([127, 0, 0, 1], 0).into();
    (
        Stream::Tcp(FramedStream::from(local, addr)),
        Stream::Tcp(FramedStream::from(peer, addr)),
    )
}

async fn overflow(remote: &mut Remote<TestUi>, stream: &mut Stream, display: usize) {
    let mut frame = VideoFrame::new();
    frame.display = display as _;
    frame.set_h265s(EncodedVideoFrames::new());
    let mut message = Message::new();
    message.set_video_frame(frame);
    assert!(
        remote
            .handle_msg_from_peer(&message.write_to_bytes().unwrap(), stream)
            .await
    );
}

fn take_refreshes(remote: &mut Remote<TestUi>) -> Vec<Message> {
    let mut refreshes = Vec::new();
    while let Ok(data) = remote.receiver.try_recv() {
        if let Data::Message(message) = data {
            if matches!(
                message.misc().union,
                Some(misc::Union::RefreshVideo(_) | misc::Union::RefreshVideoDisplay(_))
            ) {
                refreshes.push(message);
            }
        }
    }
    refreshes
}

#[tokio::test(start_paused = true)]
async fn queue_overflow_and_fps_control_share_the_cooldown() {
    let mut remote = remote(&[0]);
    let (mut stream, _peer) = streams();
    overflow(&mut remote, &mut stream, 0).await;
    assert_eq!(take_refreshes(&mut remote).len(), 1);

    for _ in 0..3 {
        overflow(&mut remote, &mut stream, 0).await;
        remote.fps_control(true, HashMap::from([(0, 1)]));
        assert!(take_refreshes(&mut remote).is_empty());
    }
    time::advance(Duration::from_millis(9999)).await;
    overflow(&mut remote, &mut stream, 0).await;
    remote.fps_control(true, HashMap::from([(0, 1)]));
    assert!(take_refreshes(&mut remote).is_empty());

    time::advance(Duration::from_millis(1)).await;
    remote.fps_control(true, HashMap::from([(0, 1)]));
    assert_eq!(take_refreshes(&mut remote).len(), 1);
    overflow(&mut remote, &mut stream, 0).await;
    assert!(take_refreshes(&mut remote).is_empty());

    time::advance(Duration::from_secs(10)).await;
    overflow(&mut remote, &mut stream, 0).await;
    assert_eq!(take_refreshes(&mut remote).len(), 1);
}

#[tokio::test(start_paused = true)]
async fn fps_control_also_blocks_a_following_queue_overflow() {
    let mut remote = remote(&[0]);
    let (mut stream, _peer) = streams();
    remote.fps_control(true, HashMap::from([(0, 1)]));
    assert_eq!(take_refreshes(&mut remote).len(), 1);
    overflow(&mut remote, &mut stream, 0).await;
    assert!(take_refreshes(&mut remote).is_empty());
}

#[tokio::test(start_paused = true)]
async fn displays_have_independent_automatic_refresh_limits() {
    let mut remote = remote(&[0, 1]);
    let (mut stream, _peer) = streams();
    overflow(&mut remote, &mut stream, 0).await;
    assert_eq!(take_refreshes(&mut remote).len(), 1);
    overflow(&mut remote, &mut stream, 1).await;
    assert_eq!(take_refreshes(&mut remote).len(), 1);
    remote.fps_control(true, HashMap::from([(0, 1), (1, 1)]));
    assert!(take_refreshes(&mut remote).is_empty());
}

#[tokio::test(start_paused = true)]
async fn automatic_refresh_cap_is_shared_and_manual_refresh_still_works() {
    let mut remote = remote(&[0]);
    let (mut stream, mut peer) = streams();
    for attempt in 0..20 {
        if attempt % 2 == 0 {
            overflow(&mut remote, &mut stream, 0).await;
        } else {
            remote.fps_control(true, HashMap::from([(0, 1)]));
        }
        assert_eq!(take_refreshes(&mut remote).len(), 1);
        time::advance(Duration::from_secs(10)).await;
    }
    overflow(&mut remote, &mut stream, 0).await;
    remote.fps_control(true, HashMap::from([(0, 1)]));
    assert!(take_refreshes(&mut remote).is_empty());

    remote.handler.refresh_video(0);
    let messages = take_refreshes(&mut remote);
    assert_eq!(messages.len(), 1);
    let message = messages.into_iter().next().unwrap();
    assert!(
        remote
            .handle_msg_from_ui(Data::Message(message.clone()), &mut stream)
            .await
    );
    let bytes = peer.next().await.unwrap().unwrap();
    assert_eq!(Message::parse_from_bytes(&bytes).unwrap(), message);
    assert!(*remote.video_threads[&0].discard_queue.read().unwrap());
}
