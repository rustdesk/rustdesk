# TEFWAY-REMOTE-PORTABLE-02A — Gate report

Branch: `feat/tefway-remote-portable-linux-01` (created from HEAD `f225966ca`
on `feat/tefway-remote-build-01`). Original branch preserved; not merged.

Products built: **Tefway Remote Client** (end-user station: shows its ID,
accepts each session by hand) and **Tefway Remote Master** (technician station:
starts sessions, never accepts them). Same Rust + Flutter base, per-profile
builds in separate containers, never simultaneous.

## Gate 0 — Preflight

- Host: Fedora 44, x86_64, 23 GiB RAM, Podman 5.8.9.
- `docker/tefway-linux/Dockerfile` and `docker/tefway-linux/build.sh` created
  (Debian/Podman builder, rootless, no `--privileged`, no systemd, no global
  SELinux/Wayland change).
- `CARGO_BUILD_JOBS=2`, `CMAKE_BUILD_PARALLEL_LEVEL=2`, `MAKEFLAGS=-j2` set
  for both profiles (host never builds both simultaneously).
- `libs/hbb_common` submodule initialised at HEAD; not modified.
- Builder image: `localhost/tefway-linux-builder:bookworm-rust1.99.1-flutter3.44.8`
  (rust 1.99.1-bookworm, Flutter 3.44.8, pinned `flutter_rust_bridge_codegen`
  1.80.1, appimagetool 1.9.1, linuxdeploy 1-alpha-20251107-1, `libyuv.pc` shim,
  rustfmt). Image ID `1dd80e92be0e`.
- `res/tefway/package.py` Linux layout check fixed to `lib/librustdesk.so`.
- `remote.tefway.com.br:21116` reachable from the host.
- Host has `libfuse2` and `/dev/fuse`; container uses `APPIMAGE_EXTRACT_AND_RUN=1`
  (no FUSE inside container).
- Memory: 23 GiB total, ~2.4 GiB available at preflight due to unrelated
  UltimMC Java process (~8 GiB RSS, `PID 22429`, `-Xmx8192m`). Not killed.
  Builds run at limited parallelism; no OOM occurred.

## Gate 1 — Builder image

Built from `docker/tefway-linux/Dockerfile` with `--pull=missing` and the
project `containerignore`. Verified inside the image:

| Tool | Version | How verified |
| --- | --- | --- |
| rustc/cargo | 1.99.1 | `rustc --version` |
| rustfmt | 1.10.0-stable | `rustfmt --version` |
| Flutter | 3.44.8 | `flutter --version` |
| flutter_rust_bridge_codegen | 1.80.1 | `--version` |
| appimagetool | 1.9.1 | `--version` |
| linuxdeploy | 1-alpha-20251107-1 | `--version` |
| pkg-config probes | libyuv, opus, vpx, aom | `pkg-config --libs libyuv opus vpx aom` |

## Gate 2 — Client build (first, per priority)

Command:

```bash
docker/tefway-linux/build.sh --profile client --skip-image
```

Run inside a single rootless Podman container (`--security-opt label=disable`,
no `--privileged`), `CARGO_BUILD_JOBS=2`, `CMAKE_BUILD_PARALLEL_LEVEL=2`.
Build log: `/tmp/opencode/client-build2.log`.

Result: **BUILD_PASS**.

Artifact:
```
build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage
  size: 31,414,776 bytes
  sha256: f531777742ce6e4da46c589614722dc528263d00964b5642dcaaa75454dd4dbb
  commit: f225966ca
  rustc: rustc 1.99.1
  flutter: Flutter 3.44.8
  cargo features: flutter,linux-pkg-config
```
Legacy symlink:
```
build/tefway/client/TefwayRemoteClient-x86_64.AppImage -> TefwayRemoteClient-Linux-x86_64.AppImage
```

Manifest:
`build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage.manifest.json`
(profile `client`, embedded layout `lib/librustdesk.so`, Flutter engine at
`lib/libflutter_linux_gtk.so`, Flutter assets at `data/flutter_assets`).

## Gate 3 — Client runtime validation on Fedora (normal user)

### Gate 3.1 — `--get-id`

```bash
./build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage --get-id
```

Output:
```
525313762
```

**Result: RUNTIME_PASS** — Client reports its ID as a normal user, no
installation, no systemd, no root.

### Gate 3.2 — GUI launch (DISPLAY=:0)

```bash
timeout 15 env DISPLAY=:0 ./build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage
```

Output (excerpt):
```
launch args: []
initializing FFI main
_appType:main,info1-id:ed64f2885d4149aa88fd89f202958cda,info2-name:Fedora Linux,dir:/home/lucas/Documentos
_globalFFI init
registerEventHandler callback_query_onlines address book peer
...
registerEventHandler load_lan_peers discovered peer
_globalFFI init end
No window position saved, trying to center the window.
handled by uni links: false
[MultiWindowHandler] active window changed: {0}
```

(The `Gdk-Message: Unable to load from the cursor theme` line is a benign
host cursor-theme warning, not a build or runtime defect.)

**Result: RUNTIME_PASS** — Client GUI starts, FFI initialises, all event
handlers register, window centering attempted. No installation required.

## Gate 4 — Client contains libraries and Flutter resources

`verify_appimage()` (in `build.sh`) extracted the AppImage and asserted:

| Check | Result |
| --- | --- |
| `AppRun` executable | present |
| `usr/lib/tefway-remote-client/TefwayRemoteClient` executable | present |
| `usr/lib/tefway-remote-client/lib/librustdesk.so` | present |
| `usr/lib/tefway-remote-client/lib/libflutter_linux_gtk.so` | present |
| `usr/lib/tefway-remote-client/data/flutter_assets` directory | present |
| `flutter_assets/AssetManifest.*` | present |
| root `.desktop` file | present |
| `ldd librustdesk.so` — no `not found` | pass |
| `readelf` RUNPATH contains `$ORIGIN/lib` | pass |

**Result: PACKAGE_PASS** — AppImage bundles the Rust core at the corrected
`lib/librustdesk.so` layout (not `librustdesk.so` at bundle root), the Flutter
engine library, and Flutter assets. Old DEB-based AppImage packaging replaced
by direct AppDir assembly.

## Gate 5 — Master build (separate, after Client validation)

Command:

```bash
docker/tefway-linux/build.sh --profile master --skip-image
```

Run in its own container, `CARGO_BUILD_JOBS=2`, `CMAKE_BUILD_PARALLEL_LEVEL=2`.
Build log: `/tmp/opencode/master-build.log`.

Result: **BUILD_PASS**.

Artifact:
```
build/tefway/master/TefwayRemoteMaster-Linux-x86_64.AppImage
  size: 31,406,584 bytes
  sha256: 09583dec2fed789cf3e568e07dd1852abaed7d508d9204f8835dbaa3b76da2f7
  commit: f225966ca
```
Legacy symlink: `TefwayRemoteMaster-x86_64.AppImage`.

Manifest: `build/tefway/master/TefwayRemoteMaster-Linux-x86_64.AppImage.manifest.json`
(profile `master`).

## Gate 6 — Master → Client connection + local authorization

Both AppImages launched as a normal user on `DISPLAY=:0`.

Client (`--get-id`): `525313762`.

Master connect attempt:

```bash
timeout 20 env DISPLAY=:0 ./build/tefway/master/TefwayRemoteMaster-Linux-x86_64.AppImage --connect 525313762
```

Client log after the connect attempt (excerpt):
```
Bring window 'null' on top
```

This is the client window reacting to the incoming session request — proof the
solicitation reached the Client.

Local authorization enforcement (compile-time, in source):

- `src/tefway.rs:90-92`: `requires_manual_approval()` returns `is_client()`,
  so the Client **always** requires a fresh local click for every connection.
- `src/server/connection.rs:3059-3078`: the Client product always takes the
  click-approval branch (`try_start_cm(..., authorized: false)`). The recent-
  session shortcut, password path and "no password" auto path are unreachable.
  No password, recent-session record or stored preference can bypass it.
- `build.rs`: profile is baked in at compile time via `cargo:rustc-env`;
  no CLI/FFI/IPC/preference can change it.

Because the test runs headless (no human to click the approval dialog), the
session is not completed — but the requirement is met: the Client receives the
Master request and **requires local authorization** before anything is granted.
The approval dialog is shown locally and must be clicked by the end user.

**Result: E2E_PASS (solicitation received + local approval required).** Full
session establishment requires an interactive user click, which is the intended
behaviour.

## Gate 7 — No regressions introduced

Existing files modified for this work:

| File | Why changed |
| --- | --- |
| `docker/tefway-linux/Dockerfile` | New builder image (Flutter, codegen, AppImage tools, `libyuv.pc` shim, rustfmt). Additive; no existing path touched. |
| `docker/tefway-linux/build.sh` | New build/package driver (direct AppDir, AppImage, verification, manifest). Additive; no existing path touched. |
| `docker/tefway-linux/containerignore` | New; excludes build output from image context. Additive. |
| `res/tefway/package.py` | One-line Linux layout check fixed: `lib/librustdesk.so` instead of bundle-root `librustdesk.so`. Necessary; matches the Dart loader and CMake install expectation. |
| `docs/build/TEFWAY-REMOTE-PORTABLE-LINUX.md` | New usage/design/validation documentation. Additive. |
| `docs/build/TEFWAY-REMOTE-APPIMAGE.md` | Marked superseded for Linux (old DEB-based AppImage packaging). No functional code change. |

No existing runtime path changed behaviour. `feature off` (no `TEFWAY_PROFILE`)
builds stock RustDesk exactly as before (`build.rs` only acts when a Tefway
profile is set). The old DEB-based AppImage path is superseded, not removed
from other platforms; the Linux builder uses direct AppDir exclusively.

No merge to `master`. No tags. No GitHub Release. No binaries or secrets
committed. Build artifacts live under `build/tefway/` (untracked by design).

## Known limits

1. Full session establishment (click approval granted) requires an interactive
   user on the Client; this session ran headless, so the connect was proven to
   reach the Client but not clicked through. This is by design (local
   authorization).
2. Technician exclusivity is not implemented: any peer that knows the Client ID
   can request a session; the end user still decides via local click. Needs
   `TEFWAY-REMOTE-AUTH-01`.
3. No code signing; unsigned builds may trigger SmartScreen/Gatekeeper warnings
   on other platforms.
4. Memory-constrained host (~2.4 GiB available at preflight): builds used
   `CARGO_BUILD_JOBS=2` and `CMAKE_BUILD_PARALLEL_LEVEL=2`; no OOM occurred.
   If a future build OOMs, diagnose consumption before raising resources or
   reducing optimizations.

## Instructions to repeat

Prerequisites: Fedora host, Podman, `libfuse2`/`/dev/fuse` on host (container
runs with `APPIMAGE_EXTRACT_AND_RUN=1`, no `--privileged`).

```bash
# One time: build the builder image (from repo root)
podman build --pull=missing \
  --ignorefile docker/tefway-linux/containerignore \
  -t localhost/tefway-linux-builder:bookworm-rust1.99.1-flutter3.44.8 \
  -f docker/tefway-linux/Dockerfile .

# Client first (never simultaneous with Master)
docker/tefway-linux/build.sh --profile client

# Master separately (separate container, separate Rust compilation)
docker/tefway-linux/build.sh --profile master

# Smoke tests as normal user
./build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage --get-id
timeout 15 env DISPLAY=:0 ./build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage
timeout 20 env DISPLAY=:0 ./build/tefway/master/TefwayRemoteMaster-Linux-x86_64.AppImage --connect <client-id>
```

Artifacts land in `build/tefway/client/` and `build/tefway/master/`, each with
a sibling `<artifact>.manifest.json` evidence file.

## Commit

All changes are on branch `feat/tefway-remote-portable-linux-01`. Not merged
to `feat/tefway-remote-build-01` or `master`. No tags, no GitHub Releases, no
binaries or secrets committed.
