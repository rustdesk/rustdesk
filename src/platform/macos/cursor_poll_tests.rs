use super::*;

#[test]
fn capture_failure_invalidates_the_density_poll() {
    let previous = unsafe { LATEST_SEED };
    unsafe {
        LATEST_SEED = (1, 2.0);
    }
    // Zero cannot be a macOS cursor ID, even if AppKit has no current cursor.
    let result = get_cursor_data(0);
    let after_failure = unsafe { LATEST_SEED };
    unsafe {
        LATEST_SEED = previous;
    }
    assert!(result.is_err());
    assert_eq!(after_failure, (0, 0.0));
}
