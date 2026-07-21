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
its own unprivileged address space, and feeds the encoder — so the root service
never loads libEGL/libGLESv2 and never copies scanout pixels. Only the **CPU
fallback path** (used when the seat/driver cannot produce a transferable DMA-BUF,
or the loaded `libdrmtap` predates the split export) copies the scanout to packed
BGRA inside the root service and streams those bytes over `_drm`. This mirrors
the Windows `portable_service` split (a privileged process captures, an
unprivileged one presents) but reuses RustDesk's own hardened IPC.

- `libdrmtap.so` is loaded through a small `dlopen` loader (`drmtap_dl`); if the
  library or one of its runtime deps is missing the load fails cleanly and the
  caller falls back to the PipeWire/portal path.
- The reader restricts the device it opens to a realpath under `/dev/dri/`
  (`drm_reader.rs`); RustDesk always runs libdrmtap in direct in-process mode
  (`helper_path` is `NULL`), so no privileged child process is ever spawned and
  none is built, shipped, or installed. There is no `drmtap-helper` binary, no
  `setcap`, no capability-bearing file, and no capture group in this deployment.
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
  above); without that authz any local process could read the screen. On the
  **default (split) path** the channel carries the scanout DMA-BUF fd, passed to
  the unprivileged `--server` over `SCM_RIGHTS` as a **read-only** descriptor
  (the `--server` holds an import-once EGLImage cache, so a given scanout buffer
  is imported once and re-imports are elided); the peer can map the scanout for
  reading but cannot write it. The **CPU fallback path** instead carries plain
  packed-BGRA bytes over the same authorized socket (no fd passing, no shared
  memory).

## Deployment

- **Off by default.** The `drm` feature is **not** in the default feature set and
  is **not** enabled in standard release packages; the drm-off build is
  byte-identical to upstream. Build it explicitly with
  `python3 build.py --flutter --drm` (Linux only).
- **Separate opt-in package.** A `--drm` build ships as a distinctly named
  `rustdesk-unattended-wayland` package (Conflicts/Replaces `rustdesk`), so
  enabling consent-free capture is an explicit install choice.
- **Bundled library, no capabilities.** The package installs `libdrmtap.so.0`
  under `/usr/lib/rustdesk/` and registers that directory with the dynamic
  linker so the in-process `dlopen("libdrmtap.so.0")` resolves:

  ```bash
  # /etc/ld.so.conf.d/rustdesk-unattended-wayland.conf contains /usr/lib/rustdesk
  ldconfig
  ```

  There is no `setcap`, no `rustdesk-capture` group, and no privileged binary:
  the capture runs inside the root `--service`, which already holds the
  capability it needs. Hosts without `/dev/dri` access (or where the library
  fails to load) transparently fall back to the PipeWire/portal path.
- **Minimum OS: Ubuntu 18.04 (or equivalent, libdrm ≥ 2.4.95).** `libdrmtap` needs the DRM
  `GetFB2` framebuffer API (libdrm 2.4.95); Ubuntu 18.04 ships 2.4.101, so 18.04 is the floor. The
  `rustdesk-unattended-wayland` deb is built and packaged on an ubuntu18.04 container in CI (a
  build-time compatibility check only — DRM capture itself is not installed or exercised there), so
  it is built against the 18.04 toolchain and libraries and is compatible with 18.04 and newer.
  Capture also requires an active KMS scanout (a Wayland/KMS session with a display
  on); on hosts where the compositor drives the display outside DRM/KMS (e.g. the proprietary NVIDIA
  X11 stack) there is no capturable CRTC and the path falls back to PipeWire/portal.
- **Recommended for** single-user, physically-controlled, or unattended hosts.

## Auditing

```bash
# the bundled capture library — no capabilities are set on it
ls -l /usr/lib/rustdesk/libdrmtap.so.0
cat /etc/ld.so.conf.d/rustdesk-unattended-wayland.conf   # expect: /usr/lib/rustdesk
# confirm no privileged helper is present (there should be none)
getcap -r /usr/lib/rustdesk 2>/dev/null                  # expect: no output
```
