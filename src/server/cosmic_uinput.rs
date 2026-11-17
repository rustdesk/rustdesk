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
//! to the current output). So, when the pointer may not be on the target output, it is first
//! walked there with hops: an absolute move onto the edge an output shares with a neighbour,
//! then a small relative step over that edge.
//!
//! Which output holds the pointer is not observable, and a hop also runs on whichever output
//! the pointer happens to be on, where it may move it elsewhere. So the hop sequence is
//! planned by simulating cosmic-comp from every output at once: a breadth-first search over
//! the sets of outputs the pointer may be on, for the shortest sequence that leaves only the
//! target.

use scrap::wayland::display::DisplayRect;

/// The relative step used to cross an edge, in pixels before pointer acceleration. The
/// uinput service halves it into device units. The pointer sits on the edge itself, so any
/// motion in the right direction crosses; the margin only absorbs acceleration below 1.
pub const STEER_STEP: i32 = 16;

/// Above this many outputs no walk is planned (the search runs over subsets of outputs).
const MAX_PLANNED_OUTPUTS: usize = 16;

/// The uinput absolute range: `(minx, maxx, miny, maxy)`, as passed to
/// `update_mouse_resolution`.
pub type Range = (i32, i32, i32, i32);

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
/// Computed in i64: the point comes from the peer unchecked.
pub fn target_index(rects: &[DisplayRect], x: i32, y: i32) -> Option<usize> {
    let (x, y) = (x as i64, y as i64);
    rects
        .iter()
        .enumerate()
        .filter(|(_, r)| r.w > 0 && r.h > 0)
        .min_by_key(|(_, r)| {
            let (rx, ry) = (r.x as i64, r.y as i64);
            let dx = (rx - x).max(0).max(x - (rx + r.w as i64 - 1));
            let dy = (ry - y).max(0).max(y - (ry + r.h as i64 - 1));
            dx as i128 * dx as i128 + dy as i128 * dy as i128
        })
        .map(|(i, _)| i)
}

/// The uinput axis value that libinput maps to `offset` pixels into an output `size` pixels
/// long, for a device whose range is `min..=max`. libinput scales a value as
/// `(v - min) * size / (max - min + 1)`; rounding up keeps the result in
/// `[offset, offset + 1)`, so the point never lands one pixel short.
pub fn axis_value(offset: i64, size: i32, min: i32, max: i32) -> i32 {
    let range = (max as i64 - min as i64 + 1).max(1);
    let size = (size as i64).max(1);
    let offset = offset.clamp(0, size - 1);
    let v = min as i64 + (offset * range + size - 1) / size;
    v.clamp(min as i64, max as i64) as i32
}

/// `(x, y)` as a uinput value relative to the output `r`.
pub fn abs_value_on(r: &DisplayRect, x: i32, y: i32, range: Range) -> (i32, i32) {
    let (minx, maxx, miny, maxy) = range;
    (
        axis_value(x as i64 - r.x as i64, r.w, minx, maxx),
        axis_value(y as i64 - r.y as i64, r.h, miny, maxy),
    )
}

/// One hop of a walk: an absolute value that puts a pointer sitting on one output onto the
/// edge it shares with a neighbour, then the relative motion that crosses that edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hop {
    pub abs: (i32, i32),
    pub rel: (i32, i32),
}

/// What to inject, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Abs(i32, i32),
    Rel(i32, i32),
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

/// Every hop over every shared edge, in both directions.
fn candidate_hops(rects: &[DisplayRect], range: Range) -> Vec<Hop> {
    let mut hops = Vec::new();
    for (c, from) in rects.iter().enumerate() {
        for (p, to) in rects.iter().enumerate() {
            if c == p {
                continue;
            }
            if let Some(((x, y), rel)) = shared_edge(from, to) {
                let hop = Hop {
                    abs: abs_value_on(from, x, y, range),
                    rel,
                };
                if !hops.contains(&hop) {
                    hops.push(hop);
                }
            }
        }
    }
    hops
}

fn output_at(rects: &[DisplayRect], x: f64, y: f64) -> Option<usize> {
    rects.iter().position(|r| {
        x >= r.x as f64 && x < (r.x + r.w) as f64 && y >= r.y as f64 && y < (r.y + r.h) as f64
    })
}

/// Where cosmic-comp puts the pointer for the absolute value `v` while `o` is active
/// (libinput's transform onto that output).
fn landing(o: &DisplayRect, v: (i32, i32), range: Range) -> (f64, f64) {
    let (minx, maxx, miny, maxy) = range;
    let sx = (maxx as i64 - minx as i64 + 1).max(1) as f64;
    let sy = (maxy as i64 - miny as i64 + 1).max(1) as f64;
    (
        o.x as f64 + (v.0 as i64 - minx as i64) as f64 * o.w as f64 / sx,
        o.y as f64 + (v.1 as i64 - miny as i64) as f64 * o.h as f64 / sy,
    )
}

/// The outputs a pointer on output `o` may end on after `hop`. The relative step is scaled
/// by acceleration we do not know, so it is tried from a pixel to twice its size; a step
/// that lands outside every output is clamped to `o`.
fn hop_results(rects: &[DisplayRect], o: usize, hop: &Hop, range: Range) -> u64 {
    let (px, py) = landing(&rects[o], hop.abs, range);
    let len = (hop.rel.0.abs() + hop.rel.1.abs()).max(1) as f64;
    let (ux, uy) = (hop.rel.0 as f64 / len, hop.rel.1 as f64 / len);
    let mut out = 0u64;
    for s in [1.0, len, 2.0 * len] {
        let at = output_at(rects, px + ux * s, py + uy * s).unwrap_or(o);
        out |= 1 << at;
    }
    out
}

/// The shortest hop sequence that ends on `target` whichever output the pointer starts on.
/// None when there is none (an output with no shared edge), or too many outputs to search.
pub fn plan_walk(rects: &[DisplayRect], target: usize, range: Range) -> Option<Vec<Hop>> {
    let n = rects.len();
    if target >= n || n > MAX_PLANNED_OUTPUTS {
        return None;
    }
    let all: u64 = (0..n)
        .filter(|&i| rects[i].w > 0 && rects[i].h > 0)
        .fold(0, |m, i| m | 1 << i);
    let goal = 1u64 << target;
    if all & goal == 0 {
        return None;
    }
    if all == goal {
        return Some(Vec::new());
    }
    let hops = candidate_hops(rects, range);
    // Breadth-first over the sets of outputs the pointer may be on; `prev` keeps, for each
    // set reached, the set it came from and the hop taken.
    let mut prev: std::collections::HashMap<u64, (u64, usize)> = Default::default();
    let mut queue = std::collections::VecDeque::from([all]);
    while let Some(set) = queue.pop_front() {
        for (h, hop) in hops.iter().enumerate() {
            let next = (0..n)
                .filter(|&o| set & (1 << o) != 0)
                .fold(0u64, |m, o| m | hop_results(rects, o, hop, range));
            if next == all || prev.contains_key(&next) {
                continue;
            }
            prev.insert(next, (set, h));
            if next == goal {
                let mut walk = Vec::new();
                let mut at = next;
                while let Some(&(from, h)) = prev.get(&at) {
                    walk.push(hops[h]);
                    at = from;
                }
                walk.reverse();
                return Some(walk);
            }
            queue.push_back(next);
        }
    }
    None
}

/// Moves on the uinput device that put the pointer on `abs` of the target output: the walk,
/// if any, then the absolute value. The kernel drops an absolute event that repeats the
/// device's current value, even when relative steps moved the pointer since, so an absolute
/// value equal to the previous one (`last`, None when unknown) is preceded by a one-unit nudge.
pub fn steps(walk: &[Hop], abs: (i32, i32), last: Option<(i32, i32)>, range: Range) -> Vec<Step> {
    let mut out = Vec::with_capacity(walk.len() * 3 + 2);
    let mut last = last;
    let mut push_abs = |out: &mut Vec<Step>, v: (i32, i32)| {
        if last.map_or(true, |l| l == v) {
            let nx = if v.0 > range.0 { v.0 - 1 } else { v.0 + 1 };
            out.push(Step::Abs(nx, v.1));
        }
        out.push(Step::Abs(v.0, v.1));
        last = Some(v);
    };
    for hop in walk {
        push_abs(&mut out, hop.abs);
        out.push(Step::Rel(hop.rel.0, hop.rel.1));
    }
    push_abs(&mut out, abs);
    out
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

    fn desktop(rects: &[DisplayRect]) -> Range {
        let minx = rects.iter().map(|r| r.x).min().unwrap();
        let maxx = rects.iter().map(|r| r.x + r.w).max().unwrap();
        let miny = rects.iter().map(|r| r.y).min().unwrap();
        let maxy = rects.iter().map(|r| r.y + r.h).max().unwrap();
        (minx, maxx, miny, maxy)
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

    fn layouts() -> Vec<Vec<DisplayRect>> {
        vec![
            layout(),
            // Three in a row: the middle target is the case a fixed hop order gets wrong.
            vec![
                rect("A", 0, 0, 1920, 1080),
                rect("B", 1920, 0, 1920, 1080),
                rect("C", 3840, 0, 1920, 1080),
            ],
            // Two by two.
            vec![
                rect("A", 0, 0, 1920, 1080),
                rect("B", 1920, 0, 1920, 1080),
                rect("C", 0, 1080, 1920, 1080),
                rect("D", 1920, 1080, 1920, 1080),
            ],
            // A column of mixed sizes.
            vec![
                rect("A", 0, 0, 2560, 1440),
                rect("B", 320, 1440, 1920, 1080),
                rect("C", 0, 2520, 1024, 768),
            ],
        ]
    }

    /// An independent model of cosmic-comp: the kernel's dropping of repeated absolute
    /// values, the absolute value mapped onto the active output, a relative step of half
    /// its size switching output only when it lands inside another one.
    fn replay(
        rects: &[DisplayRect],
        start: usize,
        steps: &[Step],
        range: Range,
    ) -> (usize, f64, f64) {
        let mut out = start;
        let r = &rects[start];
        let (mut x, mut y) = (r.x as f64 + 1.0, r.y as f64 + 1.0);
        let mut dev: Option<(i32, i32)> = None;
        for step in steps {
            match *step {
                Step::Abs(vx, vy) => {
                    if dev == Some((vx, vy)) {
                        continue;
                    }
                    dev = Some((vx, vy));
                    let (lx, ly) = landing(&rects[out], (vx, vy), range);
                    x = lx;
                    y = ly;
                }
                Step::Rel(dx, dy) => {
                    let (nx, ny) = (x + dx as f64 / 2.0, y + dy as f64 / 2.0);
                    if let Some(o) = output_at(rects, nx, ny) {
                        out = o;
                        x = nx;
                        y = ny;
                    }
                }
            }
        }
        (out, x, y)
    }

    #[test]
    fn from_every_output_the_walk_ends_on_the_target_pixel() {
        for rects in layouts() {
            let range = desktop(&rects);
            for target in 0..rects.len() {
                let walk = plan_walk(&rects, target, range).expect("a walk exists");
                let t = &rects[target];
                let (px, py) = (t.x + t.w / 3, t.y + t.h - 2);
                let abs = abs_value_on(t, px, py, range);
                for start in 0..rects.len() {
                    for last in [None, Some(abs)] {
                        let (o, x, y) =
                            replay(&rects, start, &steps(&walk, abs, last, range), range);
                        assert_eq!(o, target, "{rects:?} start {start} target {target}");
                        assert!(x >= px as f64 && x < px as f64 + 1.0, "x {px} -> {x}");
                        assert!(y >= py as f64 && y < py as f64 + 1.0, "y {py} -> {y}");
                    }
                }
            }
        }
    }

    #[test]
    fn points_land_on_the_same_pixel_of_their_output() {
        let rects = layout();
        let range = desktop(&rects);
        for &(x, y) in &[
            (0, 0),
            (1920, 1080),
            (3839, 2159),
            (100, 2100),
            (2000, 3000),
            (4000, 300),
        ] {
            let i = target_index(&rects, x, y).unwrap();
            let (lx, ly) = landing(&rects[i], abs_value_on(&rects[i], x, y, range), range);
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
        // Extreme peer values do not overflow.
        assert_eq!(target_index(&rects, i32::MIN, i32::MIN), Some(2));
        assert_eq!(target_index(&rects, i32::MAX, i32::MAX), Some(0));
    }

    #[test]
    fn a_repeated_absolute_value_is_nudged_first() {
        let range = (0, 100, 0, 100);
        assert_eq!(
            steps(&[], (5, 5), Some((5, 5)), range),
            vec![Step::Abs(4, 5), Step::Abs(5, 5)]
        );
        assert_eq!(
            steps(&[], (5, 5), Some((6, 5)), range),
            vec![Step::Abs(5, 5)]
        );
        assert_eq!(
            steps(&[], (0, 0), None, range),
            vec![Step::Abs(1, 0), Step::Abs(0, 0)]
        );
    }

    #[test]
    fn a_single_output_needs_no_walk() {
        let rects = vec![rect("A", 0, 0, 1920, 1080)];
        assert_eq!(plan_walk(&rects, 0, (0, 1920, 0, 1080)), Some(Vec::new()));
    }

    #[test]
    fn an_output_without_a_shared_edge_has_no_walk() {
        let rects = vec![
            rect("A", 0, 0, 1920, 1080),
            rect("B", 1920, 1080, 1920, 1080),
        ];
        assert_eq!(plan_walk(&rects, 0, desktop(&rects)), None);
    }

    #[test]
    fn values_stay_in_range() {
        let r = rect("A", 0, 0, 3840, 2160);
        let range = (0, 4864, 0, 3240);
        assert_eq!(abs_value_on(&r, i32::MIN, i32::MIN, range), (0, 0));
        assert_eq!(abs_value_on(&r, i32::MAX, i32::MAX, range).0 <= 4864, true);
        assert_eq!(axis_value(-5, 3840, 0, 4864), 0);
    }
}
