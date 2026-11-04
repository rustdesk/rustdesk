# Tefway Remote — Debian x86_64 DEB (NOT VERIFIED)

Status: **NOT VERIFIED** — this document describes the packaging procedure and the
mandatory control/structural checks, but the DEBs could not be built or validated
on a real Debian host because the BUILD-01 host is Windows with no Linux
environment. See `docs/gates/TEFWAY-REMOTE-BUILD-01.md`, Gates 7 and 9.

This document becomes `VERIFIED` only after the DEBs are built with `dpkg-deb`,
installed and removed cleanly on a real Debian machine, and the Master → Client
consent flow is executed.

## Products

- `tefway-remote-master_<version>_amd64.deb`
- `tefway-remote-client_<version>_amd64.deb`

(`<version>` comes from `Cargo.toml`, currently `1.5.1`.)

## Build command

```powershell
python res\tefway\package.py --profile master --format deb --build
python res\tefway\package.py --profile client --format deb --build
```

The driver stages the per-profile bundle into `build/tefway/<profile>/deb/root`,
renders `res/tefway/pkg/control.in` into `DEBIAN/control`, copies the
postinst/prerm/postrm scripts (mode 0755), installs icons into
`/usr/share/icons/hicolor/<size>x<size>/apps/`, creates `/usr/bin/<name>` as a
symlink to `/usr/share/<name>/<bin>`, and runs `dpkg-deb --root-owner-group -b`.

## Mandatory structural checks (`DEBIAN/control` and maintainer scripts)

- `Package:` is `tefway-remote-master` / `tefway-remote-client` (per profile).
- `Depends:` lists the real runtime dependencies
  (`libgtk-3-0`, `libxcb-randr0`, `libxdo3`, `libxfixes3`, `libxcb-shape0`,
  `libxcb-xfixes0`, `libasound2`, `libsystemd0`, `curl`, `libva2`, `libva-drm2`,
  `libva-x11-2`, `libgstreamer-plugins-base1.0-0`, `libpulse0`, `libdrm2`,
  `libayatana-appindicator3-1`).
- `Conflicts:` includes `rustdesk`, `tefway-remote-master`,
  `tefway-remote-client` so the two Tefway products and stock RustDesk cannot be
  installed together; no self-conflicts.
- No automatic service enable/start in maintainer scripts; the Client installs no
  unattended-access service.
- `Installed-Size` computed from the payload.
- `.desktop` and icons installed to the correct system locations.

## Verification checklist (real Debian host, isolated from the Tefway server)

- [ ] `dpkg-deb -b` builds both DEBs; `dpkg-deb -I` shows a sane `control`.
- [ ] `dpkg --install` each DEB succeeds; dependencies resolve.
- [ ] Launch both apps; confirm start and icons/menu entry.
- [ ] `dpkg --purge` each DEB; confirm no orphaned files under `/usr` and no
  leftover `/usr/bin/<name>` symlink.
- [ ] Confirm no systemd service enabled for either package (Client must have
  none).
- [ ] Master → Client consent flow executed (accept / decline / reconnect /
  Master refuses incoming).
- [ ] Rendezvous/relay/key point to `remote.tefway.com.br` / Tefway key only;
  no public RustDesk fallback.
