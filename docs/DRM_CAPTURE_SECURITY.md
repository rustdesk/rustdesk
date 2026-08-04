# DRM/KMS capture — security model & threat model

The optional `drm` feature adds a Linux capture backend that reads the active
scanout directly from DRM/KMS, **bypassing the xdg-desktop-portal consent
dialog**. It exists for unattended / login-screen / Wayland scenarios where the
portal prompt is not acceptable. Because it bypasses consent, treat it as a
**privileged, opt-in host-mode feature**, not a normal Wayland capture backend.

## How it works

Reading the active scanout needs `CAP_SYS_ADMIN` (to map other clients'
framebuffers). RustDesk's root `--service` already runs with `CAP_SYS_ADMIN`, so
the `drm` feature does the read **in-process in that root service**: it
`dlopen`s `libdrmtap.so` and calls it in direct mode — no privileged child, no
`setcap` helper. On the **default (split) path** the root service does not touch
pixels: it exports the active scanout as a DMA-BUF and passes just that
**read-only** fd to the unprivileged user `--server` over a dedicated
service-scoped IPC channel (`_drm`) via `SCM_RIGHTS`. The `--server` keeps an
**import-once EGLImage cache** (keyed on the buffer, so a given scanout buffer is
imported once and re-imports are elided), detiles/converts it to linear RGBA in
its own unprivileged address space, and feeds the encoder — so **on that path**
the root service never copies scanout pixels and never loads libEGL/libGLESv2
(measured on the running service, see *Auditing*). Only the **CPU fallback path**
(used when the seat/driver cannot produce a transferable DMA-BUF, or the consumer
has no render node of its own, see *When the CPU fallback is chosen* below)
copies the scanout to packed BGRA inside the root service and streams those bytes
over `_drm`.

**The no-GL property is a property of the default path, not of the process.** Be
precise about it, because the CPU fallback is the whole reason the split exists:
converting a scanout in-process means decoding whatever layout it is in, and a
tiled scanout (the common case on modern Intel and AMD) can only be decoded
through the GPU. `drmtap_grab_mapped` therefore reaches libdrmtap's auto-process
step, which lazily `dlopen`s libEGL/libGLESv2 **in the calling process** when the
scanout needs a GPU detile. So a host that has fallen back to the CPU path can
map the GL stack inside the `CAP_SYS_ADMIN` service. What the design does about
that is bound the cases: the fallback is entered only for the three reasons
listed below, never as a silent degradation of the split path (the loader refuses
a `libdrmtap` that cannot export the fd at all, precisely so "old library" cannot
turn into "convert in the privileged process"), and a linear or CPU-mappable
scanout is converted without touching GL. Every host measured here runs the split
path with zero GL regions in the service; a CPU-fallback host is a different
posture and is worth measuring separately. This mirrors the Windows
`portable_service` split (a privileged process captures, an unprivileged one
presents) but reuses RustDesk's own hardened IPC.

- `libdrmtap.so` is loaded through a small `dlopen` loader (`drmtap_dl`); if the
  library or one of its runtime deps is missing the load fails cleanly and the
  caller falls back to the PipeWire/portal path.
- The loader also **refuses a library that cannot do the split** — and, more
  broadly, any version outside the vetted window. Accepted is exactly the pinned
  minor with a patch floor (currently `0.5.x`, `x >= 0`): an older minor is
  refused (`0.4.x` included, even though it carries the split entry points, because
  it decodes a padded scanout pitch at the wrong stride), and a **newer minor is
  refused too** (`0.6.x` onward), because the loader mirrors C struct layouts that are only
  field-by-field verified against the pinned minor; widening the window is a
  deliberate act done together with re-verifying the layouts and moving the
  build pin. Independently of the version report, a library that does not
  actually export
  `drmtap_grab_desc` / `drmtap_open_render` / `drmtap_convert_dmabuf` (a stale or
  pre-release build) is refused as well. The only way to capture with such a library is the
  in-process convert, which in the root service means loading the vendor GL stack
  there, so it is refused and the caller falls back to PipeWire/portal. The
  privileged process therefore never loads GL because of which file happened to
  be on the load path; the CPU fallback below is entered only for a fact about
  the seat or the consumer.
- The reader restricts the device it opens to a realpath under `/dev/dri/`
  (`drm_reader.rs`); RustDesk always runs libdrmtap in direct in-process mode
  (`helper_path` is `NULL`). **No `drmtap-helper` binary is built, shipped, or
  installed by this package**: there is no `setcap`, no capability-bearing file,
  and no capture group in this deployment. Being precise about what that does
  and does not guarantee: an empty `helper_path` is not by itself a "helper
  disabled" switch in the C. `find_helper` (`privilege_helper.c`) searches six
  hardcoded paths, one of which is `/usr/lib/rustdesk/drmtap-helper`, the
  directory this package installs into, and `fork`/`exec`s the first executable
  it finds if the direct export ever returns `EACCES`/`EPERM`. Here that path is
  unreachable for two independent reasons: the root service holds
  `CAP_SYS_ADMIN` so the direct export succeeds, and the package builds only the
  shared library, so no helper exists at any of those paths. They are all
  root-writable-only, so a helper appearing there would not be an escalation
  either, but the honest statement is "a privileged child is spawned only if a
  helper binary exists at one of those fixed root-owned paths, and this package
  never installs one", not "never".
- The `_drm` socket lives beside the hardened `_service` socket
  (`/tmp/<app>-service/ipc_drm`). It is `0666` so the unprivileged `--server`
  can connect, but every accepted peer is authorized in `handle_drm_conn`
  (`authorize_service_scoped_ipc_connection`: peer must be root or the active
  session uid, with a `/proc/<pid>/exe` identity match). Connectable is not
  authorized.

## Threat model

- **Consent bypass.** This mode does not show the portal "select what to share"
  prompt. On a misconfigured install it could expose the login screen, the lock
  screen, or another local user's graphical session.
- **The scanout parse runs in the root service.** Moving the read in-process
  removes the old `setcap` helper and its world-exec attack surface. On the
  **default (split) path** the root service does only a **metadata-only** parse
  of the scanout descriptor and exports the DMA-BUF fd; the untrusted-framebuffer
  detile / pixel-format conversion runs in the **unprivileged `--server`**,
  outside `CAP_SYS_ADMIN`. Export-side validation is therefore metadata-only —
  geometry bounded to `<= MAX_DIM` (16384) and `num_planes` in `1..=4`
  (`drm_reader.rs` `grab_desc`); there is **no fourcc gate** on the export side,
  because the format check is delegated to the unprivileged converter, which
  handles every format `libdrmtap` supports (XRGB/ARGB8888, 10-bit XR30/AR30,
  HDR, CCS-compressed). The exported fd is **read-only**: `libdrmtap` exports the
  DMA-BUF via `drmPrimeHandleToFD` with `DRM_RDWR` dropped (`O_RDONLY`), and
  `drm_reader` `dup()`s it — which shares the same open file description and so
  preserves that access mode — so the unprivileged consumer can map the scanout
  for reading but never write into the live framebuffer. On the **CPU fallback
  path** the pixel-format conversion / detile instead runs inside the
  `CAP_SYS_ADMIN` service without a seccomp cage; there the frame copy has
  format / stride / geometry and integer-overflow guards (`drm_reader.rs`
  `grab`), and non-32bpp scanouts are rejected before the copy. The device is
  realpath-gated to `/dev/dri/` on both paths.
- **`_drm` is a screen-content channel.** It is authorized per connection (see
  above); without that authz any local process could read the screen. Authorization
  is also **re-checked on every frame**, not only at accept, because DRM/KMS
  capture is not session-scoped: it grabs the physical scanout of a CRTC no matter
  which session owns the display. So when the active session changes -- a user
  logging in at a greeter -- the greeter's `_drm` stream is CLOSED rather than
  continued (`drm: _drm peer no longer matches the active session`; observed with
  peer_uid=60578 against active_uid=1000, and the greeter's uinput channel goes
  with it). That is what stops an outgoing greeter process from capturing the
  logged-in user's screen. The cost is a reconnect, not the session: the client
  re-establishes itself against the new session's `--server` on its own in about
  2.5 s (~3.6 s of dark screen, measured 2026-07-31). On the
  **default (split) path** the channel carries the scanout DMA-BUF fd, passed to
  the unprivileged `--server` over `SCM_RIGHTS` as a **read-only** descriptor
  (the `--server` holds an import-once EGLImage cache, so a given scanout buffer
  is imported once and re-imports are elided); the peer can map the scanout for
  reading but cannot write it. The **CPU fallback path** instead carries plain
  packed-BGRA bytes over the same authorized socket (no fd passing, no shared
  memory).
- **When the CPU fallback is chosen.** The split path is the default; the
  consumer asks the service for the CPU-converted frame in two cases: no render
  node can be opened for this seat, or a previous convert on this display
  already failed. A third case is a **multi-GPU safety fallback**: if
  the service could not name the render node of the GPU that exports the scanout
  (an older `libdrmtap` without `drmtap_render_node`) and the host has more than
  one render node, the consumer refuses to guess one, because importing a scanout
  on a device that did not export it can succeed and return corrupted pixels
  rather than fail. The conversion then happens in the service, on the device it
  already has open, so it is correct by construction. Hosts with a single render
  node have nothing to pick wrong and keep the DMA-BUF fast path.
- **The display wake injects synthetic input from the root service.** It is
  compiled in only with the `drm-wake` feature, which `build.py --drm` adds on
  top of `drm`, and it can be switched off at runtime with
  `enable-drm-display-wake=N`. Building with `--features drm` alone leaves no
  wake code in the binary at all, so an operator auditing the deb can answer
  "is the injection path even present here?" from the artifact. A
  compositor that idles long enough DISABLES a connector, leaving no scanout for
  any backend, so on a `_drm` handshake that finds a CONNECTED display with no
  CRTC the service emits one synthetic pointer round trip over `/dev/uinput` to
  make the compositor re-enable it. The virtual device **declares** two relative
  axes and `BTN_LEFT`, because libinput classifies a device before it will treat
  its events as pointer activity at all and a single axis with no buttons is
  ignored outright (measured three ways on the same idle machine). What it
  actually **emits** is `+1` then `-1` on one axis: net-zero displacement, no
  button press, no key events. This is deliberate input injection by privileged
  code, so its bounds are worth stating precisely:
  - it can only be reached through an **already-authorized** `_drm` connection
    (same per-connection authz as every other use of the channel), so it grants
    nothing to a local attacker that the channel itself does not;
  - it runs in the root service because that is the only place it can:
    `/dev/uinput` is root-only here, and a modeset of our own is not an option
    since the compositor holds DRM master (the sysfs `dpms` attribute is
    read-only). Session-bus routes (`org.gnome.ScreenSaver`) authenticate by
    uid, refuse root, and are desktop-specific;
  - the trigger is narrow — a connected-but-undriven connector, not "no
    frames" — and connectors a wake demonstrably cannot bring back are
    remembered by connector identity and stop triggering. That memory is
    per-connector rather than global, so a permanently dark connector cannot
    suppress the wake for a different panel, and it drops any entry later seen
    scanning out. Note what that recovery rule does and does not give you: it
    clears the moment the display is driven **by anything**, but nothing else
    retries, so a connector latched after a wake that failed for a transient
    reason stays latched until that display comes back some other way — on an
    unattended host, typically not until the service restarts. It is a
    deliberate trade against waking on every connection forever for a display
    that is never coming;
  - it is rate limited to **one wake per 20 s process-wide** with exactly one
    concurrent winner (compare-exchange claim), so a reconnect storm cannot
    become an input-injection storm. That bounds the injection RATE. It does
    not bound how long a screen stays lit, and neither does the one-shot
    property below: 20 s is shorter than every idle period measured below, so a
    remote peer that reconnects in a loop can have the panel relit after each
    idle-off. What that peer gains is a lit panel on a machine whose screen it
    is already authorized to watch: it is visible to someone standing there,
    not additional access;
  - the wake is **one-shot: it resets the compositor's idle timer, it does not
    hold the display on**. If nothing else keeps the session awake, the connector
    idles off again one full idle period later -- measured 2026-07-31: 30.3 s at
    a GDM greeter, 70.3 s in a user session with `idle-delay=60`. Keeping a
    screen lit for the length of a session is the job of RustDesk's existing
    keep-awake inhibitor, not of this wake, which only recovers a connector that
    is *already* dark;
  - the uinput device is created and destroyed around the emit — nothing
    persists in the input stack between wakes;
  - without `/dev/uinput` the wake is skipped and latched off. Such a session
    was already view-only (input injection on Wayland needs uinput too), so
    this adds no new failure mode.

## Deployment

- **Off by default.** The `drm` feature is **not** in the default feature set and
  is **not** enabled in standard release packages; the drm-off build is
  byte-identical to upstream. Build it explicitly with
  `python3 build.py --flutter --drm` (Linux only).
- **Separate opt-in package.** A `--drm` build ships as a distinctly named
  `rustdesk-unattended-wayland` package (Conflicts/Replaces/**Provides** `rustdesk` --
  `Provides` is what lets a third-party package that depends on `rustdesk` be satisfied by the
  consent-free variant, so it belongs in an audit of this metadata), so
  enabling consent-free capture is an explicit install choice.
- **Bundled library, no capabilities.** The package installs the versioned
  `libdrmtap.so.0.<minor>.<patch>` plus a `libdrmtap.so.0` soname symlink under
  `/usr/lib/rustdesk/`, and the in-process `dlopen` names that absolute path
  (`/usr/lib/rustdesk/libdrmtap.so.0`). The package deliberately does **not**
  register the directory with the dynamic linker: no
  `/etc/ld.so.conf.d/` drop-in and no `ldconfig` trigger are shipped, so a
  private library cannot shadow a system one for unrelated binaries
  (Debian Policy 10.2). The bare-soname lookups remain only as a fallback for a
  development build reached through `LD_LIBRARY_PATH`.

  There is no `setcap`, no `rustdesk-capture` group, and no privileged binary:
  the capture runs inside the root `--service`, which already holds the
  capability it needs. Hosts without `/dev/dri` access (or where the library
  fails to load) transparently fall back to the PipeWire/portal path.
- **Minimum libdrm: 2.4.95.** `libdrmtap` needs the DRM `GetFB2` framebuffer API, which
  landed in libdrm 2.4.95. Ubuntu 18.04 is the oldest distribution worth naming here, and it
  straddles the floor: base bionic shipped 2.4.91, below it, while the updates/HWE stack
  (2.4.101) is above — so read this as "18.04 with updates, or anything newer", not as
  "any 18.04". That is an API statement, not a binary-compatibility one:
  the `rustdesk-unattended-wayland` deb in this repo's CI is built on an ubuntu-24.04 runner, so the
  shipped binaries carry that build host's glibc floor. Running on an older distribution means
  building the deb there (or in a matching container), which the libdrm floor above permits.
  Capture also requires an active KMS scanout (a Wayland/KMS session with a display
  on); on hosts where the compositor drives the display outside DRM/KMS (e.g. the proprietary NVIDIA
  X11 stack) there is no capturable CRTC and the path falls back to PipeWire/portal.
- **Recommended for** single-user, physically-controlled, or unattended hosts.

## Auditing

```bash
# the bundled capture library and its soname symlink — no capabilities are set on either
ls -l /usr/lib/rustdesk/libdrmtap.so.0*
# the dlopen names the symlink by absolute path, so what matters is where the symlink points:
readlink /usr/lib/rustdesk/libdrmtap.so.0   # expect: the versioned object shipped by the package
# and there should be no other object left beside it (a leftover is not loaded on its own, but it
# is what a stray ldconfig over this directory would repoint the symlink to):
ls /usr/lib/rustdesk/libdrmtap.so.0.*       # expect: exactly one versioned object
ls /etc/ld.so.conf.d/ | grep -i rustdesk               # expect: no output (none is shipped)
# confirm no privileged helper is present (there should be none)
getcap -r /usr/lib/rustdesk 2>/dev/null                  # expect: no output
```
