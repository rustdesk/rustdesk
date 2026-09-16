use super::UNSUPPORTED;
use hbb_common::{bail, ResultType};

#[derive(Clone, Copy)]
pub(super) struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    fn overlaps(self, other: Self) -> bool {
        i64::from(self.x) < other.right()
            && i64::from(other.x) < self.right()
            && i64::from(self.y) < other.bottom()
            && i64::from(other.y) < self.bottom()
    }

    fn adjacent(self, other: Self) -> bool {
        ((self.right() == i64::from(other.x) || other.right() == i64::from(self.x))
            && i64::from(self.y) < other.bottom()
            && i64::from(other.y) < self.bottom())
            || ((self.bottom() == i64::from(other.y) || other.bottom() == i64::from(self.y))
                && i64::from(self.x) < other.right()
                && i64::from(other.x) < self.right())
    }
}

fn valid(current: &[Rect], next: &[Rect]) -> bool {
    let mut groups = vec![usize::MAX; next.len()];
    for start in 0..next.len() {
        if groups[start] != usize::MAX {
            continue;
        }
        groups[start] = start;
        let mut pending = vec![start];
        while let Some(index) = pending.pop() {
            for other in 0..next.len() {
                if groups[other] == usize::MAX && next[index].adjacent(next[other]) {
                    groups[other] = start;
                    pending.push(other);
                }
            }
        }
    }
    for i in 0..next.len() {
        for j in i + 1..next.len() {
            if next[i].overlaps(next[j])
                || (current[i].adjacent(current[j]) && groups[i] != groups[j])
            {
                return false;
            }
        }
    }
    true
}

pub(super) fn resize(current: &[Rect], index: usize, size: (i32, i32)) -> ResultType<Vec<Rect>> {
    let Some(target) = current.get(index) else {
        bail!(UNSUPPORTED);
    };
    if size.0 <= 0
        || size.1 <= 0
        || current.iter().any(|r| r.width <= 0 || r.height <= 0)
        || !valid(current, current)
    {
        bail!(UNSUPPORTED);
    }
    let dx = i64::from(size.0) - i64::from(target.width);
    let dy = i64::from(size.1) - i64::from(target.height);
    // A screen spanning two rows/columns can collide with a neighbor if both
    // axes move. Prefer the existing placement, then try each axis separately.
    for (horizontal, vertical) in [(true, true), (true, false), (false, true), (false, false)] {
        let next: Option<Vec<_>> = current
            .iter()
            .enumerate()
            .map(|(i, rect)| {
                let mut rect = *rect;
                if i == index {
                    (rect.width, rect.height) = size;
                } else {
                    if horizontal && i64::from(rect.x) >= target.right() {
                        rect.x = (i64::from(rect.x) + dx).try_into().ok()?;
                    }
                    if vertical && i64::from(rect.y) >= target.bottom() {
                        rect.y = (i64::from(rect.y) + dy).try_into().ok()?;
                    }
                }
                Some(rect)
            })
            .collect();
        if let Some(next) = next.filter(|next| valid(current, next)) {
            return Ok(next);
        }
    }
    bail!(UNSUPPORTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_reachable_layouts_or_rejects_the_change() {
        for (rects, size, expected) in [
            (
                vec![
                    (0, 0, 1920, 1080),
                    (1920, 0, 1920, 1080),
                    (0, 1080, 3840, 1080),
                ],
                (1280, 720),
                Some(vec![(0, 0), (1280, 0), (0, 1080)]),
            ),
            (
                vec![
                    (0, 0, 1080, 1920),
                    (0, 1920, 1080, 1920),
                    (1080, 0, 1080, 3840),
                ],
                (720, 1280),
                Some(vec![(0, 0), (0, 1280), (1080, 0)]),
            ),
            (
                vec![(-1920, 0, 1920, 1080), (0, 0, 1920, 1080)],
                (1280, 720),
                Some(vec![(-1920, 0), (-640, 0)]),
            ),
            (
                vec![(0, 0, 100, 100), (300, 0, 100, 100)],
                (50, 50),
                Some(vec![(0, 0), (250, 0)]),
            ),
            (vec![(0, 0, 100, 100), (100, 50, 100, 50)], (50, 50), None),
            (vec![(0, 0, 100, 100), (50, 0, 100, 100)], (50, 50), None),
            (
                vec![(i32::MAX - 100, 0, 100, 100), (i32::MAX, 0, 100, 100)],
                (200, 100),
                None,
            ),
        ] {
            let current: Vec<_> = rects
                .into_iter()
                .map(|(x, y, width, height)| Rect {
                    x,
                    y,
                    width,
                    height,
                })
                .collect();
            let result = resize(&current, 0, size);
            match expected {
                Some(positions) => {
                    let next = result.unwrap();
                    assert_eq!(
                        next.iter().map(|r| (r.x, r.y)).collect::<Vec<_>>(),
                        positions
                    );
                    assert_eq!((next[0].width, next[0].height), size);
                }
                None => assert!(result.is_err()),
            }
        }
    }
}
