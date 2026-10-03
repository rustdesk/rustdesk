use super::{AXIS_MIN, AXIS_X_MAX, AXIS_Y_MAX};
use std::io;

pub(super) type Positions = [(i32, i32); 2];

pub(super) fn initial_position(delta: i32, maximum: i32, margin: i32) -> i32 {
    if delta > 0 {
        AXIS_MIN + margin
    } else if delta < 0 {
        maximum - margin
    } else {
        (AXIS_MIN + maximum) / 2
    }
}

pub(super) fn moved_positions(positions: Positions, delta: (i32, i32)) -> io::Result<Positions> {
    let mut moved = positions;
    for position in &mut moved {
        position.0 = checked_coordinate(position.0, delta.0, ("x", AXIS_X_MAX))?;
        position.1 = checked_coordinate(position.1, delta.1, ("y", AXIS_Y_MAX))?;
    }
    Ok(moved)
}

pub(super) fn fitting_delta(positions: Positions, delta: (i32, i32)) -> (i32, i32) {
    let x_available =
        available_axis_distance([positions[0].0, positions[1].0], delta.0, AXIS_X_MAX);
    let y_available =
        available_axis_distance([positions[0].1, positions[1].1], delta.1, AXIS_Y_MAX);
    let x_needed = i64::from(delta.0).abs();
    let y_needed = i64::from(delta.1).abs();
    if x_needed <= x_available && y_needed <= y_available {
        return delta;
    }
    // Scale both axes by the first boundary reached so diagonal motion keeps its direction.
    if y_needed == 0 || (x_needed != 0 && x_available * y_needed <= y_available * x_needed) {
        scale_delta(delta, x_available, x_needed)
    } else {
        scale_delta(delta, y_available, y_needed)
    }
}

fn checked_coordinate(coordinate: i32, delta: i32, axis: (&str, i32)) -> io::Result<i32> {
    let (axis_name, maximum) = axis;
    let moved = coordinate.checked_add(delta).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("smooth scroll {axis_name} delta overflow"),
        )
    })?;
    if !(AXIS_MIN..=maximum).contains(&moved) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "smooth scroll exceeded virtual touchpad bounds",
        ));
    }
    Ok(moved)
}

fn available_axis_distance(positions: [i32; 2], delta: i32, maximum: i32) -> i64 {
    if delta > 0 {
        i64::from((maximum - positions[0]).min(maximum - positions[1]))
    } else if delta < 0 {
        i64::from((positions[0] - AXIS_MIN).min(positions[1] - AXIS_MIN))
    } else {
        0
    }
}

fn scale_delta(delta: (i32, i32), numerator: i64, denominator: i64) -> (i32, i32) {
    if denominator == 0 {
        return (0, 0);
    }
    (
        (i64::from(delta.0) * numerator / denominator) as i32,
        (i64::from(delta.1) * numerator / denominator) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_large_vertical_delta_in_both_directions() {
        const BOUNDS_EXCESS: i32 = 2_000;
        let positive_start = AXIS_MIN + super::super::EDGE_MARGIN;
        let negative_start = AXIS_Y_MAX - super::super::EDGE_MARGIN;
        let delta = AXIS_Y_MAX - super::super::EDGE_MARGIN + BOUNDS_EXCESS;
        let positive = fitting_delta([(AXIS_MIN, positive_start); 2], (0, delta));
        let negative = fitting_delta([(AXIS_MIN, negative_start); 2], (0, -delta));

        assert_eq!(positive, (0, AXIS_Y_MAX - positive_start));
        assert_eq!(negative, (0, -negative_start));
        assert_eq!(delta - positive.1, BOUNDS_EXCESS);
        assert_eq!(-delta - negative.1, -BOUNDS_EXCESS);
        assert!(moved_positions([(AXIS_MIN, positive_start); 2], positive).is_ok());
        assert!(moved_positions([(AXIS_MIN, negative_start); 2], negative).is_ok());
    }
}
