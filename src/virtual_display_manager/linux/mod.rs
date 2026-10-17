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
//     carrying our EDID from a previous run of the service or named by our own `edid_firmware`
//     entry, and nothing else.
//
// Known limit, stated because sysfs cannot express it: the kernel has no attribute that reports
// `connector->force`, so a connector an operator forced *off* by hand is indistinguishable from one
// with nothing plugged into it, and this code may force it back on. There is also no way to see a
// monitor plugged into the connector we are holding, because our own EDID override is what that
// connector reports; plugging into any other connector is noticed within a poll.

use base::config::keys::OPTION_ALLOW_HEADLESS_DISPLAY;
use hbb_common::{
    anyhow::anyhow,
    bail,
    config::{option2bool, Config, DEFAULT_SETTINGS, OVERWRITE_SETTINGS},
    libc, log, tokio, ResultType,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const DRM_CLASS: &str = "/sys/class/drm";
const EDID_PARAM: &str = "/sys/module/drm/parameters/edid_firmware";
const EDID_DIR: &str = "/lib/firmware/edid";
const EDID_NAME_PREFIX: &str = "rustdesk-headless";
/// The relative name the kernel resolves under the firmware search path.
const EDID_DIR_REF: &str = "edid/";

/// How long the machine must report no output before a connector is forced, long enough that a
/// monitor waking up, a KVM switching back or a driver rebinding wins the race and nothing is forced
/// at all. Not on the first pass after the service starts: a monitor there is already reporting
/// itself, and the output a stop just released has to be back before the `--server` starts taking
/// connections, or a client that reconnects at once finds no display and is sent to the portal.
/// A real display that appears afterwards still releases ours (`REAL_OUTPUT_STABLE`).
const NO_OUTPUT_STABLE: Duration = Duration::from_secs(5);
/// sysfs re-read interval. The service loop ticks twice a second; this feature does not need to.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// A failure the watcher meets again on every poll is logged once per this interval, with a count.
const FAILURE_LOG_INTERVAL: Duration = Duration::from_secs(60);
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

/// `allow-` and not `enable-` is load-bearing. `option2bool` reads an `enable-` key as `value != "N"`
/// - an absent value would default this ON - and an `allow-` key as `value == "Y"`, which is off
/// until somebody says otherwise.
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
            || (name_unique && named.iter().any(|(n, m)| n == name && m.claims(device, id))))
}

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

/// What one of our markers records about the connector it holds: the device and the KMS object id,
/// which together name the connector instance.
#[derive(Clone, Debug, PartialEq)]
struct Marker {
    device: String,
    id: String,
}

impl Marker {
    /// Does this marker claim the connector with this identity? A connector that cannot state its
    /// own identity is claimed by nothing.
    fn claims(&self, device: Option<&str>, id: Option<&str>) -> bool {
        device == Some(self.device.as_str()) && id == Some(self.id.as_str())
    }

    fn file_name(&self) -> String {
        format!("{EDID_NAME_PREFIX}-{}-{}.bin", self.device, self.id)
    }
}

/// The marker an entry carries, or `None` when the entry is not ours at all.
fn entry_marker(entry: &str) -> Option<Marker> {
    let file = entry_file(entry).strip_prefix(EDID_DIR_REF)?;
    let rest = file.strip_prefix(EDID_NAME_PREFIX)?.strip_suffix(".bin")?;
    // `<device>-<id>`: both halves are our own alphanumeric tokens, so the LAST dash splits them.
    let (device, id) = rest.strip_prefix('-')?.rsplit_once('-')?;
    if device.is_empty() || id.is_empty() {
        return None;
    }
    Some(Marker {
        device: device.to_owned(),
        id: id.to_owned(),
    })
}

fn entry_is_ours(entry: &str) -> bool {
    entry_marker(entry).is_some()
}

/// Does an entry we did not write already govern this connector? An entry with no `<connector>:`
/// prefix applies to every connector, so it governs any of them.
///
/// The prefix is matched the way the kernel matches it, not by equality: `drm_edid_load` compares
/// the part before the colon against the START of the connector name, so `DP:operator.bin` governs
/// `DP-1` and stays ahead of anything we append. Comparing for equality let us force a connector
/// somebody else had already claimed, which is exactly what this check exists to prevent.
fn foreign_entry_covers(entry: &str, connector: &str) -> bool {
    !entry_is_ours(entry)
        && match entry.split_once(':') {
            Some((target, _)) => connector.starts_with(target),
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
fn drop_our_entries(mut which: impl FnMut(&str) -> bool) -> ResultType<()> {
    let existing = read_edid_param();
    let entries = edid_entries(&existing);
    let (mine, keep): (Vec<&str>, Vec<&str>) = entries
        .into_iter()
        .partition(|e| entry_is_ours(e) && which(e));
    if mine.is_empty() {
        return Ok(());
    }
    if let Err(e) = write_edid_param(&keep.join(",")) {
        hbb_common::throttled_log!(
            FAILURE_LOG_INTERVAL,
            warn,
            "headless display: cannot clear our edid_firmware entry: {e}"
        );
        return Err(e);
    }
    for e in &mine {
        let Some(marker) = entry_marker(e) else {
            continue;
        };
        // Another entry of ours can still point at the same file.
        if keep
            .iter()
            .any(|k| entry_marker(k).as_ref() == Some(&marker))
        {
            continue;
        }
        let _ = std::fs::remove_file(edid_path(&marker));
    }
    Ok(())
}

fn uninstall_edid() {
    let _ = drop_our_entries(|_| true);
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
///
/// A name that is a PREFIX of another enumerated connector is refused for the same reason: the
/// kernel matches our `DP-1:` entry against the start of a name, so it would also hand our EDID to
/// `DP-10`, which we never picked and would then classify as ours. This is decided against the
/// snapshot in hand: a `DP-10` that only appears LATER still loads the entry, which is
/// the limit stated at the top of this file.
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
            && !all
                .iter()
                .any(|o| o.name != c.name && o.name.starts_with(c.name.as_str()))
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
    /// The switch as the last pass saw it, so leftovers are looked for when the service starts and
    /// when the switch goes off, and not on every poll of a host that never turned it on.
    was_enabled: Option<bool>,
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
    Some(Marker {
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
    // Armed before anything is written, so a stop that lands mid-force still gives it back.
    release_on_stop(true);
    if let Err(e) = install_edid(name, &held.marker) {
        release_on_stop(!state.forced.is_empty());
        return Err(e);
    }
    if let Err(e) = write_status(sysfs, "on") {
        // Roll back, or the override outlives the attempt: nothing would be forced, so nothing would
        // look like ours, and a monitor later plugged into that connector would be described by an
        // EDID we left behind with no owner.
        uninstall_edid();
        release_on_stop(!state.forced.is_empty());
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
        release_on_stop(true);
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
        add_target(&mut targets, c);
    }
    for h in std::mem::take(&mut state.forced) {
        match hold_target(&h, &all) {
            Some(c) => add_target(&mut targets, c),
            None => log::info!(
                "headless display: dropping stale hold on {} ({}); it is no longer the connector \
                 this process forced",
                h.sysfs,
                h.name
            ),
        }
    }
    // Read the parameter while it still exists. A force whose EDID never loaded leaves no other
    // trace, and the entries below are about to go - which would leave the connector forced for the
    // rest of the boot with nothing able to name it. A marker only resolves to the connector it
    // actually names: both the card minor and the connector name get recycled, so the device and
    // the KMS object id are what keep it from landing on somebody else's connector.
    for (name, marker) in connectors_named_by_our_entries() {
        for c in marker_targets(&name, &marker, &all) {
            add_target(&mut targets, c);
        }
    }
    let mut last_err = None;
    let mut released: Vec<String> = Vec::new();
    let mut still_held: Vec<String> = Vec::new();
    for t in &targets {
        // The unforce comes FIRST, with our override still installed. A `detect` that fails then
        // has touched nothing, so there is no marker to put back and no window in which a second
        // failure could lose the record - the parameter entry simply stays, as it was. Only once
        // the connector is off the force does the entry come off, and one more `detect` makes it
        // read the wire instead of the EDID it already holds.
        if let Err(e) = write_status(&t.sysfs, "detect") {
            hbb_common::throttled_log!(
                FAILURE_LOG_INTERVAL,
                warn,
                "headless display: cannot release {}: {e}",
                t.sysfs
            );
            match &t.marker {
                Some(marker) => {
                    still_held.push(t.name.clone());
                    state.forced.push(Held {
                        sysfs: t.sysfs.clone(),
                        name: t.name.clone(),
                        marker: marker.clone(),
                    });
                }
                None => hbb_common::throttled_log!(
                    FAILURE_LOG_INTERVAL,
                    warn,
                    "headless display: {} could not be released and cannot state its identity, so \
                     there is nothing left to record the hold against",
                    t.sysfs
                ),
            }
            last_err = Some(e);
            continue;
        }
        if let Err(e) = drop_our_entries(|e| entry_connector(e) == Some(t.name.as_str())) {
            last_err = Some(e);
        }
        if let Err(e) = write_status(&t.sysfs, "detect") {
            // Released. Only a connector that is going away fails here: `status_store` returns
            // -EINTR, which the write retries, or -EINVAL, and ignores the probe result. Its cached
            // EDID goes with it, and our entry is already gone, so whatever comes back under the
            // name reads the wire.
            hbb_common::throttled_log!(
                FAILURE_LOG_INTERVAL,
                warn,
                "headless display: {} released, but the re-probe failed: {e}",
                t.sysfs
            );
        }
        released.push(t.sysfs.clone());
    }
    // Whatever is left of ours names no connector this pass could act on - an override orphaned by
    // an earlier failure, or one whose card is gone - and it would keep painting our EDID on the
    // next probe of that name.
    if let Err(e) =
        drop_our_entries(|e| !entry_connector(e).is_some_and(|c| still_held.iter().any(|h| h == c)))
    {
        last_err = Some(e);
    }
    if !released.is_empty() {
        log::info!("headless display: released {}", released.join(", "));
    }
    match last_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// One connector a release has to act on, carried with the facts needed to put its marker back.
/// `marker` is `None` for a connector that cannot state its own identity - no device link or no
/// `connector_id` - because a marker written against a shared identity would follow the name onto a
/// stranger. Every target comes from the live snapshot, so a path absent from it is never one.
struct Target {
    sysfs: String,
    name: String,
    marker: Option<Marker>,
}

/// The connectors one of our `edid_firmware` entries may be acted on through: exactly the one its
/// marker claims.
fn marker_targets<'a>(name: &str, marker: &Marker, all: &'a [Connector]) -> Vec<&'a Connector> {
    all.iter()
        .filter(|c| c.name == name && marker.claims(c.device.as_deref(), c.id.as_deref()))
        .collect()
}

/// The connectors a re-probe after a dropped hold may write `detect` to: the one carrying the
/// name, or on twins only those that read our EDID. A rebound connector serves our blob, so the
/// rule reaches it; a twin that is somebody's monitor is left alone.
fn reprobe_targets<'a>(name: &str, all: &'a [Connector]) -> Vec<&'a Connector> {
    let same: Vec<&Connector> = all.iter().filter(|c| c.name == name).collect();
    if same.len() == 1 {
        same
    } else {
        same.into_iter().filter(|c| c.ours).collect()
    }
}

/// The connector a remembered hold may still be released through, or `None` when the hold is stale.
///
/// Both halves of a sysfs name are recycled, so the path alone proves nothing: if another connector
/// occupies it now, writing `detect` there would clear a force that is not ours and can take that
/// output down. A path that is gone has nothing to release either - the KMS object went with it, and
/// a recreated connector starts unforced. Either way the hold is dropped rather than acted on. This
/// only governs the hold list: a connector still carrying our EDID is released by the `ours` pass in
/// `disable()` whatever this says.
fn hold_target<'a>(h: &Held, all: &'a [Connector]) -> Option<&'a Connector> {
    all.iter()
        .find(|c| c.sysfs == h.sysfs)
        .filter(|c| h.marker.claims(c.device.as_deref(), c.id.as_deref()))
}

/// Every caller takes its connector from the one live snapshot `disable()` read, so a target is
/// always a connector that exists: there is no arm here for a path that is not in it.
fn add_target(targets: &mut Vec<Target>, c: &Connector) {
    if targets.iter().any(|t| t.sysfs == c.sysfs) {
        return;
    }
    targets.push(Target {
        sysfs: c.sysfs.clone(),
        name: c.name.clone(),
        marker: marker_of(c),
    });
}

/// Is there anything of ours left to give back, including an override orphaned by a failed attempt?
///
/// The connector walk is the last of the three checks, and the only thing that finds a connector left
/// forced by an earlier run whose parameter entry is already gone. With the switch off it runs when
/// the service starts and when the switch goes off, not on every poll.
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
                    hbb_common::throttled_log!(
                        FAILURE_LOG_INTERVAL,
                        error,
                        "headless display: a poll panicked ({e:?}); continuing"
                    );
                }
                let next = Instant::now() + POLL_INTERVAL;
                while Instant::now() < next {
                    if STOP_REQUESTED.load(Ordering::SeqCst) {
                        stop(&mut state);
                    }
                    std::thread::sleep(STOP_CHECK);
                }
            }
        });
    if let Err(e) = spawned {
        log::warn!("headless display: cannot start the watcher thread: {e}");
    }
}

/// Set by the SIGTERM handler, which does nothing else: the release runs on the watcher thread.
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
/// Whether our SIGTERM handler is installed. Only while something of ours is forced, so a service
/// with the switch off stops exactly as it always has.
static STOP_HANDLER: AtomicBool = AtomicBool::new(false);
/// How quickly the watcher notices a stop between polls.
const STOP_CHECK: Duration = Duration::from_millis(100);

extern "C" fn on_sigterm(_: libc::c_int) {
    STOP_REQUESTED.store(true, Ordering::SeqCst);
}

/// The service is stopped with SIGTERM (`pkill` in the unit's `ExecStop`, then systemd's own), and
/// its Ctrl-C handler only covers SIGINT, so without this a stop or an uninstall would leave the
/// connector forced and our EDID installed until the next reboot. While something of ours is held,
/// SIGTERM is caught, the watcher gives it back, and the signal is raised again with its default
/// action, so the process still dies of it.
fn release_on_stop(held: bool) {
    if STOP_HANDLER.swap(held, Ordering::SeqCst) == held {
        return;
    }
    let handler = if held {
        on_sigterm as extern "C" fn(libc::c_int) as libc::sighandler_t
    } else {
        libc::SIG_DFL
    };
    // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
    unsafe { libc::signal(libc::SIGTERM, handler) };
}

fn stop(state: &mut State) -> ! {
    log::info!("headless display: the service is stopping, giving back what this process forced");
    if let Err(e) = disable(state) {
        log::warn!("headless display: release on stop failed: {e}");
    }
    // SAFETY: plain libc calls; the default action of SIGTERM ends the process here.
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
        libc::raise(libc::SIGTERM);
    }
    std::process::exit(143)
}

/// Wait until a session `--server` accepts a connection on its main IPC, for a root CLI that is
/// about to hand it the option. Every session has one, the greeter included, but it starts a moment
/// after the service: an install script that turns the feature on straight away would find none.
#[tokio::main(flavor = "current_thread")]
async fn wait_for_session_server() -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if crate::ipc::connect(1000, "").await.is_ok() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

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

/// Drop a marker that names a connector INSTANCE that no longer exists. `drm_connector_cleanup()`
/// returns the name to its allocator, so a later connector - on another card, or on the same one -
/// can carry it, and `edid_firmware` has no syntax to say which. Whatever now holds that name is
/// re-probed, because without the `detect` write it keeps serving the blob it already read.
///
/// Returns whether anything changed, so the caller can re-read a topology this just moved.
fn reconcile_markers(all: &[Connector]) -> bool {
    let gone: Vec<(String, Marker)> = connectors_named_by_our_entries()
        .into_iter()
        .filter(|(name, marker)| marker_targets(name, marker, all).is_empty())
        .collect();
    if gone.is_empty() {
        return false;
    }
    let _ =
        drop_our_entries(|e| entry_connector(e).is_some_and(|c| gone.iter().any(|(n, _)| n == c)));
    for (name, marker) in &gone {
        log::info!("headless display: dropping the hold on {name} ({marker:?})");
        // On twins only the one that reads our EDID is written to: the other is somebody's monitor.
        for c in reprobe_targets(name, all) {
            if let Err(e) = write_status(&c.sysfs, "detect") {
                hbb_common::throttled_log!(
                    FAILURE_LOG_INTERVAL,
                    warn,
                    "headless display: cannot re-probe {}: {e}",
                    c.sysfs
                );
            }
        }
    }
    true
}

/// The switch as the service holds it, read with the message a `--server` pulls its config with,
/// and resolved the way `Config::get_option` resolves it: a custom client can pin it or default it.
#[tokio::main(flavor = "current_thread")]
async fn service_switch() -> ResultType<bool> {
    use crate::ipc::Data;
    const KEY: &str = OPTION_ALLOW_HEADLESS_DISPLAY;
    let mut c = crate::ipc::connect_service(1000).await?;
    c.send(&Data::SyncConfig(None)).await?;
    let Some(Data::SyncConfig(Some(configs))) = c.next_timeout(1000).await? else {
        bail!("unexpected answer from the service");
    };
    let value = OVERWRITE_SETTINGS
        .read()
        .unwrap()
        .get(KEY)
        .cloned()
        .or_else(|| configs.1.options.get(KEY).cloned())
        .or_else(|| DEFAULT_SETTINGS.read().unwrap().get(KEY).cloned())
        .unwrap_or_default();
    Ok(option2bool(KEY, &value))
}

/// Wait for the service to hold the switch a root CLI has just set on the session `--server`, which
/// syncs a change to it within a second.
fn wait_for_service_switch(on: bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if service_switch().is_ok_and(|s| s == on) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// One pass. Silent unless something changes.
///
/// The option is read from `Config`, which the settings checkbox and `--headless-display` both
/// reach through the session `--server`'s config sync.
fn tick(state: &mut State) {
    if !is_enabled() {
        // Turned off, or never on. Give back anything of ours, once: when the service starts (a
        // previous run may have left a force behind) and when the switch goes off. A release that
        // failed is tried again on the next poll. Otherwise this stays out of sysfs.
        if state.was_enabled != Some(false) {
            if holding_something(state) && disable(state).is_err() {
                // Still holding something, so a stop has to give it back.
                release_on_stop(true);
                return;
            }
            release_on_stop(false);
            state.was_enabled = Some(false);
        }
        state.no_output_since = None;
        state.last_failure = None;
        // A release timer started before the toggle must not survive it, or a later re-enable
        // skips the stability delay.
        state.real_since = None;
        return;
    }
    // The first pass after the service starts with the switch on. See `NO_OUTPUT_STABLE`.
    let starting = state.was_enabled.is_none();
    state.was_enabled = Some(true);

    let mut all = connectors();
    if reconcile_markers(&all) {
        all = connectors();
    }
    adopt_forced(&mut all, &state.forced);
    if all.iter().any(|c| c.ours) {
        // Something of ours is up, possibly adopted from a previous run: a stop gives it back.
        release_on_stop(true);
        // A real display takes precedence over ours: release it, and the machine goes back to the
        // output the operator actually plugged in. Only a connector OTHER than the one we hold can
        // show that; see the top of this file for the one we hold.
        if !all.iter().any(is_real_output) {
            state.real_since = None;
            return;
        }
        if state.real_since.get_or_insert_with(Instant::now).elapsed() < REAL_OUTPUT_STABLE {
            return;
        }
        state.real_since = None;
        log::info!("headless display: a real display was attached");
        if disable(state).is_ok() {
            release_on_stop(false);
        }
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
    if !starting
        && state
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

/// `rustdesk --headless-display [status|enable|disable]`, with the arguments `core_main` got. Every
/// failure prints why and exits 1. `settings_disabled` is the check `core_main` makes for `--option`;
/// it is private there, so it is passed in. Only enable and disable call it.
pub fn cli(args: &[String], settings_disabled: fn() -> bool) {
    const KEY: &str = OPTION_ALLOW_HEADLESS_DISPLAY;
    fn fail(msg: &str) -> ! {
        println!("{msg}");
        std::process::exit(1)
    }
    match args.get(1).map(String::as_str) {
        // Every connector's status is world readable. The switch comes from the session
        // `--server`, the way `--option` reads one.
        Some("status") | None => {
            let enabled = option2bool(
                KEY,
                crate::ipc::get_options()
                    .get(KEY)
                    .map_or("", |v| v.as_str()),
            );
            print!("{}", status_text(enabled));
        }
        Some(verb @ ("enable" | "disable")) => {
            if settings_disabled() {
                fail("Settings are disabled!");
            }
            // Disabling must always be possible: with DRM temporarily gone the setting would
            // otherwise stay armed and re-fire when DRM returns.
            if verb == "enable" && !is_supported() {
                fail("No DRM connectors on this machine!");
            }
            if !(crate::platform::is_installed() && crate::platform::is_root()) {
                fail("Installation and administrative privileges required!");
            }
            // The same path as the settings checkbox: the option goes to the session `--server`,
            // which syncs it to the service. Forcing the connector is the service's job. Right
            // after an install that `--server` may still be starting.
            if !wait_for_session_server() {
                fail("No RustDesk session is running yet, try again in a moment.");
            }
            let on = verb == "enable";
            // Done only once the service holds it. A `--server` that has just started takes the
            // service's config on its first sync and can drop a value set before that, so the
            // option is set once more after it.
            for _ in 0..2 {
                crate::ipc::set_option(KEY, if on { "Y" } else { "N" });
                if wait_for_service_switch(on) {
                    println!("Done!");
                    return;
                }
            }
            fail("Could not save the setting!");
        }
        Some(other) => {
            fail(&format!(
                "Unknown argument {other}; expected enable, disable or status"
            ));
        }
    }
}

/// What `--headless-display status` prints. `enabled` is passed in rather than read here: the CLI
/// asks the running server over IPC, the way `--option` does, so a normal user does not get told
/// about their own config while the root service acts on a different one.
fn status_text(enabled: bool) -> String {
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
                .map(|(name, m)| format!("{name} on {} #{}", m.device, m.id))
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

    fn c(sysfs: &str, name: &str, connected: bool) -> Connector {
        Connector {
            name: name.to_owned(),
            sysfs: sysfs.to_owned(),
            connected,
            ours: false,
            device: sysfs.split_once('-').map(|(card, _)| card.to_owned()),
            id: Some(sysfs.to_owned()),
            drivable: true,
        }
    }

    #[test]
    fn the_synthetic_edid_is_valid_and_recognisably_ours() {
        let e = synthetic_edid();
        assert_eq!(e.len(), 128);
        assert_eq!(&e[0..8], &[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        // The whole block must sum to zero mod 256; it is the one check every parser does.
        assert_eq!(e.iter().fold(0u8, |a, b| a.wrapping_add(*b)), 0);
        assert!(edid_bytes_are_ours(&e));
        // A real monitor's EDID is not ours, nor is a truncated read.
        let mut dell = e.clone();
        dell[8] = 0x10;
        dell[9] = 0xAC;
        assert!(!edid_bytes_are_ours(&dell));
        assert!(!edid_bytes_are_ours(&e[..64]));
    }

    #[test]
    fn connector_choice() {
        let all = vec![
            c("card0-Writeback-1", "Writeback-1", false),
            c("card0-eDP-1", "eDP-1", false),
            c("card0-VGA-1", "VGA-1", false),
            c("card0-HDMI-A-2", "HDMI-A-2", false),
        ];
        assert!(no_real_output(&all));
        assert_eq!(
            pick_connector(&all).map(|c| c.name.as_str()),
            Some("HDMI-A-2")
        );
        // A machine with a display of its own is left alone.
        let with_monitor = [
            c("card0-HDMI-A-1", "HDMI-A-1", true),
            c("card0-DP-1", "DP-1", false),
        ];
        assert!(!no_real_output(&with_monitor));
        // A display-only card (the Touch Bar of a MacBook Pro) is not output, and is never forced.
        let touch_bar = Connector {
            drivable: false,
            ..c("card0-USB-1", "USB-1", true)
        };
        assert!(no_real_output(&[touch_bar]));
        assert!(pick_connector(&[Connector {
            drivable: false,
            ..c("card0-USB-1", "USB-1", false)
        }])
        .is_none());
        // A name that exists on two cards gets our EDID on both, so neither is picked.
        assert!(pick_connector(&[
            c("card0-HDMI-A-1", "HDMI-A-1", false),
            c("card1-HDMI-A-1", "HDMI-A-1", false),
        ])
        .is_none());
    }

    #[test]
    fn an_edid_firmware_entry_of_someone_else_is_kept_and_blocks_a_force() {
        let ours = "HDMI-A-1:edid/rustdesk-headless-0123abcd-508.bin";
        let value = format!("DP-1:edid/theirs.bin,{ours}");
        let entries = edid_entries(&value);
        assert_eq!(entries.len(), 2);
        assert!(!entry_is_ours(entries[0]));
        assert!(entry_is_ours(entries[1]));
        // The kernel matches the part before the colon against the start of the connector name.
        assert!(foreign_entry_covers("DP:edid/theirs.bin", "DP-1"));
        assert!(
            foreign_entry_covers("edid/theirs.bin", "HDMI-A-1"),
            "no prefix: every connector"
        );
        assert!(!foreign_entry_covers("DP-1:edid/theirs.bin", "HDMI-A-1"));
        assert!(!foreign_entry_covers(ours, "HDMI-A-1"));
    }
}
