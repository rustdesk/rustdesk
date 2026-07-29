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
or the consumer has no render node of its own, see *When the CPU fallback is
chosen* below) copies the scanout to packed BGRA inside the root service and
streams those bytes over `_drm`. This mirrors
the Windows `portable_service` split (a privileged process captures, an
unprivileged one presents) but reuses RustDesk's own hardened IPC.

- `libdrmtap.so` is loaded through a small `dlopen` loader (`drmtap_dl`); if the
  library or one of its runtime deps is missing the load fails cleanly and the
  caller falls back to the PipeWire/portal path.
- The loader also **refuses a library that cannot do the split**: one reporting
  below 0.4.10, and one reporting a newer version without actually exporting
  `drmtap_grab_desc` / `drmtap_open_render` / `drmtap_convert_dmabuf` (a stale or
  pre-release build). The only way to capture with such a library is the
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
  above); without that authz any local process could read the screen. On the
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

## Deployment

- **Off by default.** The `drm` feature is **not** in the default feature set and
  is **not** enabled in standard release packages; the drm-off build is
  byte-identical to upstream. Build it explicitly with
  `python3 build.py --flutter --drm` (Linux only).
- **Separate opt-in package.** A `--drm` build ships as a distinctly named
  `rustdesk-unattended-wayland` package (Conflicts/Replaces `rustdesk`), so
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
- **Minimum libdrm: 2.4.95 (Ubuntu 18.04 or equivalent).** `libdrmtap` needs the DRM
  `GetFB2` framebuffer API (libdrm 2.4.95); Ubuntu 18.04 ships 2.4.101, so every supported
  distribution satisfies the API floor. That is an API statement, not a binary-compatibility one:
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
# is what a stray ldconfig over this directory would repoint the symlink to)
ls /etc/ld.so.conf.d/ | grep -i rustdesk               # expect: no output (none is shipped)
# confirm no privileged helper is present (there should be none)
getcap -r /usr/lib/rustdesk 2>/dev/null                  # expect: no output
```
