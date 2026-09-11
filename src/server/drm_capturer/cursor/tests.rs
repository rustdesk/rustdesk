use super::*;

fn capture() -> Capture {
    Capture {
        stop: None,
        thread: None,
        error: Arc::new(Mutex::new(None)),
        ready: Arc::new(Mutex::new(false)),
        wire_cursor: None,
    }
}

fn sprite(id: u64) -> DrmCursorData {
    DrmCursorData {
        id,
        width: 1,
        height: 1,
        hotx: 0,
        hoty: 0,
        colors: vec![255; 4],
    }
}

#[test]
fn wire_cursor_remains_active_until_a_sprite_is_published() {
    let capture = capture();
    assert!(!*capture.ready());
    let target = (-1614401, 1);
    publish(target, &capture.ready, sprite(7));
    assert!(*capture.ready());
    let mut cursors = super::super::DRM_CURSOR.lock().unwrap();
    assert_eq!(cursors.remove(&target.0).unwrap().1.id, 7);
}

#[test]
fn wire_publication_cannot_overwrite_the_first_mutter_sprite() {
    let capture = capture();
    let target = (-1614402, 1);
    let ready = capture.ready();
    assert!(!*ready);
    let worker_ready = capture.ready.clone();
    let (started, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        started.send(()).unwrap();
        publish(target, &worker_ready, sprite(9));
    });
    receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    super::super::set_drm_cursor(target.0, target.1, sprite(8));
    drop(ready);
    worker.join().unwrap();
    assert!(*capture.ready());
    let mut cursors = super::super::DRM_CURSOR.lock().unwrap();
    assert_eq!(cursors.remove(&target.0).unwrap().1.id, 9);
}
