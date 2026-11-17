//! Absolute pointer injection for compositors that map a uinput absolute device onto the
//! output under the pointer, not onto the whole desktop.
//!
//! RustDesk programs its uinput mouse with an absolute range covering the whole desktop
//! (the bounding box of every output) and sends whole-desktop coordinates, which is how
//! mutter and KWin map such a device. cosmic-comp (COSMIC) instead maps the device's full
//! range onto the seat's *active output* (`src/input/mod.rs`, `PointerMotionAbsolute`, the
//! non-EI branch). With more than one monitor every point lands scaled by
//! `output size / desktop size` and on whichever monitor holds the pointer, so clicks miss
//! more and more towards the bottom and right of the shared screen.
//!
//! Here a whole-desktop point is turned into a value of the uinput range relative to the
//! output that contains it. Absolute motion cannot leave the active output on COSMIC, but
//! relative motion can when one step lands inside another output (a larger step is clamped
//! to the current output). So, when the target output may not be the active one, the pointer
//! is first walked across the shared edges towards it: placed on the edge of a neighbouring
//! output, then nudged over the edge.

use scrap::wayland::display::DisplayRect;

/// The relative step used to cross an edge, in pixels before pointer acceleration. The
/// uinput service halves it into device units. The pointer sits on the edge itself, so any
/// motion in the right direction crosses; the margin only absorbs acceleration below 1.
pub const STEER_STEP: i32 = 16;

/// Set `RUSTDESK_UINPUT_PER_OUTPUT=1` to force this mapping on another compositor that
/// behaves like COSMIC, or `=0` to turn it off.
pub fn compositor_maps_abs_per_output() -> bool {
    lazy_static::lazy_static! {
        static ref PER_OUTPUT: bool = detect();
    }
    *PER_OUTPUT
}

fn detect() -> bool {
    match std::env::var("RUSTDESK_UINPUT_PER_OUTPUT").as_deref() {
        Ok("1") | Ok("true") => return true,
        Ok("0") | Ok("false") => return false,
        _ => {}
    }
    if std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.split(':').any(|p| p.eq_ignore_ascii_case("COSMIC")))
        .unwrap_or(false)
    {
        return true;
    }
    // The `--server` process is started without the session's XDG_CURRENT_DESKTOP, so look
    // for the compositor itself among this user's processes.
    let uid = unsafe { hbb_common::libc::getuid() };
    let found = cosmic_comp_running(uid);
    hbb_common::log::info!("uinput per-output absolute mapping (cosmic-comp running): {found}");
    found
}

fn cosmic_comp_running(uid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return false;
    };
    dir.flatten().any(|entry| {
        let is_pid = entry
            .file_name()
            .to_str()
            .map(|n| n.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or(false);
        is_pid
            && entry.metadata().map(|m| m.uid() == uid).unwrap_or(false)
            && std::fs::read_to_string(entry.path().join("comm"))
                .map(|c| c.trim() == "cosmic-comp")
                .unwrap_or(false)
    })
}

/// The output that contains `(x, y)`, or the nearest one for a point in a gap or outside.
pub fn target_index(rects: &[DisplayRect], x: i32, y: i32) -> Option<usize> {
    rects
        .iter()
        .enumerate()
        .filter(|(_, r)| r.w > 0 && r.h > 0)
        .min_by_key(|(_, r)| {
            let dx = (r.x - x).max(0).max(x - (r.x + r.w - 1));
            let dy = (r.y - y).max(0).max(y - (r.y + r.h - 1));
            dx as i64 * dx as i64 + dy as i64 * dy as i64
        })
        .map(|(i, _)| i)
}

/// The uinput axis value that libinput maps to `offset` pixels into an output `size` pixels
/// long, for a device whose range is `min..=max`. libinput scales a value as
/// `(v - min) * size / (max - min + 1)`; rounding up keeps the result in
/// `[offset, offset + 1)`, so the point never lands one pixel short.
pub fn axis_value(offset: i32, size: i32, min: i32, max: i32) -> i32 {
    let range = (max as i64 - min as i64 + 1).max(1);
    let size = (size as i64).max(1);
    let offset = (offset as i64).clamp(0, size - 1);
    let v = min as i64 + (offset * range + size - 1) / size;
    v.clamp(min as i64, max as i64) as i32
}

/// `(x, y)` as a uinput value relative to the output `r`, for the uinput range `range`
/// (`minx, maxx, miny, maxy`, as passed to `update_mouse_resolution`).
pub fn abs_value_on(r: &DisplayRect, x: i32, y: i32, range: (i32, i32, i32, i32)) -> (i32, i32) {
    let (minx, maxx, miny, maxy) = range;
    (
        axis_value(x - r.x, r.w, minx, maxx),
        axis_value(y - r.y, r.h, miny, maxy),
    )
}

/// One hop of a walk: an absolute value that puts a pointer sitting on output `from` onto its
/// edge shared with the next output, then the relative motion that crosses that edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hop {
    pub abs: (i32, i32),
    pub rel: (i32, i32),
}

/// The edge `from` shares with `to`, as the point on `from` next to it (in desktop
/// coordinates) and the direction that crosses it. None when they only touch at a corner.
fn shared_edge(from: &DisplayRect, to: &DisplayRect) -> Option<((i32, i32), (i32, i32))> {
    let overlap = |a0: i32, a1: i32, b0: i32, b1: i32| {
        let lo = a0.max(b0);
        let hi = a1.min(b1);
        (lo < hi).then(|| lo + (hi - lo) / 2)
    };
    let mid_x = || overlap(from.x, from.x + from.w, to.x, to.x + to.w);
    let mid_y = || overlap(from.y, from.y + from.h, to.y, to.y + to.h);
    if to.y + to.h == from.y {
        mid_x().map(|x| ((x, from.y), (0, -STEER_STEP)))
    } else if from.y + from.h == to.y {
        mid_x().map(|x| ((x, from.y + from.h - 1), (0, STEER_STEP)))
    } else if to.x + to.w == from.x {
        mid_y().map(|y| ((from.x, y), (-STEER_STEP, 0)))
    } else if from.x + from.w == to.x {
        mid_y().map(|y| ((from.x + from.w - 1, y), (STEER_STEP, 0)))
    } else {
        None
    }
}

/// The hops that bring the pointer from any output onto `target`: for every other output,
/// the hop to its neighbour one step closer to `target`, farthest outputs first, so a pointer
/// moved one step closer is moved again by a later hop. A hop meant for another output also
/// runs while the pointer sits elsewhere; it then only nudges the pointer from an edge point,
/// which the relative step clamps at the edge of the output unless an output lies beyond it.
pub fn steer_hops(rects: &[DisplayRect], target: usize, range: (i32, i32, i32, i32)) -> Vec<Hop> {
    if target >= rects.len() {
        return Vec::new();
    }
    // Breadth-first from the target over shared edges.
    let mut parent: Vec<Option<usize>> = vec![None; rects.len()];
    let mut dist: Vec<Option<usize>> = vec![None; rects.len()];
    dist[target] = Some(0);
    let mut queue = std::collections::VecDeque::from([target]);
    while let Some(p) = queue.pop_front() {
        for c in 0..rects.len() {
            if dist[c].is_none() && shared_edge(&rects[c], &rects[p]).is_some() {
                dist[c] = Some(dist[p].unwrap() + 1);
                parent[c] = Some(p);
                queue.push_back(c);
            }
        }
    }
    let mut order: Vec<usize> = (0..rects.len())
        .filter(|&c| c != target && dist[c].is_some())
        .collect();
    order.sort_by_key(|&c| std::cmp::Reverse(dist[c]));
    order
        .into_iter()
        .filter_map(|c| {
            let from = &rects[c];
            let ((x, y), rel) = shared_edge(from, &rects[parent[c]?])?;
            Some(Hop {
                abs: abs_value_on(from, x, y, range),
                rel,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(name: &str, x: i32, y: i32, w: i32, h: i32) -> DisplayRect {
        DisplayRect {
            name: name.to_owned(),
            x,
            y,
            w,
            h,
            transform: 0,
        }
    }

    // The layout this was found on: a 4K monitor at the origin, a 1080p one below its right
    // half and a small one to its right. Desktop rect 4864x3240.
    fn layout() -> Vec<DisplayRect> {
        vec![
            rect("DVI-I-1", 1920, 2160, 1920, 1080),
            rect("DVI-I-2", 3840, 0, 1024, 600),
            rect("HDMI-A-1", 0, 0, 3840, 2160),
        ]
    }
    const RANGE: (i32, i32, i32, i32) = (0, 4864, 0, 3240);

    // What cosmic-comp does with a value: libinput's transform onto the active output.
    fn land(r: &DisplayRect, v: (i32, i32), range: (i32, i32, i32, i32)) -> (f64, f64) {
        let sx = (range.1 - range.0 + 1) as f64;
        let sy = (range.3 - range.2 + 1) as f64;
        (
            r.x as f64 + (v.0 - range.0) as f64 * r.w as f64 / sx,
            r.y as f64 + (v.1 - range.2) as f64 * r.h as f64 / sy,
        )
    }

    #[test]
    fn points_land_on_the_same_pixel_of_their_output() {
        let rects = layout();
        for &(x, y) in &[
            (0, 0),
            (1920, 1080),
            (3839, 2159),
            (100, 2100),
            (2000, 3000),
            (4000, 300),
        ] {
            let i = target_index(&rects, x, y).unwrap();
            let (lx, ly) = land(&rects[i], abs_value_on(&rects[i], x, y, RANGE), RANGE);
            assert!(lx >= x as f64 && lx < x as f64 + 1.0, "x {x} -> {lx}");
            assert!(ly >= y as f64 && ly < y as f64 + 1.0, "y {y} -> {ly}");
        }
    }

    #[test]
    fn the_target_is_the_containing_output_or_the_nearest() {
        let rects = layout();
        assert_eq!(target_index(&rects, 10, 2150), Some(2));
        assert_eq!(target_index(&rects, 1920, 2160), Some(0));
        assert_eq!(target_index(&rects, 3840, 0), Some(1));
        // The empty corner below the 4K monitor's left half: its bottom edge is nearest.
        assert_eq!(target_index(&rects, 100, 3000), Some(2));
    }

    #[test]
    fn hops_cross_into_the_target_from_each_neighbour() {
        let rects = layout();
        let hops = steer_hops(&rects, 2, RANGE);
        assert_eq!(hops.len(), 2);
        for hop in &hops {
            // Run the hop as if the pointer were on each neighbour; one of them must cross.
            let crossed = [0usize, 1].iter().any(|&c| {
                let (px, py) = land(&rects[c], hop.abs, RANGE);
                let (nx, ny) = (px + hop.rel.0 as f64 / 4.0, py + hop.rel.1 as f64 / 4.0);
                let t = &rects[2];
                nx >= t.x as f64
                    && nx < (t.x + t.w) as f64
                    && ny >= t.y as f64
                    && ny < (t.y + t.h) as f64
            });
            assert!(crossed, "{hop:?}");
        }
    }

    #[test]
    fn far_outputs_hop_first() {
        // A row of three: the leftmost is two hops from the rightmost.
        let rects = vec![
            rect("A", 0, 0, 100, 100),
            rect("B", 100, 0, 100, 100),
            rect("C", 200, 0, 100, 100),
        ];
        let range = (0, 300, 0, 100);
        let hops = steer_hops(&rects, 2, range);
        assert_eq!(hops.len(), 2);
        // First A -> B (A's right edge), then B -> C.
        assert_eq!(land(&rects[0], hops[0].abs, range).0.floor(), 99.0);
        assert_eq!(hops[0].rel, (STEER_STEP, 0));
        assert_eq!(land(&rects[1], hops[1].abs, range).0.floor(), 199.0);
    }

    #[test]
    fn a_single_output_needs_no_walk() {
        let rects = vec![rect("A", 0, 0, 1920, 1080)];
        assert!(steer_hops(&rects, 0, (0, 1920, 0, 1080)).is_empty());
    }

    #[test]
    fn axis_value_stays_in_range() {
        assert_eq!(axis_value(0, 3840, 0, 4864), 0);
        assert_eq!(axis_value(5000, 3840, 0, 4864), 4864);
        assert_eq!(axis_value(-5, 3840, 0, 4864), 0);
    }
}
