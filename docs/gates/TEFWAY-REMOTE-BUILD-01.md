# TEFWAY-REMOTE-BUILD-01 — Gate report

Branch: `feat/tefway-remote-build-01` (created from reference commit
`8355cce9625d4d33349cc6f828c341cab9dca37a` on `master`).
Working tree at start: clean, no local changes. Nothing was reset, cleaned or
discarded. Submodule `libs/hbb_common` already initialised; not modified.

Products: **Tefway Remote Master** (technician station: starts sessions, never
accepts them) and **Tefway Remote Client** (end-user station: shows its ID,
accepts each session by hand). Same Rust + Flutter base, no duplicated project,
per-file build manifests so a Master binary can never be packaged as Client.

## Tooling used (this host)

| Tool | Version | Source |
| --- | --- | --- |
| Rust / cargo | 1.99.1 | rustup, `x86_64-pc-windows-msvc` |
| Flutter | 3.44.8 | stable channel, local install |
| `flutter_rust_bridge_codegen` | 1.80.1 | pinned by CI (`bridge.yml`), matches `flutter/pubspec.yaml` |
| Python | 3.11.9 | `python` on PATH |
| vcpkg | 2026-05-27 | `C:\vcpkg`, baseline `9e593bb18ea69cc5095e012465dcd675a822ed0d` (`vcpkg.json`) |
| Visual Studio | Build Tools 2022 17.14.8 | C++ workload |

## Files modified

| File | Change |
| --- | --- |
| `build.rs` | `TEFWAY_PROFILE` handling hardened: any value other than `master`/`client` (when set) now **aborts the build** instead of silently shipping stock RustDesk. An unset profile still builds stock RustDesk for ordinary development (upstream behaviour untouched). When a Tefway profile is selected, `target/release/tefway-build-manifest.json` is written: profile, version, source commit, OS, architecture, compiled resources (cargo features) and the pinned Tefway server identity. No secrets. |
| `src/tefway.rs` | Added `use hbb_common::log;` so `log::info!` in `server_options()` resolves (was an unresolved module under the `flutter` feature). No behaviour change. |
| `res/tefway/package.py` | Reworked for Gate 1 profile isolation: (a) cross-checks the profile embedded in `tefway-build-manifest.json` against the requested profile before packaging; (b) rejects any pre-existing bundle that carries no matching profile marker, so `--skip-cargo` or packaging without `--build` cannot reuse a bundle built for another product or no product; (c) writes a sibling `<artifact>.manifest.json` per artifact with profile, version, source commit, OS, arch, resources, server identity and SHA-256 of the package (no secrets); (d) renames `rustdesk`/`rustdesk.exe` to `TefwayRemoteMaster`/`TefwayRemoteClient` only after validating the bundle still contains the runtime files it needs (exe, `librustdesk.dll`, `flutter_assets`); (e) invokes `flutter` through `cmd /c` on Windows (flutter is a `.bat`). |
| `src/tefway.rs` (existing, from CLIENT-02) | Profile constants (`Tefway Remote Master/Client`, `refuses_incoming`, `requires_manual_approval`, `is_incoming_only`, `server_options` forcing rendezvous/relay/key/API-server). Unchanged here. |
| `build.rs`/`src/tefway.rs` integration (existing, from CLIENT-02) | `src/server/connection.rs` refuses Master logins and forces Client click approval; `src/flutter_ffi.rs` reports the baked-in profile; `src/client.rs` uses `tefway::is_incoming_only()`. Unchanged here. |

## Gate 0 — Preflight

All preflight items verified at the reference commit:

- Git state clean, HEAD `8355cce9` = reference commit.
- `Cargo.toml` / `Cargo.lock` present; `flutter_rust_bridge` pinned to `1.80.1`.
- `libs/hbb_common` submodule initialised at `fae47e02`, not modified.
- `res/tefway/` assets present: icons, `tefway.ico`, `package.py`, `pkg/` templates.
- vcpkg at `C:\vcpkg`; codec ports (`opus`, `libyuv`, `libvpx`, `aom`) installed for
  `x64-windows-static` (needed by `magnum-opus`/`scrap` on Windows).
- Generated bridge (`src/bridge_generated*.rs`, `flutter/lib/generated_bridge.dart`)
  is gitignored; regenerated locally with the pinned codegen 1.80.1 (not committed).

## Gate 1 — Build-system profile isolation (implemented and verified in code)

- Build Master → profile `master`; build Client → profile `client`.
- Unknown `TEFWAY_PROFILE` aborts the build (panic in `build.rs`).
- Unset profile → stock RustDesk, never packaged as Tefway by `package.py`.
- Packager validates the profile embedded in the binary manifest (item 6).
- `--skip-cargo` / packaging without `--build` rejected unless the bundle carries a
  matching profile marker (item 7).
- Per-artifact `<artifact>.manifest.json` evidence (item 8).
- Executable rename validated against required runtime files (item 10).

Verified by running `cargo build --locked --features flutter --lib --release` with
`TEFWAY_PROFILE=master`: build succeeded and
`target/release/tefway-build-manifest.json` contains `profile: master`,
`commit: 7e6de0a...`, `os: windows`, `arch: x86_64`, resources list and the
Tefway server identity.

## Gate 2 — Rust + Flutter (toolchain fixed)

- `cargo check --locked --features flutter --lib --release`: **passes** (exit 0).
- `cargo build --locked --features flutter --lib --release`: **passes**, produces
  `target/release/librustdesk.dll`.
- `cargo test --lib tefway`: **5/5 pass** (profile policy unit tests).
- vcpkg codec libraries installed: `opus.lib`, `libyuv.lib`, `libvpx.lib`,
  `aom.lib` under `C:\vcpkg\installed\x64-windows-static\lib`.
- Bridge regenerated locally with pinned codegen 1.80.1:
  `src/bridge_generated.rs`, `src/bridge_generated.io.rs`,
  `flutter/lib/generated_bridge.dart`, `flutter/lib/generated_bridge.freezed.dart`
  (all gitignored, not committed).
- libclang present at `C:\Program Files\LLVM\bin\clang.exe` (bindgen dependency).

## Gate 3 — First Windows build

**BLOCKED by environment, not by code.** `flutter build windows --release` fails
with:

> Building with plugins requires symlink support. Please enable Developer Mode in
> your system settings.

This host runs with reduced privileges: creating symlinks (including in `%TEMP%`)
fails with "a operação requer privilégio de administrador", and writing
`HKLM\...\AppModelUnlock\AllowDevelopmentWithoutDevLicense` is denied, so
Developer Mode cannot be enabled here. `flutter` is invoked correctly through
`cmd /c` (fixed in `package.py`); the failure is purely the missing symlink
privilege.

Consequence: neither `TefwayRemoteMaster.exe` nor `TefwayRemoteClient.exe` could
be produced on this host, and the Master → Client functional test (Gate 3
second half, Gate 4, Gate 5 runtime checks) could not be executed.

**To unblock:** run the packaging on a Windows machine with Administrator rights
or Developer Mode enabled, then:

```powershell
# Master (technician station)
python res\tefway\package.py --profile master --format windows --build

# Client (end-user station)
python res\tefway\package.py --profile client --format windows --build
```

Expected output:
```
build\tefway\master\TefwayRemoteMaster\TefwayRemoteMaster.exe
build\tefway\client\TefwayRemoteClient\TefwayRemoteClient.exe
```

Each directory is the complete portable package (exe + `data\` + DLLs + Flutter
assets). Not a single self-contained exe; do not ship the exe alone.

## Gate 4 — Tefway server (configured in code, not tested at runtime)

`src/tefway.rs` pins, via `server_options()` in the overwrite map (outranks user
config, cannot be edited in the UI):

| Setting | Value |
| --- | --- |
| Rendezvous | `remote.tefway.com.br` (port 21116) |
| Relay | `remote.tefway.com.br` (port 21117) |
| Public key | `3KBIfqldtiCmJkAl2O6rammBCLK4iRiTH3vQdRXoEEs=` |
| API server | empty |

No fallback to public RustDesk servers: `custom-rendezvous-server` is never empty.
The RustDesk server was not touched.

## Gate 5 — Security profile enforcement (code review, not runtime-tested)

Enforcement points verified in source:

- `src/tefway.rs`: `refuses_incoming()` (Master), `requires_manual_approval()`
  (Client), `is_incoming_only()` (Client), `server_options()` (locked servers,
  click approval, disabled trusted devices / permanent-password change).
- `src/server/connection.rs`: Master login refused before any scope check; Client
  short-circuited to the click-approval branch (recent-session, password and
  "no password" auto paths unreachable).
- `src/client.rs` / `src/flutter_ffi.rs`: report the baked-in profile; incoming-only
  enforced at two call sites.
- `src/core_main.rs`: calls `crate::tefway::apply()`.
- `build.rs`: profile baked in at compile time; no CLI/FFI/preference can change it.

**TEFWAY-REMOTE-AUTH-01 remains pending:** technician exclusivity is not
implemented. Any peer that knows the ID can request a session; nothing proves the
peer is a Tefway technician. Do not declare the product ready for public safe
distribution until this is addressed.

## Gates 6–8 — RPM / DEB / AppImage

Not produced: this host is Windows and has no Linux/Fedora/Debian environment.
The `package.py` drivers (`--format rpm|deb|appimage`) and templates in
`res/tefway/pkg/` are in place and follow the CLIENT-02 closeout fixes
(per-package conflicts, renamed `@BIN@` symlinks/units, no automatic systemd
enable/start for the Client). They must be validated on real Fedora/Debian hosts.

## Gate 9 — Evidence status matrix

| State | Meaning |
| --- | --- |
| BUILD_PASS | compilation concluded |
| PACKAGE_PASS | package generated and structurally validated |
| RUNTIME_PASS | application started |
| E2E_PASS | real remote session homologated |
| SECURITY_PASS | consent/security controls tested |

**Rust (Windows host): BUILD_PASS** (`cargo check` + `cargo build` release,
`flutter` feature). Tests: **5/5 tefway profile tests pass**.

**Windows EXE (Master/Client): NOT PRODUCED** — blocked by missing symlink/
Developer Mode privilege (see Gate 3). Consequently: no PACKAGE_PASS,
RUNTIME_PASS, E2E_PASS or SECURITY_PASS for Windows executables on this host.

**Fedora RPM / Debian DEB / Linux AppImage: NOT PRODUCED** — no Linux environment
available on this host. Docs are DRAFT / NOT VERIFIED.

## Known limits

1. Windows EXE build requires Administrator rights or Developer Mode (symlinks).
2. Technician exclusivity not implemented (`TEFWAY-REMOTE-AUTH-01` pending).
3. No code signing; unsigned builds raise SmartScreen warnings.
4. No MSI/installer branding pass.
5. No runtime functional test (Master → Client consent, reconnect, Master
   refuses incoming) executed in this session.

## Instructions to repeat the build

Prerequisites (Windows, Administrator or Developer Mode enabled):

- Rust 1.75+ (CI pin) with `x86_64-pc-windows-msvc`, Flutter 3.44.8.
- vcpkg baseline `9e593bb18ea69cc5095e012465dcd675a822ed0d`; install
  `opus`, `libyuv`, `libvpx`, `aom` for `x64-windows-static`.
- `C:\vcpkg` on `VCPKG_ROOT` (or `VCPKG_INSTALLED_ROOT`).

```powershell
# One time: generate the gitignored bridge with the pinned codegen
flutter_rust_bridge_codegen --rust-input ./src/flutter_ffi.rs `
  --dart-output ./flutter/lib/generated_bridge.dart `
  --c-output ./flutter/macos/Runner/bridge_generated.h

# Master
python res\tefway\package.py --profile master --format windows --build

# Client
python res\tefway\package.py --profile client --format windows --build
```

Artifacts land in `build\tefway\master\` and `build\tefway\client\`, each with a
sibling `<artifact>.manifest.json` evidence file.

## Commit

All changes are on branch `feat/tefway-remote-build-01` (commit
`7e6de0a5a` plus the work below). Not merged to `master`. No tags, no GitHub
Releases, no binaries or secrets committed.
