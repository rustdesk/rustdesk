# Tefway Remote — Fedora x86_64 RPM (NOT VERIFIED)

Status: **NOT VERIFIED** — this document describes the packaging procedure and
the mandatory spec corrections, but the RPMs could not be built or validated on a
real Fedora host because the BUILD-01 host is Windows with no Linux environment.
See `docs/gates/TEFWAY-REMOTE-BUILD-01.md`, Gates 6 and 9.

This document becomes `VERIFIED` only after the RPMs are built with `rpmbuild`,
installed and removed cleanly on a real Fedora machine (X11 and Wayland), and the
Master → Client consent flow is executed.

## Products

- `tefway-remote-master-<version>-1.x86_64.rpm`
- `tefway-remote-client-<version>-1.x86_64.rpm`

(`<version>` comes from `Cargo.toml`, currently `1.5.1`.)

## Build command

```powershell
python res\tefway\package.py --profile master --format rpm --build
python res\tefway\package.py --profile client --format rpm --build
```

The driver stages the per-profile bundle, renders
`res/tefway/pkg/tefway-remote.spec.in` (replacing `@PKG@`, `@NAME@`, `@BIN@`,
`@VERSION@`, `@DESCRIPTION@`) and runs `rpmbuild -bb` against a fresh
`_topdir` under `build/tefway/<profile>/rpm/`.

## Mandatory spec corrections (applied to `res/tefway/pkg/tefway-remote.spec.in`)

- No service is enabled or started automatically during packaging (`%post` does
  not call `systemctl enable`/`systemctl start`; no systemd activation for the
  Client at all — the Client must never offer unattended access).
- The Client installs no unattended-access service.
- The package does not remove or replace an existing RustDesk installation
  unexpectedly; it declares `Conflicts: rustdesk tefway-remote-master
  tefway-remote-client` so the two Tefway products and stock RustDesk cannot be
  installed together.
- No self-conflicts; `Obsoletes: rustdesk` kept only as the upstream transition
  hint (review before shipping).
- No orphaned files after uninstall (`%postun` removes `/usr/bin/@BIN@`,
  `/usr/share/applications/<name>.desktop`, and runs `update-desktop-database`).
- `.desktop` installed to `/usr/share/applications/`; icons to
  `/usr/share/icons/hicolor/<size>x<size>/apps/<name>.png` (16/32/48/64/128/256/512).
- Symlink `/usr/bin/<name> → /usr/share/<name>/<bin>` validated (target binary
  exists after the bundle rename `rustdesk → TefwayRemoteMaster/Client`).
- File permissions verified; SELinux context reviewed on the target host.

## Packaging notes

- Build in an isolated environment first (mock/container), not on the physical
  Tefway server and not on the Debian production server.
- The server identity (rendezvous/relay/key/API server empty) is baked into the
  Rust binary by `build.rs`/`src/tefway.rs`; the RPM carries no server config.

## Verification checklist (real Fedora host)

- [ ] `rpmbuild -bb` produces both RPMS with no errors/warnings.
- [ ] `rpm -qp --queryformat` shows correct name/version/arch.
- [ ] Install each RPM; files land under `/usr/share/<name>`, `/usr/bin/<name>`,
  icons and `.desktop` in place.
- [ ] Launch `tefway-remote-master` / `tefway-remote-client` (X11 and Wayland);
  document any real limitation.
- [ ] Uninstall; confirm no orphaned files and no leftover systemd units.
- [ ] `systemctl is-enabled <name>.service` is disabled/not found (Client has no
  service).
- [ ] SELinux: no denials on launch.
