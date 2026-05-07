use local_recorder::{CaptureDriver, CaptureWorker, LocalRecorderConfig};

#[test]
fn capture_worker_marks_driver_running_until_stopped() {
    let driver = CaptureDriver::new();

    assert!(!driver.running());

    CaptureWorker::start(&driver, LocalRecorderConfig::default()).unwrap();

    assert!(driver.running());

    CaptureWorker::stop(&driver);

    assert!(!driver.running());
}
