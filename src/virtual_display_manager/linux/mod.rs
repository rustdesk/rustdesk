// A Linux box with nothing plugged in has no enabled connector, so there is no CRTC, no scanout,
// and nothing for capture to read: the session falls back to the portal, which asks for consent on
// a screen nobody is looking at. The kernel can force a disconnected connector on, which hands the
// compositor an ordinary output on the device it is already driving, so capture takes its normal
// path with no virtual driver involved.
//
// Deliberately conservative, because this writes to sysfs from the root service:
//   - off unless the operator turns it on, on every build including the unattended one;
//   - only a connector that currently reports no display is ever forced, and only where a
//     compositor can actually drive it (`promote_split_topology` for what that means per card);
//   - an `edid_firmware` entry someone else configured is preserved, and if theirs already covers
//     the connector we would have picked, we force nothing at all;
//   - the release path gives back the connector this process forced, plus any connector still
//     carrying our EDID from a previous run of the service, and nothing else.
//
// Known limit, stated because sysfs cannot express it: the kernel has no attribute that reports
// `connector->force`, so a connector an operator forced *off* by hand is indistinguishable from one
// with nothing plugged into it, and this code may force it back on. There is also no way to see a
// monitor plugged into the connector we are holding, because our own EDID override is what that
// connector reports; plugging into any other connector is noticed within a poll.

use hbb_common::{anyhow::anyhow, bail, config::Config, log, ResultType};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// User-facing switch, kept in this crate the same way `enable-drm-display-wake` is: it belongs to
/// a compile-gated Linux backend rather than to the shared config vocabulary.
///
/// `allow-` and not `enable-` is load-bearing. `option2bool` reads an `enable-` key as `value != "N"`
/// - an absent value would default this ON - and an `allow-` key as `value == "Y"`, which is off
/// until somebody says otherwise. The prefix is what decides it, so the key does not have to be
/// registered anywhere for that to hold.
pub const OPTION_ALLOW_HEADLESS_DISPLAY: &str = "allow-headless-display";

const DRM_CLASS: &str = "/sys/class/drm";
const EDID_PARAM: &str = "/sys/module/drm/parameters/edid_firmware";
const EDID_DIR: &str = "/lib/firmware/edid";
const EDID_NAME_PREFIX: &str = "rustdesk-headless";
/// The relative name the kernel resolves under the firmware search path.
const EDID_DIR_REF: &str = "edid/";

/// How long the machine must report no output before a connector is forced. Short enough that a
/// headless boot is remotable quickly, long enough that a monitor waking up, a KVM switching back
/// or a driver rebinding wins the race and nothing is forced at all.
const NO_OUTPUT_STABLE: Duration = Duration::from_secs(5);
/// sysfs re-read interval. The service loop ticks twice a second; this feature does not need to.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How long to wait for a forced connector to report a display before saying so in the log.
const SETTLE_TIMEOUT: Duration = Duration::from_millis(1500);
const SETTLE_STEP: Duration = Duration::from_millis(50);
/// How long a real display has to keep reporting itself before ours is released. A connector flips
/// its sysfs `status` within milliseconds of the hotplug, but the compositor needs longer to commit
/// a mode on it, and releasing in between publishes a topology where the new display has no CRTC yet
/// and ours is already gone - which reads downstream as "this machine has no displays" and drops a
/// live session to the portal. Deliberately a delay and not a wait for the new connector to be
/// CRTC-bound: on single-CRTC hardware the compositor cannot bind it until ours is released, so that
/// condition would never come true.
const REAL_OUTPUT_STABLE: Duration = Duration::from_secs(2);
/// Backoff after a failed attempt. Nothing that makes `enable` fail is transient - a topology with
/// no forceable connector stays that way - so retrying on the next poll would only produce a warning
/// every few seconds for the life of the service.
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(300);

struct Connector {
    /// Kernel connector name, e.g. `HDMI-A-1`. This is what `edid_firmware` matches on, and it
    /// carries no card qualifier: the parameter has no syntax for one.
    name: String,
    /// `<card>-<connector>`, the sysfs directory name.
    sysfs: String,
    connected: bool,
    /// The connector is reporting our own synthetic EDID, so this is one we forced.
    ours: bool,
    /// The card's stable bus identity, see `device_id`. Ownership is recorded against this and
    /// not against the connector name alone, which the kernel re-issues after a rebind.
    device: String,
    /// A compositor can actually drive this output: its card has a render node, or the whole
    /// machine splits scanout and rendering between cards (`promote_split_topology`).
    drivable: bool,
}

fn read_trim(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_owned())
}

/// Does this card have a render node? A display-only DRM device can scan out but nothing can render
/// to it, so an output forced there is one no compositor will drive - and an output already present
/// there is not one an operator can work on either. Measured on a 2018 MacBook Pro, where `card0` is
/// the Touch Bar (`card0-USB-1`, no render node) and the two GPUs are `card1` and `card2`.
fn card_is_renderable(card: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(Path::new(DRM_CLASS).join(card).join("device/drm")) else {
        // No such directory means we cannot tell. Treat that as usable rather than refusing to work
        // on a driver that lays sysfs out differently.
        return true;
    };
    entries
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with("renderD"))
}

/// The stable identity of the DRM device a card sits on: the name of the bus device it hangs off
/// (`0000:01:00.0` on PCI, `fec00000.v3d` on a SoC), sanitised so it can live in a file name.
/// `cardN` cannot play this role - the minor comes from an allocator that recycles, measured on a
/// MacBook whose Touch Bar connector was `card0-USB-1` one boot and `card3-USB-2` the next.
///
/// `unknown` when sysfs does not say. Two such cards then share an identity, which is the
/// unqualified behaviour rather than a refusal to work.
fn device_id(card: &str) -> String {
    let sanitised = std::fs::read_link(Path::new(DRM_CLASS).join(card).join("device"))
        .ok()
        .and_then(|t| t.file_name().map(|n| n.to_string_lossy().into_owned()))
        .map(|n| {
            n.chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect::<String>()
        })
        .unwrap_or_default();
    if sanitised.is_empty() {
        "unknown".to_owned()
    } else {
        sanitised
    }
}

/// Every connector of every card, with the facts the rest of this module decides on.
fn connectors() -> Vec<Connector> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(DRM_CLASS) else {
        return out;
    };
    // One render-node lookup per card, not per connector: a machine with eight connectors on one
    // card would otherwise walk the same directory eight times every poll.
    let mut renderable_cache: HashMap<String, bool> = HashMap::new();
    let named = connectors_named_by_our_entries();
    // Two passes, because the name fallback needs to know whether a name is unique in the topology
    // before trusting it (`connector_is_ours`).
    let mut raw = Vec::new();
    let mut name_count: HashMap<String, usize> = HashMap::new();
    for e in entries.flatten() {
        let dir = e.path();
        let sysfs = e.file_name().to_string_lossy().into_owned();
        // cardN-CONNECTOR. cardN itself, renderD* and the version node have no status file.
        if !dir.join("status").exists() {
            continue;
        }
        let (card, name) = match sysfs.split_once('-') {
            Some((card, name)) => (card.to_owned(), name.to_owned()),
            None => continue,
        };
        *name_count.entry(name.clone()).or_default() += 1;
        raw.push((dir, sysfs, card, name));
    }
    let mut device_cache: HashMap<String, String> = HashMap::new();
    for (dir, sysfs, card, name) in raw {
        let connected = read_trim(&dir.join("status")).as_deref() == Some("connected");
        let drivable = *renderable_cache
            .entry(card.clone())
            .or_insert_with_key(|c| card_is_renderable(c));
        let device = device_cache
            .entry(card)
            .or_insert_with_key(|c| device_id(c))
            .clone();
        out.push(Connector {
            ours: connector_is_ours(
                connected,
                &std::fs::read(dir.join("edid")).unwrap_or_default(),
                &name,
                &device,
                &named,
                name_count.get(&name).copied().unwrap_or(1) == 1,
            ),
            name,
            sysfs,
            connected,
            device,
            drivable,
        });
    }
    promote_split_topology(&mut out, system_has_render_node());
    out.sort_by(|a, b| a.sysfs.cmp(&b.sysfs));
    out
}

/// A Raspberry Pi-style SoC splits the work between two DRM devices: every connector sits on a
/// display card with no render node (vc4) and the GPU has no connectors (v3d). There the
/// display-only card is not a Touch Bar-like dead end but the machine's one scanout path, so as
/// long as something in the system can render at all its connectors are drivable. On a machine
/// where any connector already sits on a card with a render node this changes nothing.
fn promote_split_topology(all: &mut [Connector], system_has_render_node: bool) {
    if system_has_render_node && !all.iter().any(|c| c.drivable) {
        for c in all.iter_mut() {
            c.drivable = true;
        }
    }
}

fn system_has_render_node() -> bool {
    std::fs::read_dir(DRM_CLASS)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("renderD"))
        })
        .unwrap_or(false)
}

/// True when nothing reports a display an operator could work on. A connector we forced does not
/// count, or the tick could never tell that its own output is all there is. Neither does a
/// display-only card, for the same reason `pick_connector` will not force one.
///
/// Deliberately not "no scanout": a display that is merely asleep still reports connected, and
/// waking it is a different job.
fn no_real_output(all: &[Connector]) -> bool {
    !all.is_empty() && !all.iter().any(is_real_output)
}

fn is_real_output(c: &Connector) -> bool {
    // A writeback connector can read `connected` (a capture sink, not a display anyone watches),
    // and counting it would both block arming and prematurely release a held connector.
    c.connected && !c.ours && c.drivable && !c.name.starts_with("Writeback")
}

pub fn is_supported() -> bool {
    Path::new(DRM_CLASS).exists() && !connectors().is_empty()
}

fn is_enabled() -> bool {
    Config::get_bool_option(OPTION_ALLOW_HEADLESS_DISPLAY)
}

// ---------------------------------------------------------------------------------------------
// The synthetic EDID
//
// Two jobs. Without an EDID the kernel invents a default mode list that tops out at 1024x768, so
// this one carries a real mode list. And because it is ours and recognisable, it doubles as the
// record of which connector was forced by an earlier run of this service - a connector released by
// hand or by a reboot stops looking like ours in the same instant, with no state file to go stale.
//
// Generated rather than shipped: a real monitor's EDID carries that monitor's vendor id and serial
// number and is not ours to redistribute.
// ---------------------------------------------------------------------------------------------

/// 5-bit-packed "RDK".
const EDID_MFG: u16 = ((b'R' as u16 - 64) << 10) | ((b'D' as u16 - 64) << 5) | (b'K' as u16 - 64);
const EDID_PRODUCT: u16 = 0x0001;
const EDID_MONITOR_NAME: &[u8] = b"RustDesk VD";
/// Offset of the second 18-byte descriptor, which holds the monitor name.
const EDID_NAME_DESCRIPTOR: usize = 72;

/// One standard-timing entry: `(hactive, aspect_code, refresh)`. Aspect codes are the EDID ones:
/// 0 = 16:10, 1 = 4:3, 2 = 5:4, 3 = 16:9.
const STD_TIMINGS: [(u16, u8, u8); 8] = [
    (1680, 0, 60),
    (1600, 3, 60),
    (1440, 0, 60),
    (1280, 2, 60),
    (1280, 3, 60),
    (1152, 1, 60),
    (1024, 1, 60),
    (800, 1, 60),
];

fn synthetic_edid() -> Vec<u8> {
    let mut e = vec![0u8; 128];
    e[0..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
    e[8] = (EDID_MFG >> 8) as u8;
    e[9] = (EDID_MFG & 0xFF) as u8;
    e[10] = (EDID_PRODUCT & 0xFF) as u8;
    e[11] = (EDID_PRODUCT >> 8) as u8;
    // Serial number left at zero, which EDID defines as unspecified. We are not inventing one.
    e[16] = 1; // week
    e[17] = 36; // year 1990 + 36
    e[18] = 1; // EDID 1.4
    e[19] = 4;
    e[20] = 0x80; // digital input
    e[21] = 53; // 53 cm wide
    e[22] = 30; // 30 cm high
    e[23] = 0x78; // gamma 2.2
    e[24] = 0x0A; // RGB 4:4:4 + YCrCb 4:4:4, first detailed descriptor is the preferred timing
                  // Chromaticity, roughly sRGB. Cosmetic, but a parser dislikes an all-zero block.
    e[25..35].copy_from_slice(&[0xEE, 0x91, 0xA3, 0x54, 0x4C, 0x99, 0x26, 0x0F, 0x50, 0x54]);
    // Established timings: 640x480@60, 800x600@60, 1024x768@60.
    e[35] = 0x21;
    e[36] = 0x08;
    e[37] = 0x00;
    for (i, (h, aspect, refresh)) in STD_TIMINGS.iter().enumerate() {
        e[38 + i * 2] = ((h / 8) - 31) as u8;
        e[39 + i * 2] = (aspect << 6) | (refresh - 60);
    }
    // Descriptor 1: 1920x1080 @ 60 Hz, 148.5 MHz - the CEA-861 timing every display understands.
    e[54..72].copy_from_slice(&detailed_timing_1080p());
    // Descriptor 2: the monitor name, so it shows up as something readable in a display panel. The
    // spec terminates a short string with 0x0A and pads with spaces.
    let d = EDID_NAME_DESCRIPTOR;
    e[d + 3] = 0xFC; // monitor name tag
    e[d + 5..d + 5 + EDID_MONITOR_NAME.len()].copy_from_slice(EDID_MONITOR_NAME);
    e[d + 5 + EDID_MONITOR_NAME.len()] = 0x0A;
    for b in e[d + 6 + EDID_MONITOR_NAME.len()..d + 18].iter_mut() {
        *b = 0x20;
    }
    // Descriptors 3 and 4: unused, which EDID spells as a dummy descriptor rather than zeros.
    for base in [90usize, 108] {
        e[base + 3] = 0x10;
    }
    e[126] = 0; // no extension blocks
    let sum = e[..127].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    e[127] = 0u8.wrapping_sub(sum);
    e
}

fn detailed_timing_1080p() -> [u8; 18] {
    const CLOCK_10KHZ: u16 = 14850;
    const H_ACTIVE: u16 = 1920;
    const H_BLANK: u16 = 280;
    const V_ACTIVE: u16 = 1080;
    const V_BLANK: u16 = 45;
    const H_SYNC_OFF: u16 = 88;
    const H_SYNC_W: u16 = 44;
    const V_SYNC_OFF: u16 = 4;
    const V_SYNC_W: u16 = 5;
    const H_MM: u16 = 531;
    const V_MM: u16 = 299;
    [
        (CLOCK_10KHZ & 0xFF) as u8,
        (CLOCK_10KHZ >> 8) as u8,
        (H_ACTIVE & 0xFF) as u8,
        (H_BLANK & 0xFF) as u8,
        (((H_ACTIVE >> 8) << 4) | (H_BLANK >> 8)) as u8,
        (V_ACTIVE & 0xFF) as u8,
        (V_BLANK & 0xFF) as u8,
        (((V_ACTIVE >> 8) << 4) | (V_BLANK >> 8)) as u8,
        (H_SYNC_OFF & 0xFF) as u8,
        (H_SYNC_W & 0xFF) as u8,
        (((V_SYNC_OFF & 0xF) << 4) | (V_SYNC_W & 0xF)) as u8,
        (((H_SYNC_OFF >> 8) << 6)
            | ((H_SYNC_W >> 8) << 4)
            | ((V_SYNC_OFF >> 4) << 2)
            | (V_SYNC_W >> 4)) as u8,
        (H_MM & 0xFF) as u8,
        (V_MM & 0xFF) as u8,
        (((H_MM >> 8) << 4) | (V_MM >> 8)) as u8,
        0,    // no horizontal border
        0,    // no vertical border
        0x1E, // digital separate sync, both polarities positive, non-interlaced
    ]
}

/// Is this connector one we forced? Two independent records, because either can be missing.
///
/// The EDID is the one that survives a service restart and a process that never knew about the
/// force. The name is the one that survives a kernel that forced the connector on without ever
/// loading our override - in which case the connector reads `connected` carrying nothing of ours,
/// and calling that a display the operator plugged in would leave the feature inert while still
/// holding a connector it can no longer release.
///
/// The name fallback only counts when the name is unique in the topology: with a DP-1 on two cards,
/// claiming both would hide a real display behind our own and the release would never fire.
fn connector_is_ours(
    connected: bool,
    edid: &[u8],
    name: &str,
    device: &str,
    named: &[(String, String)],
    name_unique: bool,
) -> bool {
    connected
        && (edid_bytes_are_ours(edid)
            || (name_unique
                && named
                    .iter()
                    .any(|(n, d)| n == name && (d.is_empty() || d == device))))
}

#[allow(dead_code)]
fn edid_is_ours(edid: &Path) -> bool {
    std::fs::read(edid)
        .map(|bytes| edid_bytes_are_ours(&bytes))
        .unwrap_or(false)
}

/// Vendor id, product code AND the monitor-name descriptor. Any one of those alone is something a
/// real monitor's firmware could plausibly carry - product code `0x0001` especially - and this
/// predicate is what decides whether a connector may be released, so it should not be a coin flip.
fn edid_bytes_are_ours(bytes: &[u8]) -> bool {
    let d = EDID_NAME_DESCRIPTOR;
    bytes.len() >= d + 18
        && bytes[8] == (EDID_MFG >> 8) as u8
        && bytes[9] == (EDID_MFG & 0xFF) as u8
        && bytes[10] == (EDID_PRODUCT & 0xFF) as u8
        && bytes[11] == (EDID_PRODUCT >> 8) as u8
        && bytes[d + 3] == 0xFC
        && bytes[d + 5..].starts_with(EDID_MONITOR_NAME)
}

/// One firmware file per device, so the entry itself says which device the hold is on. The bare
/// name is what an earlier build wrote; it is still ours, it just carries no device.
fn edid_file_name(device: &str) -> String {
    if device.is_empty() {
        format!("{EDID_NAME_PREFIX}.bin")
    } else {
        format!("{EDID_NAME_PREFIX}-{device}.bin")
    }
}

fn edid_firmware_ref(device: &str) -> String {
    format!("{EDID_DIR_REF}{}", edid_file_name(device))
}

fn edid_path(device: &str) -> PathBuf {
    Path::new(EDID_DIR).join(edid_file_name(device))
}

/// `edid_firmware` is a comma-separated list of `[<connector>:]<file>` entries, so the value has to
/// be parsed rather than compared: a bare `ends_with` on our own file name reads a list that merely
/// *ends* with our entry as entirely ours, and would then overwrite everything before it.
fn edid_entries(v: &str) -> Vec<&str> {
    v.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect()
}

fn entry_file(entry: &str) -> &str {
    entry.rsplit(':').next().unwrap_or(entry)
}

/// The connector an entry targets, or `None` for one that applies to every connector.
fn entry_connector(entry: &str) -> Option<&str> {
    entry.split_once(':').map(|(target, _)| target)
}

/// The device an entry records the hold against, or `None` when the entry is not ours at all.
/// `Some("")` is the unqualified form an earlier build wrote: ours, but not device-checkable.
fn entry_device(entry: &str) -> Option<&str> {
    let file = entry_file(entry).strip_prefix(EDID_DIR_REF)?;
    let rest = file.strip_prefix(EDID_NAME_PREFIX)?.strip_suffix(".bin")?;
    match rest.strip_prefix('-') {
        Some(device) if !device.is_empty() => Some(device),
        _ if rest.is_empty() => Some(""),
        _ => None,
    }
}

fn entry_is_ours(entry: &str) -> bool {
    entry_device(entry).is_some()
}

/// Does an entry we did not write already govern this connector? An entry with no `<connector>:`
/// prefix applies to every connector, so it governs any of them.
fn foreign_entry_covers(entry: &str, connector: &str) -> bool {
    !entry_is_ours(entry)
        && match entry.split_once(':') {
            Some((target, _)) => target == connector,
            None => true,
        }
}

fn read_edid_param() -> String {
    read_trim(Path::new(EDID_PARAM)).unwrap_or_default()
}

/// Writing the parameter needs care in both directions. A zero-length write to this sysfs attribute
/// is a silent no-op that leaves the old value in place - measured, and it is why this always writes
/// at least a newline and reads the value back instead of trusting the write.
fn write_edid_param(value: &str) -> ResultType<()> {
    let payload = if value.is_empty() {
        "\n".to_owned()
    } else {
        format!("{value}\n")
    };
    std::fs::write(EDID_PARAM, payload)?;
    let now = read_edid_param();
    if now != value {
        bail!("edid_firmware still reads '{now}' after being set to '{value}'");
    }
    Ok(())
}

/// Add our entry for one connector, keeping every entry somebody else put there. Refuses if a
/// foreign entry already governs this connector: overriding it would change what that connector
/// reports, which is not ours to do.
fn install_edid(connector: &str, device: &str) -> ResultType<()> {
    let existing = read_edid_param();
    let entries = edid_entries(&existing);
    if let Some(theirs) = entries.iter().find(|e| foreign_entry_covers(e, connector)) {
        bail!("edid_firmware entry '{theirs}' already covers {connector}, leaving it alone");
    }
    std::fs::create_dir_all(EDID_DIR)?;
    std::fs::write(edid_path(device), synthetic_edid())?;
    let mine = format!("{connector}:{}", edid_firmware_ref(device));
    // Only our entry for THIS connector is replaced. A release that fails on several connectors
    // has to be able to record every one of them, and dropping the rest here would lose all but
    // the last.
    let mut wanted: Vec<String> = entries
        .iter()
        .filter(|e| !(entry_is_ours(e) && entry_connector(e) == Some(connector)))
        .map(|e| (*e).to_owned())
        .collect();
    wanted.push(mine);
    write_edid_param(&wanted.join(","))
}

/// Drop our entries, keeping everyone else's, so the next probe of those connectors reads whatever
/// is really on the wire. The firmware file goes only once the parameter no longer points at it: a
/// pointer to a missing file would make the kernel fail the load on every probe.
fn drop_our_entries(mut which: impl FnMut(&str) -> bool) {
    let existing = read_edid_param();
    let entries = edid_entries(&existing);
    let (mine, keep): (Vec<&str>, Vec<&str>) = entries
        .into_iter()
        .partition(|e| entry_is_ours(e) && which(e));
    if mine.is_empty() {
        return;
    }
    if let Err(e) = write_edid_param(&keep.join(",")) {
        log::warn!("headless display: cannot clear our edid_firmware entry: {e}");
        return;
    }
    for e in &mine {
        let Some(device) = entry_device(e) else {
            continue;
        };
        // Another entry of ours can still point at the same file.
        if keep.iter().any(|k| entry_device(k) == Some(device)) {
            continue;
        }
        let _ = std::fs::remove_file(edid_path(device));
    }
}

fn uninstall_edid() {
    drop_our_entries(|_| true)
}

/// The connector names our own `edid_firmware` entries refer to.
///
/// This is the only record left of a force whose EDID never actually loaded: the connector reads
/// `connected` either way, so nothing marks it as ours, and `State` is empty after a service
/// restart. Reading it back before the entries are removed is what keeps such a force releasable.
fn connectors_named_by_our_entries() -> Vec<(String, String)> {
    edid_entries(&read_edid_param())
        .into_iter()
        .filter_map(|e| {
            let device = entry_device(e)?;
            Some((entry_connector(e)?.to_owned(), device.to_owned()))
        })
        .collect()
}

fn write_status(sysfs: &str, value: &str) -> ResultType<()> {
    let p = Path::new(DRM_CLASS).join(sysfs).join("status");
    std::fs::write(&p, value).map_err(|e| {
        anyhow!(
            "cannot write {} ({e}); forcing a connector needs the root service",
            p.display()
        )
    })?;
    Ok(())
}

/// Poll one connector until its status matches, so a caller does not hand a topology that is still
/// being probed to whatever asks next. Returns whether it got there.
fn wait_for(sysfs: &str, connected: bool) -> bool {
    let p = Path::new(DRM_CLASS).join(sysfs).join("status");
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        if (read_trim(&p).as_deref() == Some("connected")) == connected {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(SETTLE_STEP);
    }
}

/// Which connector to force. Only a disconnected one, only on a card something can render to, and
/// only a kind of output a headless machine plausibly has a cable for - `Writeback` is not an output
/// at all, and an internal panel has nothing behind it.
///
/// A name duplicated across cards is REFUSED outright, not deprioritized: `edid_firmware` matches
/// the bare name, so forcing one twin puts our EDID on the other card's connector at its next
/// probe, and a monitor plugged there would classify as ours and never trigger the release.
fn pick_connector(all: &[Connector]) -> Option<&Connector> {
    const PREFERENCE: &[&str] = &["HDMI", "DP-", "DVI", "VGA"];
    let mut per_name: HashMap<&str, usize> = HashMap::new();
    for c in all {
        *per_name.entry(c.name.as_str()).or_default() += 1;
    }
    let usable = |c: &&Connector| {
        !c.connected
            && c.drivable
            && per_name.get(c.name.as_str()).copied().unwrap_or(1) == 1
            && PREFERENCE.iter().any(|k| c.name.starts_with(k))
    };
    for kind in PREFERENCE {
        let mut of_kind: Vec<&Connector> = all
            .iter()
            .filter(usable)
            .filter(|c| c.name.starts_with(kind))
            .collect();
        of_kind.sort_by_key(|c| &c.sysfs);
        if let Some(c) = of_kind.first() {
            return Some(c);
        }
    }
    None
}

/// Everything this watcher remembers between polls. A local rather than a set of statics, because
/// exactly one thread runs the loop: a global would need poison handling for state no other thread
/// can see.
#[derive(Default)]
struct State {
    /// The connectors this process forced, so a release does not depend only on the kernel still
    /// reporting our EDID. If a driver drops the override, the marker goes with it and nothing else
    /// would remember which connector to give back. A list, because a release that fails on more
    /// than one must not remember only the first.
    forced: Vec<String>,
    /// When the machine first reported no output, which is what the stable period is measured from.
    no_output_since: Option<Instant>,
    /// When forcing last failed, so a host where it cannot work is not retried every poll.
    last_failure: Option<Instant>,
    /// When a real display was first seen while we were holding a connector. The release waits for
    /// it, see `REAL_OUTPUT_STABLE`.
    real_since: Option<Instant>,
}

/// Install our EDID on one connector and force it on. No policy: the caller has already decided.
///
/// The EDID goes first. Forcing the connector on is what triggers the probe that reads it, so doing
/// it the other way round would need an unforce/force cycle - and that cycle publishes a topology
/// with no displays at all. Verified on a headless box: `edid_firmware` then `on`, with no `detect`
/// in between, brings the connector up carrying our EDID.
///
/// `status == connected` is as far as this can go. Binding a CRTC to the connector is the
/// compositor's decision, and there is no sysfs attribute to wait on for it, so the log says what
/// was forced and not that anything is being scanned out.
fn force_on(state: &mut State, name: &str, sysfs: &str, device: &str) -> ResultType<()> {
    install_edid(name, device)?;
    if let Err(e) = write_status(sysfs, "on") {
        // Roll back, or the override outlives the attempt: nothing would be forced, so nothing would
        // look like ours, and a monitor later plugged into that connector would be described by an
        // EDID we left behind with no owner.
        uninstall_edid();
        return Err(e);
    }
    state.forced = vec![sysfs.to_owned()];
    let settled = wait_for(sysfs, true);
    log::info!(
        "headless display: forced {sysfs} on with a 1920x1080 edid{}",
        if settled {
            ""
        } else {
            ", but it has not come up yet"
        }
    );
    // A forced connector reads `connected` whether or not the override loaded, so success above says
    // nothing about the EDID. Saying so matters: without our EDID the connector still works as a
    // scanout but falls back to the kernel's 1024x768 mode list, and nothing marks it as ours.
    if settled && !edid_is_ours(&Path::new(DRM_CLASS).join(sysfs).join("edid")) {
        log::warn!(
            "headless display: {sysfs} is on but is not reporting our edid, so the kernel did not \
             load {} (check the firmware search path). The display works with the kernel default \
             modes; releasing it relies on the edid_firmware entry instead.",
            edid_firmware_ref(device)
        );
    }
    Ok(())
}

/// Force a connector on so this machine has a scanout. Returns the connector forced.
fn enable(state: &mut State) -> ResultType<String> {
    let all = connectors();
    if all.is_empty() {
        bail!("no DRM connectors on this machine");
    }
    if let Some(c) = all.iter().find(|c| c.ours) {
        state.forced = vec![c.sysfs.clone()];
        return Ok(c.sysfs.clone());
    }
    if all.iter().any(is_real_output) {
        bail!("a display is already attached, nothing to force");
    }
    let target = pick_connector(&all).ok_or_else(|| anyhow!("no connector to force"))?;
    let (name, sysfs, device) = (
        target.name.clone(),
        target.sysfs.clone(),
        target.device.clone(),
    );
    force_on(state, &name, &sysfs, &device)?;
    Ok(sysfs)
}

/// Release what we forced, and only that: connectors still carrying our EDID, plus the one this
/// process forced even if its EDID has since been dropped. A connector forced by someone else does
/// not appear in either set.
fn disable(state: &mut State) -> ResultType<()> {
    let all = connectors();
    let mut targets: Vec<Target> = Vec::new();
    for c in all.iter().filter(|c| c.ours) {
        add_target(&mut targets, &all, &c.sysfs);
    }
    for sysfs in std::mem::take(&mut state.forced) {
        add_target(&mut targets, &all, &sysfs);
    }
    // Read the parameter while it still exists. A force whose EDID never loaded leaves no other
    // trace, and the entries below are about to go - which would leave the connector forced for the
    // rest of the boot with nothing able to name it. The name resolves against the live list, and
    // connector indices come from a per-type ida that is global to drm.ko, so two live connectors
    // never share a name; the device qualifier is what keeps a name the kernel re-issued after a
    // rebind from resolving to somebody else's connector.
    for (name, device) in connectors_named_by_our_entries() {
        for c in all
            .iter()
            .filter(|c| c.name == name && (device.is_empty() || c.device == device))
        {
            add_target(&mut targets, &all, &c.sysfs);
        }
    }
    let mut last_err = None;
    let mut released: Vec<String> = Vec::new();
    let mut held: Vec<String> = Vec::new();
    for t in &targets {
        // The override comes off before the unforce, or the re-probe would read our EDID straight
        // back and a real monitor on that connector would keep being described by it. One entry at
        // a time: every connector gets its write attempted, and a failure must not strand the
        // others with their override already gone.
        drop_our_entries(|e| entry_connector(e) == Some(t.name.as_str()));
        if let Err(e) = write_status(&t.sysfs, "detect") {
            log::warn!("headless display: cannot release {}: {e}", t.sysfs);
            // Put the marker back. The parameter entry is what survives this process and, for a
            // force whose override never loaded, it is the only record there is. If this write
            // fails too the hold lives on in `state.forced` alone and a restart loses it, which is
            // as far as a machine refusing both writes can be carried.
            if let Err(e) = install_edid(&t.name, &t.device) {
                log::warn!(
                    "headless display: cannot re-record the hold on {}: {e}",
                    t.sysfs
                );
            }
            state.forced.push(t.sysfs.clone());
            held.push(t.name.clone());
            last_err = Some(e);
        } else {
            released.push(t.sysfs.clone());
        }
    }
    // Whatever is left of ours names no connector this pass could act on - an override orphaned by
    // an earlier failure, or one whose card is gone - and it would keep painting our EDID on the
    // next probe of that name.
    drop_our_entries(|e| !entry_connector(e).is_some_and(|c| held.iter().any(|h| h == c)));
    if !released.is_empty() {
        log::info!("headless display: released {}", released.join(", "));
    }
    match last_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// One connector a release has to act on, carried with the facts needed to put its marker back.
struct Target {
    sysfs: String,
    name: String,
    device: String,
}

fn add_target(targets: &mut Vec<Target>, all: &[Connector], sysfs: &str) {
    if targets.iter().any(|t| t.sysfs == sysfs) {
        return;
    }
    match all.iter().find(|c| c.sysfs == sysfs) {
        Some(c) => targets.push(Target {
            sysfs: c.sysfs.clone(),
            name: c.name.clone(),
            device: c.device.clone(),
        }),
        // Gone from sysfs since the record was made. Still worth the write, and the sysfs directory
        // name carries the connector name; the device is no longer knowable.
        None => targets.push(Target {
            sysfs: sysfs.to_owned(),
            name: sysfs
                .split_once('-')
                .map(|(_, name)| name.to_owned())
                .unwrap_or_default(),
            device: String::new(),
        }),
    }
}

/// Is there anything of ours left to give back, including an override orphaned by a failed attempt?
///
/// The connector walk is the last of the three checks because the first two answer instantly when we
/// are holding something, but with the feature off none of them short-circuits, so a disabled host
/// does pay the walk every poll: a readdir plus a `status` and an `edid` read per connector. That is
/// deliberate - the walk is the only thing that finds a connector left forced by an earlier run whose
/// parameter entry is already gone, and it costs a few page-cached kernfs reads against a service
/// loop that reads seat0 twice a second anyway.
fn holding_something(state: &State) -> bool {
    !state.forced.is_empty()
        || edid_entries(&read_edid_param())
            .iter()
            .any(|e| entry_is_ours(e))
        || connectors().iter().any(|c| c.ours)
}

/// Start the watcher. Called once from the root service, which is the process that holds the
/// privilege to write sysfs.
///
/// Its own thread rather than the service loop, because forcing a connector waits for it to come up
/// and that must not delay `--server` supervision. Built with `Builder` for the same reason the DRM
/// producer is: `thread::spawn` panics if the thread cannot be created, and that panic would unwind
/// out of the service startup and take the whole root service with it, for a feature whose failure
/// should cost nothing but this feature.
pub fn start_watcher() {
    let spawned = std::thread::Builder::new()
        .name("headless-display".into())
        .spawn(|| {
            let mut state = State::default();
            loop {
                // A panic inside one pass must not kill the watcher for the rest of the boot: that
                // would leave whatever is forced forced, with nothing left running to give it back.
                // `AssertUnwindSafe` because the state is ours alone and a half-updated timestamp
                // costs at most one extra poll.
                let pass = std::panic::AssertUnwindSafe(|| tick(&mut state));
                if let Err(e) = std::panic::catch_unwind(pass) {
                    log::error!("headless display: a poll panicked ({e:?}); continuing");
                }
                std::thread::sleep(POLL_INTERVAL);
            }
        });
    if let Err(e) = spawned {
        log::warn!("headless display: cannot start the watcher thread: {e}");
    }
}

/// One pass. Silent unless something changes; see `holding_something` for what it costs when the
/// feature is off.
///
/// The option is read from `Config` rather than from disk on purpose. A UI toggle writes the user's
/// config and the user `--server` syncs it to this process over `ipc_service` in well under a second,
/// and `--headless-display` pushes it here itself, so the in-memory value is the one that reflects
/// what the operator just asked for.
/// Overlay the process's own record of what it forced onto the sysfs view: while the connector
/// settles - or when the override never loaded on it - sysfs cannot name it ours, and without
/// this the tick would re-force it and count it as a real output. Matched by the card-qualified
/// sysfs name, never the bare one. Only a CONNECTED connector is adopted: a forced record whose
/// connector reads disconnected means the kernel-side force was lost (GPU reset, an operator's
/// detect), and adopting it would park the tick in the release-watch branch instead of letting
/// the arming path re-force.
fn adopt_forced(all: &mut [Connector], forced: &[String]) {
    for c in all.iter_mut() {
        if c.connected && forced.iter().any(|f| *f == c.sysfs) {
            c.ours = true;
        }
    }
}

/// Drop a marker whose device is not the one that connector sits on now, and re-probe what it named.
///
/// Connector names are unique among LIVE connectors, but `drm_connector_cleanup()` frees the index,
/// so an unbind and rebind can hand `DP-3` to a different card. `edid_firmware` has no card syntax,
/// so the entry would follow the name onto that stranger, paint our EDID on it and make it look
/// like ours - which hides a real display behind our own. The `detect` write is what stops it
/// reporting our EDID; without it the connector keeps the blob it already probed.
///
/// Returns whether anything was dropped, so the caller can re-read a topology this just changed.
fn prune_foreign_markers(all: &[Connector]) -> bool {
    let stale: Vec<(String, String)> = connectors_named_by_our_entries()
        .into_iter()
        .filter(|(name, device)| {
            !device.is_empty() && !all.iter().any(|c| &c.name == name && &c.device == device)
        })
        .collect();
    if stale.is_empty() {
        return false;
    }
    for (name, device) in &stale {
        log::info!("headless display: dropping the hold on {name}, no longer on {device}");
    }
    drop_our_entries(|e| {
        entry_connector(e).is_some_and(|c| stale.iter().any(|(name, _)| name == c))
    });
    for c in all
        .iter()
        .filter(|c| stale.iter().any(|(name, _)| *name == c.name))
    {
        if let Err(e) = write_status(&c.sysfs, "detect") {
            log::warn!("headless display: cannot re-probe {}: {e}", c.sysfs);
        }
    }
    true
}

fn tick(state: &mut State) {
    if !is_enabled() {
        // Turned off, or never on. Give back anything we are still holding, then stay out of sysfs.
        if holding_something(state) {
            let _ = disable(state);
        }
        state.no_output_since = None;
        state.last_failure = None;
        // A release timer started before the toggle must not survive it, or a later re-enable
        // skips the stability delay.
        state.real_since = None;
        return;
    }

    let mut all = connectors();
    if prune_foreign_markers(&all) {
        all = connectors();
    }
    adopt_forced(&mut all, &state.forced);
    if all.iter().any(|c| c.ours) {
        // A real display takes precedence over ours: release it, and the machine goes back to the
        // output the operator actually plugged in. Only visible for a connector other than the one we
        // are holding, because ours is what that connector reports.
        if !all.iter().any(is_real_output) {
            state.real_since = None;
            return;
        }
        if state.real_since.get_or_insert_with(Instant::now).elapsed() < REAL_OUTPUT_STABLE {
            return;
        }
        state.real_since = None;
        log::info!("headless display: a real display was attached");
        let _ = disable(state);
        return;
    }

    // Reaching here with our firmware entry still present means ownership could not be attributed
    // to ANY connector: not by EDID, not by a unique name, not by this process's record (a restart
    // cleared it, and a rebind can have twinned the name since). An unattributable hold cannot be
    // managed - left alone it counts as a real output and wedges both arming and release - so give
    // it back; the next tick re-forces cleanly if the machine is truly headless.
    if state.forced.is_empty() && !connectors_named_by_our_entries().is_empty() {
        log::info!("headless display: releasing a hold this process cannot attribute");
        let _ = disable(state);
        return;
    }

    match state.last_failure {
        Some(t) if t.elapsed() < RETRY_AFTER_FAILURE => return,
        Some(_) => state.last_failure = None,
        None => {}
    }

    if !no_real_output(&all) {
        state.no_output_since = None;
        return;
    }
    if state
        .no_output_since
        .get_or_insert_with(Instant::now)
        .elapsed()
        < NO_OUTPUT_STABLE
    {
        return;
    }
    state.no_output_since = None;

    match enable(state) {
        Ok(c) => log::info!("headless display: {c} is now the scanout for this machine"),
        Err(e) => {
            // Backed off rather than retried every poll: a topology with no forceable connector does
            // not become forceable a second later, and the warning would repeat for the whole boot.
            state.last_failure = Some(Instant::now());
            log::warn!(
                "headless display: not forcing a connector: {e}; retrying in {} s",
                RETRY_AFTER_FAILURE.as_secs()
            );
        }
    }
}

/// What `--headless-display status` prints. `enabled` is passed in rather than read here: the CLI
/// asks the running server over IPC, the way `--option` does, so a normal user does not get told
/// about their own config while the root service acts on a different one.
pub fn status_text(enabled: bool) -> String {
    let all = connectors();
    let mut s = format!(
        "headless display: {}\n",
        if enabled { "enabled" } else { "disabled" }
    );
    let named = connectors_named_by_our_entries();
    match all.iter().find(|c| c.ours) {
        Some(c) => s.push_str(&format!("forced by rustdesk: {}\n", c.sysfs)),
        // A connector our own entry names but whose EDID does not read back is a force whose
        // override never loaded. Printing it is the difference between a diagnosable state and a
        // machine that looks untouched while holding a connector on.
        None if !named.is_empty() => s.push_str(&format!(
            "forced by rustdesk: {} (our edid did not load; released by name)\n",
            named
                .iter()
                .map(|(name, device)| format!("{name} on {device}"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        None => s.push_str("forced by rustdesk: none\n"),
    }
    let param = read_edid_param();
    if !param.is_empty() {
        s.push_str(&format!("edid_firmware: {param}\n"));
    }
    if all.is_empty() {
        s.push_str("no DRM connectors on this machine\n");
        return s;
    }
    s.push_str("connectors:\n");
    for c in &all {
        s.push_str(&format!(
            "  {:<20} {}{}{}\n",
            c.sysfs,
            if c.connected {
                "connected"
            } else {
                "disconnected"
            },
            if c.ours { " (rustdesk)" } else { "" },
            if c.drivable { "" } else { " (no render node)" },
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_synthetic_edid_is_structurally_valid() {
        let e = synthetic_edid();
        assert_eq!(e.len(), 128);
        assert_eq!(&e[0..8], &[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        assert_eq!(e[126], 0, "one block, so no extensions");
        // The whole block must sum to zero mod 256; it is the one check every parser does.
        assert_eq!(e.iter().fold(0u8, |a, b| a.wrapping_add(*b)), 0);
        // Serial number stays unspecified rather than invented.
        assert_eq!(&e[12..16], &[0, 0, 0, 0]);
    }

    #[test]
    fn the_edid_carries_the_1080p_timing_and_a_mode_list() {
        let e = synthetic_edid();
        let clock = u16::from_le_bytes([e[54], e[55]]) as u32 * 10_000;
        assert_eq!(clock, 148_500_000);
        let hactive = ((e[58] as u16 & 0xF0) << 4) | e[56] as u16;
        let vactive = ((e[61] as u16 & 0xF0) << 4) | e[59] as u16;
        assert_eq!((hactive, vactive), (1920, 1080));
        let modes = (0..8)
            .filter(|i| e[38 + i * 2] != 0x01)
            .map(|i| (e[38 + i * 2] as u16 + 31) * 8)
            .collect::<Vec<_>>();
        assert!(modes.contains(&1280), "{modes:?}");
        assert!(modes.contains(&1024), "{modes:?}");
    }

    #[test]
    fn our_own_edid_is_recognised_and_a_near_miss_is_not() {
        let e = synthetic_edid();
        assert!(edid_bytes_are_ours(&e));

        // Same block with a different vendor id. 0x10AC is DEL, which is what the real monitor EDID
        // on the headless test box carries, so this is the exact confusion to rule out.
        let mut dell = e.clone();
        dell[8] = 0x10;
        dell[9] = 0xAC;
        assert!(!edid_bytes_are_ours(&dell));

        // Our vendor and product but somebody else's name: product code 0x0001 is common enough that
        // the name has to count too.
        let mut renamed = e.clone();
        renamed[EDID_NAME_DESCRIPTOR + 5] = b'X';
        assert!(!edid_bytes_are_ours(&renamed));

        // An empty or truncated read is not ours either.
        assert!(!edid_bytes_are_ours(&[]));
        assert!(!edid_bytes_are_ours(&e[..64]));
    }

    #[test]
    fn a_foreign_edid_firmware_entry_survives_and_ours_is_told_apart() {
        // A list is a list: a value that merely *ends* with our file is not entirely ours.
        let mixed = "DP-1:edid/theirs.bin,HDMI-A-1:edid/rustdesk-headless.bin";
        let entries = edid_entries(mixed);
        assert_eq!(entries.len(), 2);
        assert!(!entry_is_ours(entries[0]));
        assert!(entry_is_ours(entries[1]));

        // And the other order, which a suffix test would call foreign and then never clean up.
        let other_order = "HDMI-A-1:edid/rustdesk-headless.bin,DP-1:edid/theirs.bin";
        assert!(edid_entries(other_order).iter().any(|e| entry_is_ours(e)));

        assert!(edid_entries("").is_empty());
    }

    #[test]
    fn our_entries_name_the_connectors_a_release_has_to_reach() {
        // The parse behind the only recovery path for a force whose EDID never loaded. It has to
        // survive a foreign entry, either ordering, and an entry with no connector prefix.
        let dev = "0000-01-00-0";
        let mine = |c: &str| format!("{c}:edid/rustdesk-headless-{dev}.bin");
        let names = |v: &str| {
            edid_entries(v)
                .into_iter()
                .filter_map(|e| Some((entry_connector(e)?.to_owned(), entry_device(e)?.to_owned())))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&format!("DP-1:edid/theirs.bin,{}", mine("HDMI-A-1"))),
            vec![("HDMI-A-1".to_owned(), dev.to_owned())]
        );
        assert_eq!(
            names(&format!("{},DP-1:edid/theirs.bin", mine("HDMI-A-1"))),
            vec![("HDMI-A-1".to_owned(), dev.to_owned())]
        );
        // Two of ours, which is what a machine that forced one connector and then another leaves.
        assert_eq!(
            names(&format!("{},{}", mine("HDMI-A-1"), mine("DP-2"))),
            vec![
                ("HDMI-A-1".to_owned(), dev.to_owned()),
                ("DP-2".to_owned(), dev.to_owned())
            ]
        );
        assert!(names("DP-1:edid/theirs.bin").is_empty());
        assert!(names("").is_empty());
        // What an earlier build wrote: still ours, so still releasable, with no device to check.
        assert_eq!(
            names("HDMI-A-1:edid/rustdesk-headless.bin"),
            vec![("HDMI-A-1".to_owned(), String::new())]
        );
        // A file that merely starts like ours is not ours.
        assert!(names("HDMI-A-1:edid/rustdesk-headless-thing.png").is_empty());
    }

    // rustdesk#15908: the kernel frees a connector's name when the connector goes, so a rebind can
    // hand `HDMI-A-1` to a different card - and `edid_firmware` has no card syntax to stop our
    // entry following it there. The device the entry records is what refuses the stranger.
    #[test]
    fn a_marker_does_not_follow_its_name_onto_another_device() {
        let named = vec![("HDMI-A-1".to_owned(), "0000-01-00-0".to_owned())];
        assert!(connector_is_ours(
            true,
            &[],
            "HDMI-A-1",
            "0000-01-00-0",
            &named,
            true
        ));
        // Same connector name, different card: not ours.
        assert!(!connector_is_ours(
            true,
            &[],
            "HDMI-A-1",
            "0000-02-00-0",
            &named,
            true
        ));
        // An unqualified entry from an earlier build still resolves by name alone, or a machine
        // upgraded mid-hold could never give the connector back.
        let legacy = vec![("HDMI-A-1".to_owned(), String::new())];
        assert!(connector_is_ours(
            true,
            &[],
            "HDMI-A-1",
            "0000-02-00-0",
            &legacy,
            true
        ));
    }

    #[test]
    fn a_force_whose_edid_never_loaded_is_still_ours() {
        let mine = synthetic_edid();
        let dev = "0000-01-00-0";
        let named = vec![("HDMI-A-1".to_owned(), dev.to_owned())];

        // The ordinary case: our EDID came back, and the name is not even needed.
        assert!(connector_is_ours(true, &mine, "HDMI-A-1", dev, &[], true));
        // The case this exists for: forced on, override never loaded, so the connector reports
        // nothing of ours. Only our own edid_firmware entry still names it.
        assert!(connector_is_ours(true, &[], "HDMI-A-1", dev, &named, true));
        // A real monitor on a connector we never named is never ours, either way round.
        assert!(!connector_is_ours(true, &[], "DP-2", dev, &named, true));
        assert!(!connector_is_ours(true, &[], "DP-2", dev, &[], true));
        // And a disconnected connector is nobody's, whatever the parameter says.
        assert!(!connector_is_ours(false, &mine, "HDMI-A-1", dev, &named, true));
        // A duplicated name claims nothing by name alone: with DP-1 on two cards, the fallback
        // would mark both ours and a real display behind one of them would never release ours.
        assert!(!connector_is_ours(true, &[], "HDMI-A-1", dev, &named, false));
        // The EDID still decides when it does read back, duplicated name or not.
        assert!(connector_is_ours(true, &mine, "HDMI-A-1", dev, &named, false));
    }

    #[test]
    fn a_foreign_entry_that_covers_our_connector_blocks_us() {
        assert!(foreign_entry_covers("HDMI-A-1:edid/theirs.bin", "HDMI-A-1"));
        assert!(!foreign_entry_covers("DP-1:edid/theirs.bin", "HDMI-A-1"));
        // No connector prefix means every connector, so it covers whatever we would pick.
        assert!(foreign_entry_covers("edid/theirs.bin", "HDMI-A-1"));
        // Our own entry never blocks us.
        assert!(!foreign_entry_covers(
            "HDMI-A-1:edid/rustdesk-headless-0000-01-00-0.bin",
            "HDMI-A-1"
        ));
        // And the unqualified form an earlier build wrote is still ours, not a stranger's.
        assert!(!foreign_entry_covers(
            "HDMI-A-1:edid/rustdesk-headless.bin",
            "HDMI-A-1"
        ));
    }

    fn c(sysfs: &str, name: &str, connected: bool, ours: bool) -> Connector {
        Connector {
            name: name.to_owned(),
            sysfs: sysfs.to_owned(),
            connected,
            ours,
            device: sysfs.split_once('-').map(|(card, _)| card).unwrap_or("").to_owned(),
            drivable: true,
        }
    }

    fn unrenderable(sysfs: &str, name: &str, connected: bool) -> Connector {
        Connector {
            drivable: false,
            ..c(sysfs, name, connected, false)
        }
    }

    #[test]
    fn what_counts_as_real_output() {
        assert!(no_real_output(&[c(
            "card0-HDMI-A-1",
            "HDMI-A-1",
            false,
            false
        )]));
        assert!(
            no_real_output(&[c("card0-HDMI-A-1", "HDMI-A-1", true, true)]),
            "our own display must not stop the tick from releasing it"
        );
        assert!(!no_real_output(&[c(
            "card0-HDMI-A-1",
            "HDMI-A-1",
            true,
            false
        )]));
        assert!(
            !no_real_output(&[]),
            "no connectors at all is not the headless case"
        );
        // The MacBook Touch Bar is permanently connected on a card with no render node. Counting it
        // as real output would stop the feature from ever arming there, and would make it release a
        // working forced display in favour of an output nothing can draw on.
        assert!(no_real_output(&[unrenderable(
            "card0-USB-1",
            "USB-1",
            true
        )]));
    }

    #[test]
    fn connector_choice() {
        let all = vec![
            c("card0-Writeback-1", "Writeback-1", false, false),
            c("card0-eDP-1", "eDP-1", false, false),
            c("card0-VGA-1", "VGA-1", false, false),
            c("card0-HDMI-A-2", "HDMI-A-2", false, false),
        ];
        assert_eq!(
            pick_connector(&all).map(|c| c.name.as_str()),
            Some("HDMI-A-2")
        );

        // Nothing forceable: an internal panel and a writeback are not outputs to offer.
        assert!(pick_connector(&[
            c("card0-Writeback-1", "Writeback-1", false, false),
            c("card0-eDP-1", "eDP-1", false, false),
        ])
        .is_none());
        // A connected connector is never forced.
        assert!(pick_connector(&[c("card0-HDMI-A-1", "HDMI-A-1", true, false)]).is_none());
        // Nor is a connector on a display-only card.
        assert!(pick_connector(&[unrenderable("card0-USB-1", "USB-1", false)]).is_none());
        // An unknown connector kind is left alone rather than forced as a last resort.
        assert!(pick_connector(&[c("card0-DSI-1", "DSI-1", false, false)]).is_none());
    }

    /// The real topology of a 2018 MacBook Pro, read off the machine: three DRM devices, `card0`
    /// being the Touch Bar with no render node and a connector that is permanently `connected`.
    /// It is here because that permanently-connected non-renderable output is what an earlier
    /// revision counted as real output, which stopped the feature from ever arming on this hardware.
    fn macbook_pro_2018() -> Vec<Connector> {
        vec![
            unrenderable("card0-USB-1", "USB-1", true),
            c("card1-DP-1", "DP-1", false, false),
            c("card1-DP-2", "DP-2", false, false),
            c("card1-DP-3", "DP-3", false, false),
            c("card1-HDMI-A-1", "HDMI-A-1", false, false),
            c("card1-HDMI-A-2", "HDMI-A-2", false, false),
            c("card1-HDMI-A-3", "HDMI-A-3", false, false),
            c("card2-DP-4", "DP-4", false, false),
            c("card2-DP-5", "DP-5", false, false),
            c("card2-DP-6", "DP-6", true, false),
            c("card2-DP-7", "DP-7", false, false),
            c("card2-eDP-1", "eDP-1", true, false),
        ]
    }

    #[test]
    fn a_machine_with_its_own_displays_is_left_alone() {
        // DP-6 and the internal panel are both connected on a renderable card, so this machine has
        // output and the feature must do nothing at all here.
        assert!(!no_real_output(&macbook_pro_2018()));
    }

    #[test]
    fn the_touch_bar_alone_does_not_count_as_output() {
        // Same machine with every real display gone: only the non-renderable Touch Bar is left, and
        // the feature must arm and pick an HDMI on a card something can render to.
        let mut all = macbook_pro_2018();
        for x in all.iter_mut() {
            if x.drivable {
                x.connected = false;
            }
        }
        assert!(no_real_output(&all));
        assert_eq!(
            pick_connector(&all).map(|c| c.sysfs.as_str()),
            Some("card1-HDMI-A-1")
        );
    }

    /// The measured topology of a Raspberry Pi 5 (Ubuntu 25.10, kernel 6.17): `card0` is the v3d
    /// GPU with a render node and no connectors, `card1` is vc4 with every connector and no render
    /// node. Nothing here owns both, which is what `promote_split_topology` exists for.
    fn raspberry_pi_5(monitor_attached: bool) -> Vec<Connector> {
        vec![
            unrenderable("card1-HDMI-A-1", "HDMI-A-1", false),
            unrenderable("card1-HDMI-A-2", "HDMI-A-2", monitor_attached),
            unrenderable("card1-Writeback-1", "Writeback-1", false),
            unrenderable("card1-Writeback-2", "Writeback-2", false),
        ]
    }

    #[test]
    fn a_split_soc_counts_its_display_card() {
        // With a monitor attached the machine has real output and must be left alone.
        let mut all = raspberry_pi_5(true);
        promote_split_topology(&mut all, true);
        assert!(!no_real_output(&all));
        // Headless it arms, and the forceable connector is on the display-only card.
        let mut all = raspberry_pi_5(false);
        promote_split_topology(&mut all, true);
        assert!(no_real_output(&all));
        assert_eq!(
            pick_connector(&all).map(|c| c.name.as_str()),
            Some("HDMI-A-1")
        );
    }

    #[test]
    fn a_machine_with_no_render_node_at_all_is_not_promoted() {
        let mut all = raspberry_pi_5(false);
        promote_split_topology(&mut all, false);
        assert!(pick_connector(&all).is_none());
    }

    #[test]
    fn promotion_leaves_a_machine_with_hybrid_cards_alone() {
        // The Touch Bar case: real GPUs own connectors here, so nothing gets promoted even in the
        // arming state where every display is gone.
        let mut all = macbook_pro_2018();
        for x in all.iter_mut() {
            if x.drivable {
                x.connected = false;
            }
        }
        promote_split_topology(&mut all, true);
        assert!(
            all.iter().any(|c| !c.drivable),
            "the Touch Bar must stay display-only"
        );
    }

    #[test]
    fn a_name_that_exists_on_two_cards_is_refused_outright() {
        // `edid_firmware` matches a bare connector name with no card qualifier: forcing one twin
        // puts our EDID on the other card's connector at its next probe, and a monitor plugged
        // there would classify as ours and never trigger the release. So a duplicated name is not
        // deprioritized, it is unusable - even when the alternative is forcing nothing.
        let all = vec![
            c("card1-DP-1", "DP-1", false, false),
            c("card1-DP-9", "DP-9", false, false),
            c("card2-DP-1", "DP-1", false, false),
        ];
        assert_eq!(
            pick_connector(&all).map(|c| c.sysfs.as_str()),
            Some("card1-DP-9")
        );
        let only_dupes = vec![
            c("card1-DP-1", "DP-1", false, false),
            c("card2-DP-1", "DP-1", false, false),
        ];
        assert!(pick_connector(&only_dupes).is_none());
    }

    #[test]
    fn a_connected_writeback_neither_blocks_arming_nor_releases_a_hold() {
        // vc4 writebacks can read `connected`; a capture sink is not a display anyone watches.
        let wb = c("card1-Writeback-1", "Writeback-1", true, false);
        assert!(no_real_output(&[wb]));
    }

    #[test]
    fn the_forced_record_outranks_sysfs() {
        // While the forced connector settles (or when its override never loaded), sysfs cannot
        // call it ours; the process's own record must, or the tick re-forces and miscounts it.
        let mut all = vec![
            c("card1-HDMI-A-1", "HDMI-A-1", true, false),
            c("card1-HDMI-A-2", "HDMI-A-2", false, false),
        ];
        adopt_forced(&mut all, &["card1-HDMI-A-1".to_owned()]);
        assert!(all[0].ours, "the held connector is adopted by sysfs name");
        assert!(!all[1].ours);
        // And the bare name must not match: the record is card-qualified.
        let mut all2 = vec![c("card1-HDMI-A-1", "HDMI-A-1", true, false)];
        adopt_forced(&mut all2, &["HDMI-A-1".to_owned()]);
        assert!(!all2[0].ours);
        adopt_forced(&mut all2, &[]);
        assert!(!all2[0].ours);
        // A disconnected forced record means the kernel-side force was lost: not adopted, so the
        // arming path can re-force instead of the tick parking in the release watch.
        let mut all3 = vec![c("card1-HDMI-A-1", "HDMI-A-1", false, false)];
        adopt_forced(&mut all3, &["card1-HDMI-A-1".to_owned()]);
        assert!(!all3[0].ours);
        // A release that failed on two connectors remembers both, or the second is never retried.
        let mut all4 = vec![
            c("card1-HDMI-A-1", "HDMI-A-1", true, false),
            c("card1-DP-2", "DP-2", true, false),
        ];
        adopt_forced(
            &mut all4,
            &["card1-HDMI-A-1".to_owned(), "card1-DP-2".to_owned()],
        );
        assert!(all4.iter().all(|c| c.ours));
    }
}
