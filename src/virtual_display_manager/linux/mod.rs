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

/// How often a held connector is re-probed to see whether a display has been plugged into it.
/// Bounds how long such a display stays invisible; see `real_display_probe_due`.
const HELD_CONNECTOR_PROBE_INTERVAL: Duration = Duration::from_secs(300);
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
    /// The card's stable bus identity, see `device_id`. `None` when sysfs does not say, and a
    /// connector with no identity is never claimed.
    device: Option<String>,
    /// The connector's KMS object id, see `connector_instance_id`. With `device` it names the
    /// connector INSTANCE; the pair is what a marker records.
    id: Option<String>,
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

/// A stable identity for the DRM device a card sits on, as a short token that can live in a file
/// name. The FULL symlink target is hashed rather than trimmed: two different bus paths can share a
/// basename, and a lossy transformation would let them collide into one identity.
///
/// `None` when sysfs does not say. A hold is then refused rather than recorded against an identity
/// that other cards could share.
///
/// `cardN` cannot play this role - the minor comes from an allocator that recycles, measured on a
/// MacBook whose Touch Bar connector was `card0-USB-1` one boot and `card3-USB-2` the next.
fn device_id(card: &str) -> Option<String> {
    let target = std::fs::read_link(Path::new(DRM_CLASS).join(card).join("device")).ok()?;
    Some(fnv1a(target.to_string_lossy().as_bytes()))
}

/// FNV-1a, written out because the marker has to be readable by a LATER build: `DefaultHasher` is
/// explicitly not stable across releases, so a hold recorded with it would stop being recognised
/// after an upgrade.
fn fnv1a(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// The connector's KMS object id - the number `drmModeGetConnector` takes, exposed as
/// `connector_id` in sysfs. This is what identifies the connector INSTANCE: the kernel returns a
/// connector's name to its type allocator when the connector goes, so a later connector on the same
/// card can carry the same name, and the device id alone would not tell them apart. Measured on an
/// Iris Xe: 508/518/528 for the three connectors, unchanged across a re-probe.
fn connector_instance_id(sysfs: &str) -> Option<String> {
    read_trim(&Path::new(DRM_CLASS).join(sysfs).join("connector_id")).filter(|s| !s.is_empty())
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
    let mut device_cache: HashMap<String, Option<String>> = HashMap::new();
    for (dir, sysfs, card, name) in raw {
        let connected = read_trim(&dir.join("status")).as_deref() == Some("connected");
        let drivable = *renderable_cache
            .entry(card.clone())
            .or_insert_with_key(|c| card_is_renderable(c));
        let device = device_cache
            .entry(card)
            .or_insert_with_key(|c| device_id(c))
            .clone();
        let id = connector_instance_id(&sysfs);
        out.push(Connector {
            ours: connector_is_ours(
                connected,
                &std::fs::read(dir.join("edid")).unwrap_or_default(),
                &name,
                device.as_deref(),
                id.as_deref(),
                &named,
                name_count.get(&name).copied().unwrap_or(1) == 1,
            ),
            name,
            sysfs,
            connected,
            device,
            id,
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
    device: Option<&str>,
    id: Option<&str>,
    named: &[(String, Marker)],
    name_unique: bool,
) -> bool {
    connected
        && (edid_bytes_are_ours(edid)
            || (name_unique
                && named
                    .iter()
                    .any(|(n, m)| n == name && m.claims(device, id))))
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

/// One firmware file per connector instance, so the entry itself says what the hold is on.
fn edid_firmware_ref(marker: &Marker) -> String {
    format!("{EDID_DIR_REF}{}", marker.file_name())
}

fn edid_path(marker: &Marker) -> PathBuf {
    Path::new(EDID_DIR).join(marker.file_name())
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

/// What one of our markers records about the connector it holds.
///
/// `Legacy` is the unqualified form an earlier build wrote. It is still ours, so a machine upgraded
/// mid-hold can still give the connector back, but it names no instance - so the first pass that
/// can attribute it rewrites it, and a pass that cannot drops it. It is never left standing as a
/// permanent wildcard.
#[derive(Clone, Debug, PartialEq)]
enum Marker {
    Legacy,
    Instance { device: String, id: String },
}

impl Marker {
    /// Does this marker claim the connector with this identity? A connector that cannot state its
    /// own identity is claimed by nothing.
    fn claims(&self, device: Option<&str>, id: Option<&str>) -> bool {
        match self {
            Marker::Legacy => true,
            Marker::Instance { device: d, id: i } => {
                device == Some(d.as_str()) && id == Some(i.as_str())
            }
        }
    }

    fn file_name(&self) -> String {
        match self {
            Marker::Legacy => format!("{EDID_NAME_PREFIX}.bin"),
            Marker::Instance { device, id } => format!("{EDID_NAME_PREFIX}-{device}-{id}.bin"),
        }
    }
}

/// The marker an entry carries, or `None` when the entry is not ours at all.
fn entry_marker(entry: &str) -> Option<Marker> {
    let file = entry_file(entry).strip_prefix(EDID_DIR_REF)?;
    let rest = file.strip_prefix(EDID_NAME_PREFIX)?.strip_suffix(".bin")?;
    if rest.is_empty() {
        return Some(Marker::Legacy);
    }
    // `<device>-<id>`: both halves are our own alphanumeric tokens, so the LAST dash splits them.
    let (device, id) = rest.strip_prefix('-')?.rsplit_once('-')?;
    if device.is_empty() || id.is_empty() {
        return None;
    }
    Some(Marker::Instance {
        device: device.to_owned(),
        id: id.to_owned(),
    })
}

fn entry_is_ours(entry: &str) -> bool {
    entry_marker(entry).is_some()
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
fn install_edid(connector: &str, marker: &Marker) -> ResultType<()> {
    let existing = read_edid_param();
    let entries = edid_entries(&existing);
    if let Some(theirs) = entries.iter().find(|e| foreign_entry_covers(e, connector)) {
        bail!("edid_firmware entry '{theirs}' already covers {connector}, leaving it alone");
    }
    std::fs::create_dir_all(EDID_DIR)?;
    std::fs::write(edid_path(marker), synthetic_edid())?;
    let mine = format!("{connector}:{}", edid_firmware_ref(marker));
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
        let Some(marker) = entry_marker(e) else {
            continue;
        };
        // Another entry of ours can still point at the same file.
        if keep.iter().any(|k| entry_marker(k).as_ref() == Some(&marker)) {
            continue;
        }
        let _ = std::fs::remove_file(edid_path(&marker));
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
fn connectors_named_by_our_entries() -> Vec<(String, Marker)> {
    edid_entries(&read_edid_param())
        .into_iter()
        .filter_map(|e| Some((entry_connector(e)?.to_owned(), entry_marker(e)?)))
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
    /// than one must not remember only the first, and each entry carries the full identity: the
    /// sysfs name alone is recycled, so a bare record would re-adopt a stranger.
    forced: Vec<Held>,
    /// When the machine first reported no output, which is what the stable period is measured from.
    no_output_since: Option<Instant>,
    /// When forcing last failed, so a host where it cannot work is not retried every poll.
    last_failure: Option<Instant>,
    /// When a real display was first seen while we were holding a connector. The release waits for
    /// it, see `REAL_OUTPUT_STABLE`.
    real_since: Option<Instant>,
    /// When the held connector was last re-probed for a display of its own, see
    /// `real_display_probe_due`.
    last_probe: Option<Instant>,
}

/// One connector this process holds, with everything needed to recognise it again. The sysfs
/// directory is what a write targets, the name is what `edid_firmware` matches, and the marker is
/// what says WHICH connector instance - both halves of it get recycled on their own.
#[derive(Clone, Debug, PartialEq)]
struct Held {
    sysfs: String,
    name: String,
    marker: Marker,
}

/// The identity of a live connector, or `None` when it cannot state one. A connector that cannot be
/// identified is never forced and never adopted: a marker recorded against a shared identity would
/// follow the name onto a stranger, which is the whole failure the marker exists to prevent.
fn marker_of(c: &Connector) -> Option<Marker> {
    Some(Marker::Instance {
        device: c.device.clone()?,
        id: c.id.clone()?,
    })
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
fn force_on(state: &mut State, held: &Held) -> ResultType<()> {
    let (name, sysfs) = (held.name.as_str(), held.sysfs.as_str());
    install_edid(name, &held.marker)?;
    if let Err(e) = write_status(sysfs, "on") {
        // Roll back, or the override outlives the attempt: nothing would be forced, so nothing would
        // look like ours, and a monitor later plugged into that connector would be described by an
        // EDID we left behind with no owner.
        uninstall_edid();
        return Err(e);
    }
    state.forced = vec![held.clone()];
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
            edid_firmware_ref(&held.marker)
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
        if let Some(marker) = marker_of(c) {
            state.forced = vec![Held {
                sysfs: c.sysfs.clone(),
                name: c.name.clone(),
                marker,
            }];
        }
        return Ok(c.sysfs.clone());
    }
    if all.iter().any(is_real_output) {
        bail!("a display is already attached, nothing to force");
    }
    let target = pick_connector(&all).ok_or_else(|| anyhow!("no connector to force"))?;
    // A connector that cannot state its identity is not forced at all: the marker would have to be
    // written against something other cards could share, and it would then follow the name onto a
    // stranger after a rebind.
    let marker = marker_of(target).ok_or_else(|| {
        anyhow!(
            "{} cannot be identified (no device link or connector_id in sysfs), refusing to force it",
            target.sysfs
        )
    })?;
    let held = Held {
        sysfs: target.sysfs.clone(),
        name: target.name.clone(),
        marker,
    };
    force_on(state, &held)?;
    Ok(held.sysfs)
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
    for h in std::mem::take(&mut state.forced) {
        add_target(&mut targets, &all, &h.sysfs);
    }
    // Read the parameter while it still exists. A force whose EDID never loaded leaves no other
    // trace, and the entries below are about to go - which would leave the connector forced for the
    // rest of the boot with nothing able to name it. A marker only resolves to the connector it
    // actually names: both the card minor and the connector name get recycled, so the device and
    // the KMS object id are what keep it from landing on somebody else's connector.
    for (name, marker) in connectors_named_by_our_entries() {
        for c in all
            .iter()
            .filter(|c| c.name == name && marker.claims(c.device.as_deref(), c.id.as_deref()))
        {
            add_target(&mut targets, &all, &c.sysfs);
        }
    }
    let mut last_err = None;
    let mut released: Vec<String> = Vec::new();
    let mut still_held: Vec<String> = Vec::new();
    for t in &targets {
        // The override comes off before the unforce, or the re-probe would read our EDID straight
        // back and a real monitor on that connector would keep being described by it. One entry at
        // a time: every connector gets its write attempted, and a failure must not strand the
        // others with their override already gone.
        drop_our_entries(|e| entry_connector(e) == Some(t.name.as_str()));
        if let Err(e) = write_status(&t.sysfs, "detect") {
            log::warn!("headless display: cannot release {}: {e}", t.sysfs);
            // Put the marker back. The parameter entry is what survives this process and, for a
            // force whose override never loaded, it is the only record there is. A target whose
            // connector is gone has no identity to record, and an unqualified marker would follow
            // the name onto whatever appears next, so that one is left to the process record alone.
            match &t.marker {
                Some(marker) => {
                    if let Err(e) = install_edid(&t.name, marker) {
                        log::warn!(
                            "headless display: cannot re-record the hold on {}: {e}",
                            t.sysfs
                        );
                    } else {
                        still_held.push(t.name.clone());
                    }
                    state.forced.push(Held {
                        sysfs: t.sysfs.clone(),
                        name: t.name.clone(),
                        marker: marker.clone(),
                    });
                }
                None => log::warn!(
                    "headless display: {} could not be released and is gone from sysfs, so there \
                     is nothing left to record the hold against",
                    t.sysfs
                ),
            }
            last_err = Some(e);
        } else {
            released.push(t.sysfs.clone());
        }
    }
    // Whatever is left of ours names no connector this pass could act on - an override orphaned by
    // an earlier failure, or one whose card is gone - and it would keep painting our EDID on the
    // next probe of that name.
    drop_our_entries(|e| !entry_connector(e).is_some_and(|c| still_held.iter().any(|h| h == c)));
    if !released.is_empty() {
        log::info!("headless display: released {}", released.join(", "));
    }
    match last_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// One connector a release has to act on, carried with the facts needed to put its marker back.
/// `marker` is `None` for a connector that is gone from sysfs: there is nothing left to identify.
struct Target {
    sysfs: String,
    name: String,
    marker: Option<Marker>,
}

fn add_target(targets: &mut Vec<Target>, all: &[Connector], sysfs: &str) {
    if targets.iter().any(|t| t.sysfs == sysfs) {
        return;
    }
    match all.iter().find(|c| c.sysfs == sysfs) {
        Some(c) => targets.push(Target {
            sysfs: c.sysfs.clone(),
            name: c.name.clone(),
            marker: marker_of(c),
        }),
        // Gone from sysfs since the record was made. Still worth the write - the connector may be
        // forced on with nothing else able to name it - but no marker can be written for it.
        None => targets.push(Target {
            sysfs: sysfs.to_owned(),
            name: sysfs
                .split_once('-')
                .map(|(_, name)| name.to_owned())
                .unwrap_or_default(),
            marker: None,
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
fn adopt_forced(all: &mut [Connector], forced: &[Held]) {
    for c in all.iter_mut() {
        // The full identity, not the sysfs name: both the card minor and the connector name are
        // recycled, so a name-only record would re-adopt whatever appeared in the old one's place
        // and then hide it from `is_real_output`.
        if c.connected
            && forced.iter().any(|h| {
                h.sysfs == c.sysfs && h.marker.claims(c.device.as_deref(), c.id.as_deref())
            })
        {
            c.ours = true;
        }
    }
}

/// Whether the held connector is due a probe of its own.
///
/// Never while a capture is open: the probe drops the force for a moment, and on a single-output
/// machine that empties the topology, which restarts the video service of whoever is watching. So
/// this waits for a machine nobody is looking at - which is also exactly when it matters, since the
/// display it is looking for is no use to anybody while a remote session is already running.
fn real_display_probe_due(last: Option<Instant>, capture_active: bool) -> bool {
    if capture_active {
        return false;
    }
    match last {
        None => true,
        Some(t) => t.elapsed() >= HELD_CONNECTOR_PROBE_INTERVAL,
    }
}

/// Drop the force on each held connector for one probe and see what it says on its own.
///
/// Only the FORCE comes off; the override stays installed, so a connector with a display on it
/// comes back `connected` and one with nothing comes back `disconnected`. That is the whole
/// question, and it is answered without touching the parameter, so nothing else has to be undone if
/// this fails halfway.
fn probe_held_connectors(state: &mut State) {
    let held: Vec<String> = connectors()
        .into_iter()
        .filter(|c| c.ours)
        .map(|c| c.sysfs)
        .collect();
    for sysfs in held {
        if let Err(e) = write_status(&sysfs, "detect") {
            log::warn!("headless display: cannot probe {sysfs} for a display of its own: {e}");
            continue;
        }
        let connected = wait_for(&sysfs, true);
        if connected {
            log::info!(
                "headless display: {sysfs} reports a display of its own; giving the connector back"
            );
            let _ = disable(state);
            return;
        }
        // Nothing there. Put the force back; the override never came off, so this is one write.
        if let Err(e) = write_status(&sysfs, "on") {
            log::warn!("headless display: cannot re-force {sysfs} after probing it: {e}");
        }
    }
}

/// Bring the markers back in line with the machine, in one pass with enumerated outcomes.
///
/// Two things go wrong on their own. A marker can name a connector INSTANCE that no longer exists:
/// `drm_connector_cleanup()` returns the name to its allocator, so a later connector - on another
/// card, or on the same one - can carry it, and `edid_firmware` has no syntax to say which. Such a
/// marker is dropped and whatever now holds that name is re-probed, because without the `detect`
/// write it keeps serving the blob it already read. And a marker written by an older build carries
/// no identity at all; leaving it would make it a permanent wildcard, so it is either rewritten
/// against the one connector it can be attributed to, or dropped like any other unattributable
/// hold.
///
/// Returns whether anything changed, so the caller can re-read a topology this just moved.
fn reconcile_markers(all: &[Connector]) -> bool {
    let mut drop_names: Vec<String> = Vec::new();
    let mut reprobe: Vec<String> = Vec::new();
    let mut upgrade: Vec<(String, Marker)> = Vec::new();
    for (name, marker) in connectors_named_by_our_entries() {
        match marker_action(&name, &marker, all) {
            MarkerAction::Keep => {}
            MarkerAction::Upgrade(m) => upgrade.push((name, m)),
            MarkerAction::Drop { reprobe: probe } => {
                log::info!("headless display: dropping the hold on {name} ({marker:?})");
                if probe {
                    reprobe.push(name.clone());
                }
                drop_names.push(name);
            }
        }
    }
    if drop_names.is_empty() && upgrade.is_empty() {
        return false;
    }
    if !drop_names.is_empty() {
        drop_our_entries(|e| entry_connector(e).is_some_and(|c| drop_names.iter().any(|n| n == c)));
    }
    for (name, marker) in &upgrade {
        if let Err(e) = install_edid(name, marker) {
            log::warn!("headless display: cannot re-record the hold on {name}: {e}");
        } else {
            log::info!("headless display: the hold on {name} now records which connector it is");
        }
    }
    for c in all
        .iter()
        .filter(|c| reprobe.iter().any(|name| *name == c.name))
    {
        if let Err(e) = write_status(&c.sysfs, "detect") {
            log::warn!("headless display: cannot re-probe {}: {e}", c.sysfs);
        }
    }
    true
}

/// What has to happen to one marker. Four outcomes and no fifth, which is the point of naming them:
/// the bug this replaced came from a marker that fell through every branch and stayed forever.
#[derive(Debug, PartialEq)]
enum MarkerAction {
    Keep,
    /// A marker from an older build, attributed to exactly one connector that can identify itself.
    Upgrade(Marker),
    /// Give the hold back. `reprobe` when a live connector still carries the name: it is serving
    /// the EDID the kernel already handed it, and only a `detect` write makes it read the wire.
    Drop {
        reprobe: bool,
    },
}

fn marker_action(name: &str, marker: &Marker, all: &[Connector]) -> MarkerAction {
    match marker {
        // No identity at all. Left alone it would claim any connector that ever carries this name,
        // which is the failure the qualified form exists to prevent, so it does not survive a pass.
        Marker::Legacy => {
            let mut candidates = all.iter().filter(|c| c.name == name);
            match (candidates.next(), candidates.next()) {
                (Some(c), None) => match marker_of(c) {
                    Some(m) => MarkerAction::Upgrade(m),
                    None => MarkerAction::Drop { reprobe: true },
                },
                // None: nothing to attribute it to. Several: attributing it would be a guess.
                (found, _) => MarkerAction::Drop {
                    reprobe: found.is_some(),
                },
            }
        }
        Marker::Instance { .. } => {
            if all
                .iter()
                .any(|c| c.name == name && marker.claims(c.device.as_deref(), c.id.as_deref()))
            {
                MarkerAction::Keep
            } else {
                MarkerAction::Drop {
                    reprobe: all.iter().any(|c| c.name == name),
                }
            }
        }
    }
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
        // skips the stability delay. The probe clock goes with it, so a re-enable looks at once
        // rather than waiting out an interval that started under the old setting.
        state.real_since = None;
        state.last_probe = None;
        return;
    }

    let mut all = connectors();
    if reconcile_markers(&all) {
        all = connectors();
    }
    adopt_forced(&mut all, &state.forced);
    if all.iter().any(|c| c.ours) {
        // A real display takes precedence over ours: release it, and the machine goes back to the
        // output the operator actually plugged in. A connector OTHER than the one we hold shows
        // that by itself; the one we hold cannot, because the kernel serves our override for it -
        // so a monitor plugged into that port would stay invisible for the rest of the boot, and on
        // a machine with one output, which is the machine this feature is for, that is its only
        // display. Measured on such a box: it had a monitor attached the whole time and our EDID
        // was what everything read. So the held connector gets re-probed on its own.
        if !all.iter().any(is_real_output) {
            state.real_since = None;
            if real_display_probe_due(state.last_probe, crate::ipc::drm_capture_active()) {
                state.last_probe = Some(Instant::now());
                probe_held_connectors(state);
            }
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
                .map(|(name, marker)| match marker {
                    Marker::Legacy => format!("{name} (recorded by an older build)"),
                    Marker::Instance { device, id } => format!("{name} on {device} #{id}"),
                })
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
        let inst = |d: &str, i: &str| Marker::Instance {
            device: d.to_owned(),
            id: i.to_owned(),
        };
        let mine = |c: &str, d: &str, i: &str| format!("{c}:edid/rustdesk-headless-{d}-{i}.bin");
        let names = |v: &str| {
            edid_entries(v)
                .into_iter()
                .filter_map(|e| Some((entry_connector(e)?.to_owned(), entry_marker(e)?)))
                .collect::<Vec<_>>()
        };
        let d = "a1b2c3d4e5f60718";
        assert_eq!(
            names(&format!("DP-1:edid/theirs.bin,{}", mine("HDMI-A-1", d, "508"))),
            vec![("HDMI-A-1".to_owned(), inst(d, "508"))]
        );
        assert_eq!(
            names(&format!("{},DP-1:edid/theirs.bin", mine("HDMI-A-1", d, "508"))),
            vec![("HDMI-A-1".to_owned(), inst(d, "508"))]
        );
        // Two of ours, which is what a machine that forced one connector and then another leaves.
        assert_eq!(
            names(&format!(
                "{},{}",
                mine("HDMI-A-1", d, "508"),
                mine("DP-2", d, "528")
            )),
            vec![
                ("HDMI-A-1".to_owned(), inst(d, "508")),
                ("DP-2".to_owned(), inst(d, "528"))
            ]
        );
        assert!(names("DP-1:edid/theirs.bin").is_empty());
        assert!(names("").is_empty());
        // What an earlier build wrote: still ours, so still releasable, with no identity to check.
        assert_eq!(
            names("HDMI-A-1:edid/rustdesk-headless.bin"),
            vec![("HDMI-A-1".to_owned(), Marker::Legacy)]
        );
        // A file that merely starts like ours is not ours.
        assert!(names("HDMI-A-1:edid/rustdesk-headless-thing.png").is_empty());
    }

    // rustdesk#15908: every marker has to leave one pass with a decision. The bug this replaces was
    // a marker that matched no branch and stayed valid for the rest of the boot.
    #[test]
    fn every_marker_leaves_a_pass_with_a_decision() {
        let inst = |d: &str, i: &str| Marker::Instance {
            device: d.to_owned(),
            id: i.to_owned(),
        };
        let live = vec![c("card1-HDMI-A-1", "HDMI-A-1", true, false)];
        let mine = marker_of(&live[0]).unwrap();

        // The connector it names is there: nothing to do.
        assert_eq!(
            marker_action("HDMI-A-1", &mine, &live),
            MarkerAction::Keep
        );
        // Same name, another connector instance: give it back AND re-probe, because that connector
        // is already serving the EDID the kernel handed it.
        assert_eq!(
            marker_action("HDMI-A-1", &inst("card1", "card1-HDMI-A-9"), &live),
            MarkerAction::Drop { reprobe: true }
        );
        // The connector is gone entirely: give it back, with nothing to re-probe.
        assert_eq!(
            marker_action("DP-4", &inst("card9", "card9-DP-4"), &live),
            MarkerAction::Drop { reprobe: false }
        );
        // An older build's marker, attributable to exactly one connector: record what it is.
        assert_eq!(
            marker_action("HDMI-A-1", &Marker::Legacy, &live),
            MarkerAction::Upgrade(mine)
        );
        // The same marker with the name on two cards cannot be attributed, so it is not kept.
        let twins = vec![
            c("card1-DP-1", "DP-1", true, false),
            c("card2-DP-1", "DP-1", true, false),
        ];
        assert_eq!(
            marker_action("DP-1", &Marker::Legacy, &twins),
            MarkerAction::Drop { reprobe: true }
        );
        // And one that names nothing live is dropped rather than left as a wildcard.
        assert_eq!(
            marker_action("DP-9", &Marker::Legacy, &live),
            MarkerAction::Drop { reprobe: false }
        );
    }

    // rustdesk#15908: the connector we hold is the one place a display can appear without us being
    // able to see it, because the kernel serves our override for it. So it gets probed - but never
    // while somebody is capturing, since the probe empties the topology for a moment and that
    // restarts their video service.
    #[test]
    fn the_held_connector_is_probed_only_while_nobody_is_watching() {
        let long_ago = Instant::now() - HELD_CONNECTOR_PROBE_INTERVAL;
        // Never probed: look now.
        assert!(real_display_probe_due(None, false));
        // ...but not with a capture open, however overdue it is.
        assert!(!real_display_probe_due(None, true));
        assert!(!real_display_probe_due(Some(long_ago), true));
        // Interval elapsed on an idle machine: look.
        assert!(real_display_probe_due(Some(long_ago), false));
        // Just looked: leave it alone, or every poll would blip the topology.
        assert!(!real_display_probe_due(Some(Instant::now()), false));
    }

    // rustdesk#15908: a device identity has to survive being put in a file name without two
    // different devices ever landing on the same value.
    #[test]
    fn two_devices_never_share_an_identity() {
        // Same basename, different bus path: trimming to the basename would have merged these.
        let a = fnv1a(b"../../devices/pci0000:00/0000:00:02.0");
        let b = fnv1a(b"../../devices/pci0000:80/0000:00:02.0");
        assert_ne!(a, b);
        assert_eq!(a.len(), 16);
        // Stable, because a marker written by one build is read by the next.
        assert_eq!(a, fnv1a(b"../../devices/pci0000:00/0000:00:02.0"));
    }

    // rustdesk#15908: the kernel frees a connector's name when the connector goes, so a rebind can
    // hand `HDMI-A-1` to a different card - and `edid_firmware` has no card syntax to stop our
    // entry following it there. The device the entry records is what refuses the stranger.
    #[test]
    fn a_marker_does_not_follow_its_name_onto_another_connector() {
        let inst = |d: &str, i: &str| Marker::Instance {
            device: d.to_owned(),
            id: i.to_owned(),
        };
        let named = vec![("HDMI-A-1".to_owned(), inst("devA", "508"))];
        let ours = |device, id, named: &[(String, Marker)]| {
            connector_is_ours(true, &[], "HDMI-A-1", device, id, named, true)
        };
        assert!(ours(Some("devA"), Some("508"), &named));
        // Same name, another card: the device half refuses it.
        assert!(!ours(Some("devB"), Some("508"), &named));
        // Same name and the SAME card, but a connector created later - the kernel returns a
        // connector's name to its allocator when it goes, so the device alone would accept this.
        assert!(!ours(Some("devA"), Some("531"), &named));
        // A connector that cannot state its own identity is claimed by nothing.
        assert!(!ours(None, Some("508"), &named));
        assert!(!ours(Some("devA"), None, &named));
        // An entry from an earlier build carries no identity, so it still resolves by name - it is
        // rewritten or dropped by `reconcile_markers`, never left standing as a wildcard.
        let legacy = vec![("HDMI-A-1".to_owned(), Marker::Legacy)];
        assert!(ours(Some("devB"), Some("999"), &legacy));
    }

    #[test]
    fn a_force_whose_edid_never_loaded_is_still_ours() {
        let mine = synthetic_edid();
        let (dev, id) = (Some("devA"), Some("508"));
        let named = vec![(
            "HDMI-A-1".to_owned(),
            Marker::Instance {
                device: "devA".to_owned(),
                id: "508".to_owned(),
            },
        )];

        // The ordinary case: our EDID came back, and the name is not even needed.
        assert!(connector_is_ours(true, &mine, "HDMI-A-1", dev, id, &[], true));
        // The case this exists for: forced on, override never loaded, so the connector reports
        // nothing of ours. Only our own edid_firmware entry still names it.
        assert!(connector_is_ours(true, &[], "HDMI-A-1", dev, id, &named, true));
        // A real monitor on a connector we never named is never ours, either way round.
        assert!(!connector_is_ours(true, &[], "DP-2", dev, id, &named, true));
        assert!(!connector_is_ours(true, &[], "DP-2", dev, id, &[], true));
        // And a disconnected connector is nobody's, whatever the parameter says.
        assert!(!connector_is_ours(false, &mine, "HDMI-A-1", dev, id, &named, true));
        // A duplicated name claims nothing by name alone: with DP-1 on two cards, the fallback
        // would mark both ours and a real display behind one of them would never release ours.
        assert!(!connector_is_ours(true, &[], "HDMI-A-1", dev, id, &named, false));
        // The EDID still decides when it does read back, duplicated name or not.
        assert!(connector_is_ours(true, &mine, "HDMI-A-1", dev, id, &named, false));
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
        // The card stands in for the device identity and the sysfs name for the instance id, so a
        // test connector is as distinguishable as a real one.
        Connector {
            name: name.to_owned(),
            sysfs: sysfs.to_owned(),
            connected,
            ours,
            device: sysfs.split_once('-').map(|(card, _)| card.to_owned()),
            id: Some(sysfs.to_owned()),
            drivable: true,
        }
    }

    fn held(sysfs: &str, name: &str) -> Held {
        Held {
            sysfs: sysfs.to_owned(),
            name: name.to_owned(),
            marker: Marker::Instance {
                device: sysfs.split_once('-').map(|(card, _)| card).unwrap_or("").to_owned(),
                id: sysfs.to_owned(),
            },
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
        adopt_forced(&mut all, &[held("card1-HDMI-A-1", "HDMI-A-1")]);
        assert!(all[0].ours, "the held connector is adopted by sysfs name");
        assert!(!all[1].ours);
        // And the bare name must not match: the record is card-qualified.
        let mut all2 = vec![c("card1-HDMI-A-1", "HDMI-A-1", true, false)];
        adopt_forced(&mut all2, &[held("HDMI-A-1", "HDMI-A-1")]);
        assert!(!all2[0].ours);
        adopt_forced(&mut all2, &[]);
        assert!(!all2[0].ours);
        // A disconnected forced record means the kernel-side force was lost: not adopted, so the
        // arming path can re-force instead of the tick parking in the release watch.
        let mut all3 = vec![c("card1-HDMI-A-1", "HDMI-A-1", false, false)];
        adopt_forced(&mut all3, &[held("card1-HDMI-A-1", "HDMI-A-1")]);
        assert!(!all3[0].ours);
        // A release that failed on two connectors remembers both, or the second is never retried.
        let mut all4 = vec![
            c("card1-HDMI-A-1", "HDMI-A-1", true, false),
            c("card1-DP-2", "DP-2", true, false),
        ];
        adopt_forced(
            &mut all4,
            &[held("card1-HDMI-A-1", "HDMI-A-1"), held("card1-DP-2", "DP-2")],
        );
        assert!(all4.iter().all(|c| c.ours));
    }
}
