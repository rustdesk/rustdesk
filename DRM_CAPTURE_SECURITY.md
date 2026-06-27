# DRM/KMS capture — security model & threat model

The optional `drm` feature adds a Linux capture backend that reads the active
scanout directly from DRM/KMS, **bypassing the xdg-desktop-portal consent
dialog**. It exists for unattended / login-screen / Wayland scenarios where the
portal prompt is not acceptable. Because it bypasses consent, treat it as a
**privileged, opt-in host-mode feature**, not a normal Wayland capture backend.

## How it works

Capture needs DRM master / `CAP_SYS_ADMIN` to read other clients' framebuffers.
Rather than run RustDesk as root, the `drm` feature ships a small privileged
helper, **`drmtap-helper`** (from
[`libdrmtap-sys`](https://crates.io/crates/libdrmtap-sys)), which carries
`cap_sys_admin+ep` via file capabilities and talks to the unprivileged RustDesk
process over a `socketpair`, passing the scanout back as a DMA-BUF fd.

## Threat model

- **Consent bypass.** This mode does not show the portal "select what to share"
  prompt. On a misconfigured install it could expose the login screen, the lock
  screen, or another local user's graphical session.
- **The TCB is `drmtap-helper` / `libdrmtap-sys`.** The privileged behaviour
  (caller authentication, IPC parsing, seccomp, DRM access, framebuffer bounds
  checks) lives there and has been security-reviewed. It is hardened:
  - validates it was spawned by the library (inherited socket on fd 3) **and**
    checks the peer UID via `SO_PEERCRED`;
  - restricts the device it opens to a realpath under `/dev/dri/`, opened
    `O_RDONLY`;
  - `PR_SET_NO_NEW_PRIVS`, drops all capabilities except `CAP_SYS_ADMIN`, then
    installs a **default-KILL seccomp allowlist** that deliberately forbids
    `open`/`openat` (the device is opened once before the filter loads), so a
    compromised helper cannot open arbitrary files even with `CAP_SYS_ADMIN`;
  - built with stack-protector-strong, FORTIFY, PIE and full RELRO;
  - integer-overflow / DoS size guards on the framebuffer geometry.
- **`CAP_SYS_ADMIN` is broad.** The helper is deliberately tiny and confined as
  above, but any unfound bug in it is a local-privilege concern — hence the
  access-controlled install below.

## Deployment

- **Off by default.** The `drm` feature is **not** in the default feature set and
  is **not** enabled in standard release packages. Build it explicitly with
  `python3 build.py --flutter --drm` (Linux only).
- **Not world-executable.** The `.deb` postinst installs the helper as:

  ```bash
  groupadd -r rustdesk-capture
  chown root:rustdesk-capture /usr/lib/rustdesk/drmtap-helper
  chmod 0750 /usr/lib/rustdesk/drmtap-helper
  setcap cap_sys_admin+ep /usr/lib/rustdesk/drmtap-helper
  ```

  Only members of `rustdesk-capture` can run it; an administrator opts users in
  with `usermod -aG rustdesk-capture <user>`. Everyone else (and every host
  where the group is empty) transparently falls back to the PipeWire/portal
  path. There is no window where the binary is both `0755` and capability-bearing
  (mode/owner are set before `setcap`).
- **Recommended for** single-user, physically-controlled, or unattended hosts.
  On shared/multi-user hosts, only add trusted operators to `rustdesk-capture`;
  the group is the access-control boundary.
- The unattended-input path (`--uinput-service`) is a separate root component
  with the same multi-user considerations; document who can reach it on shared
  hosts.

## Auditing

```bash
getcap /usr/lib/rustdesk/drmtap-helper      # expect: cap_sys_admin=ep
getfacl /usr/lib/rustdesk/drmtap-helper     # expect: root:rustdesk-capture 0750
setcap -r /usr/lib/rustdesk/drmtap-helper   # revoke the capability
```
