# Tefway Remote — Linux AppImage x86_64 (NOT VERIFIED)

Status: **NOT VERIFIED** — this document describes the AppImage packaging
procedure, but the AppImages could not be built or executed because the BUILD-01
host is Windows with no Linux environment. See
`docs/gates/TEFWAY-REMOTE-BUILD-01.md`, Gates 8 and 9.

This document becomes `VERIFIED` only after both AppImages are produced, run on
a clean compatible Linux machine, and the Master → Client consent flow is
executed.

## Products

- `TefwayRemoteMaster-x86_64.AppImage`
- `TefwayRemoteClient-x86_64.AppImage`

## Build command

```powershell
python res\tefway\package.py --profile master --format appimage --build
python res\tefway\package.py --profile client --format appimage --build
```

The driver first builds the Debian DEB (same `--profile`), renders
`res/tefway/pkg/AppImageBuilder.yml.in` (referencing the DEB path, `@PKG@`,
`@NAME@`, `@BIN@`, `@VERSION@`), and runs `appimage-builder --recipe`.
The produced AppImage is renamed to
`<exe>-x86_64.AppImage` under `build/tefway/<profile>/` with a sibling
`<artifact>.manifest.json` evidence file.

## Packaging notes

- AppImage does **not** eliminate graphical-environment dependencies or
  capture/authorization requirements; validate on a real desktop (X11/Wayland)
  with the needed runtime libraries present.
- Bundle the libraries the Rust/Flutter runtime actually loads; do not assume a
  minimal host.
- No service is installed by the AppImage; it is a portable, per-user artifact.
- Verify the embedded profile via the sibling `.manifest.json` (profile field
  must be `master` or `client`).

## Verification checklist (clean compatible Linux host)

- [ ] `appimage-builder` produces both AppImages with no errors.
- [ ] `chmod +x` and execute each AppImage; confirm launch (`RUNTIME_PASS`).
- [ ] Confirm icons render (light and dark theme), no missing assets.
- [ ] Confirm the Rust library loads (session/connection UI works).
- [ ] Confirm connection to `remote.tefway.com.br` (no public RustDesk fallback).
- [ ] Master → Client consent flow executed (accept / decline / reconnect /
  Master refuses incoming).
- [ ] Confirm the AppImage embeds the expected profile (`master`/`client`) via
  `*.manifest.json`.
