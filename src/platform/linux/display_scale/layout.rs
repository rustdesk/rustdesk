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

pub(super) fn plan(
    current: &[Rect],
    sizes: &[(i32, i32)],
    maximum: &[(i32, i32)],
    limit: (i32, i32),
    identities: &[String],
) -> ResultType<Vec<Rect>> {
    if current.is_empty()
        || current.len() != sizes.len()
        || current.len() != maximum.len()
        || current.len() != identities.len()
        || current.iter().any(|r| r.width <= 0 || r.height <= 0)
        || !valid(current, current)
        || sizes
            .iter()
            .zip(maximum)
            .any(|(&(w, h), &(mw, mh))| w <= 0 || h <= 0 || w > mw || h > mh)
    {
        bail!(UNSUPPORTED);
    }
    // Certify the entire mode/scale range before accepting any change, so
    // restoring a different combination does not depend on the path taken.
    let fits = |horizontal| {
        let (mut width, mut height) = (0i64, 0i64);
        for &(w, h) in maximum {
            if horizontal {
                width += i64::from(w);
                height = height.max(i64::from(h));
            } else {
                width = width.max(i64::from(w));
                height += i64::from(h);
            }
        }
        width <= i64::from(limit.0) && height <= i64::from(limit.1)
    };
    let horizontal = if fits(true) {
        true
    } else if fits(false) {
        false
    } else {
        bail!(UNSUPPORTED);
    };
    let within = |rects: &[Rect]| {
        rects.iter().all(|r| {
            r.x >= 0
                && r.y >= 0
                && r.right() <= i64::from(limit.0)
                && r.bottom() <= i64::from(limit.1)
        })
    };
    let changed: Vec<_> = current
        .iter()
        .zip(sizes)
        .enumerate()
        .filter(|(_, (r, size))| (r.width, r.height) != **size)
        .map(|(i, _)| i)
        .collect();
    if changed.len() == 1 {
        let index = changed[0];
        if let Ok(next) = resize(current, index, sizes[index]) {
            if within(&next) {
                return Ok(next);
            }
        }
    } else {
        let next: Vec<_> = current
            .iter()
            .zip(sizes)
            .map(|(r, &(width, height))| Rect {
                width,
                height,
                ..*r
            })
            .collect();
        if valid(current, &next) && within(&next) {
            return Ok(next);
        }
    }
    let mut order: Vec<_> = (0..current.len()).collect();
    order.sort_by(|&a, &b| identities[a].cmp(&identities[b]));
    if order
        .windows(2)
        .any(|p| identities[p[0]] == identities[p[1]])
    {
        bail!(UNSUPPORTED);
    }
    let mut next = current.to_vec();
    let mut offset = 0i64;
    for index in order {
        let (width, height) = sizes[index];
        let coordinate = i32::try_from(offset)?;
        next[index] = Rect {
            x: if horizontal { coordinate } else { 0 },
            y: if horizontal { 0 } else { coordinate },
            width,
            height,
        };
        offset += i64::from(if horizontal { width } else { height });
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangles(values: &[(i32, i32, i32, i32)]) -> Vec<Rect> {
        values
            .iter()
            .map(|&(x, y, width, height)| Rect {
                x,
                y,
                width,
                height,
            })
            .collect()
    }

    fn planned(current: &[Rect], sizes: &[(i32, i32)]) -> Vec<Rect> {
        plan(
            current,
            sizes,
            &vec![(16384, 16384); current.len()],
            (131072, 131072),
            &(0..current.len())
                .map(|i| i.to_string())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    #[test]
    fn original_after_consecutive_scale_changes() {
        for values in [
            [
                (1920, 0, 3840, 2160),
                (1920, 2160, 3840, 2160),
                (0, 2700, 1920, 1080),
                (840, 240, 1080, 1920),
            ],
            [
                (0, 0, 1920, 1080),
                (0, 1080, 1080, 1920),
                (1920, 0, 3840, 2160),
                (1920, 2160, 1280, 720),
            ],
        ] {
            for transpose in [false, true] {
                let current = rectangles(&values.map(|(x, y, w, h)| {
                    if transpose {
                        (y, x, h, w)
                    } else {
                        (x, y, w, h)
                    }
                }));
                let original: Vec<_> = current.iter().map(|r| (r.width, r.height)).collect();
                let mut sizes = original.clone();
                sizes[2] = (original[2].0 * 2 / 3, original[2].1 * 2 / 3);
                let scaled = planned(&current, &sizes);
                sizes[2] = (original[2].0 * 4 / 5, original[2].1 * 4 / 5);
                let scaled = planned(&scaled, &sizes);
                let restored = planned(&scaled, &original);
                assert_eq!(
                    restored
                        .iter()
                        .map(|r| (r.width, r.height))
                        .collect::<Vec<_>>(),
                    original
                );
            }
        }
    }

    #[test]
    fn originals_preserve_other_targets_and_batch_mode_changes() {
        let current = rectangles(&[
            (0, 0, 1280, 720),
            (0, 720, 1280, 720),
            (1280, 0, 2560, 1440),
            (1280, 1440, 2560, 1440),
        ]);
        let original: Vec<_> = current.iter().map(|r| (r.width, r.height)).collect();
        let mut sizes = original.clone();
        sizes[0] = (640, 360);
        let first = planned(&current, &sizes);
        sizes[2] = (1280, 720);
        let both = planned(&first, &sizes);
        for order in [[0, 2], [2, 0]] {
            let mut desired = sizes.clone();
            desired[order[0]] = original[order[0]];
            let next = planned(&both, &desired);
            assert_eq!(
                (next[order[1]].width, next[order[1]].height),
                sizes[order[1]]
            );
            let restored = planned(&next, &original);
            assert_eq!(
                restored
                    .iter()
                    .map(|r| (r.width, r.height))
                    .collect::<Vec<_>>(),
                original
            );
        }
        // A mode change and restoring a pre-existing 50% scale can enlarge
        // several screens at once; no intermediate per-screen layout is needed.
        let larger = vec![(3840, 2160), (1280, 720), (5120, 2880), (2560, 1440)];
        let changed = planned(&both, &larger);
        let restored = planned(&changed, &original);
        assert_eq!(
            restored
                .iter()
                .map(|r| (r.width, r.height))
                .collect::<Vec<_>>(),
            original
        );
        assert!(plan(
            &current,
            &sizes,
            &vec![(7680, 4320); 4],
            (16384, 16384),
            &["a".into(), "b".into(), "c".into(), "d".into()]
        )
        .is_err());
    }

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
