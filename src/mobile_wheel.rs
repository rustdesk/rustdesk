pub(crate) fn mobile_wheel_delta(xy: (i32, i32), reverse_mouse_wheel: &str) -> (i32, i32) {
    if reverse_mouse_wheel == "Y" {
        (-xy.0, -xy.1)
    } else {
        xy
    }
}

#[cfg(test)]
mod tests {
    use super::mobile_wheel_delta;

    #[test]
    fn reverses_both_axes_when_enabled() {
        assert_eq!(mobile_wheel_delta((2, -3), "Y"), (-2, 3));
    }

    #[test]
    fn preserves_both_axes_when_disabled() {
        assert_eq!(mobile_wheel_delta((2, -3), "N"), (2, -3));
        assert_eq!(mobile_wheel_delta((2, -3), ""), (2, -3));
    }
}
