# Tefway Remote — Release handoff

State of the `feat/tefway-remote-client-02` branch after the
TEFWAY-REMOTE-CLIENT-02 gate. Read this before starting a build: it separates
what was verified from what was only written, and states what is still missing
before public distribution.

## What this branch delivers

1. **Branding** — the owner's `assets/logo.svg` wordmark is integrated in the
   Flutter UI (light and dark variants), and `assets/favicon.ico` is turned into
   the full Windows/Linux icon set.
2. **Two products from one tree** — `Tefway Remote Master` and
   `Tefway Remote Client`, selected at compile time by `TEFWAY_PROFILE` and
   baked into the binary. No runtime switch exists.
3. **Client consent** — every incoming session on the Client requires a local
   click, enforced in Rust on the login path, not in the Flutter UI.
4. **Tefway server** — rendezvous, relay and key are forced in code and cannot
   be edited by the user; no public-server fallback.
5. **Build system** — profile-aware build/packaging driver and per-distro setup
   documents for Windows, Fedora, Debian and AppImage.

## Enforcement map (Rust, not UI)

| Requirement | Mechanism | Location |
| --- | --- | --- |
| Profile cannot be changed at runtime | `TEFWAY_PROFILE` → `cargo:rustc-env` → `option_env!` | `build.rs`, `src/tefway.rs` |
| Client cannot start a session | incoming-only gate on `_start` and on the password path | `src/client.rs` (2 call sites now read `tefway::is_incoming_only()`) |
| Client UI has no "connect" affordance | `flutter_ffi::is_incoming_only` feeds the existing incoming-only UI | `src/flutter_ffi.rs` |
| Master cannot receive | login refused before scope check | `src/server/connection.rs` |
| Client must accept every session | click-approval branch short-circuited for the Client | `src/server/connection.rs` |
| Client cannot use unattended access | `verification-method`, `approve-mode`, `enable-trusted-devices`, `disable-change-permanent-password`, `allow-logon-screen-password` all forced | `src/tefway.rs` |
| Distribution servers fixed | `OVERWRITE_SETTINGS` outranks user config | `src/tefway.rs` |

Nothing in the Client can be toggled from Settings: the settings UI writes user
config, and every value above is read from a map that outranks it.

## Verification performed in this gate

Environment: Windows 11, rustc 1.99.1, Flutter 3.44.8 (local), Python 3.11.9.

| Check | Command | Result |
| --- | --- | --- |
| Rust formatting | `rustfmt --edition 2021 --check src/tefway.rs src/lib.rs src/core_main.rs src/client.rs src/server/connection.rs src/flutter_ffi.rs build.rs` | **PASS** (after formatting `src/tefway.rs`) |
| Rust compiler/type check | `cargo check --locked --lib` | **NOT RUN TO COMPLETION** — the host's vcpkg tree has only `x64-windows`; `magnum-opus` fails on `opus/opus_multistream.h`. This is an environment gap, not a code finding. |
| Rust symbol usage | grep against `libs/base/src/config/keys.rs` and `libs/hbb_common/src/config.rs` | **PASS** — every key and static referenced by `src/tefway.rs` exists and is public |
| Dart syntax | `dart format` parse of the three touched Dart files | **PASS** — parsed, no syntax errors (the two pre-existing files are not dart-format clean and were not reformatted) |
| Flutter analysis / build | `flutter analyze`, `flutter build` | **BLOCKED** — `flutter/lib/generated_bridge.dart` is git-ignored and absent; any Flutter step needs codegen first |
| Icon generation | `python res/tefway/gen_icons.py` | **PASS** — 16/32/48/64/128/256/512 PNG + 7-frame ICO; small sizes inspected visually, legible |
| SVG integration | `flutter_svg` parses the file's 21 `<path>` elements, no `<text>`/`<image>` | **PASS** by inspection |
| Asset integrity | `assets/logo.svg`, `assets/favicon.ico` untouched, valid | **PASS** |

Not executed at all: any Windows/Linux package build, any runtime session
between Master and Client, any consent flow on real devices. Nothing in this
branch has been run against `remote.tefway.com.br` yet.

## Artifact matrix

| Platform | Command | Artifact |
| --- | --- | --- |
| Windows | `python res\tefway\package.py --profile master --format windows --build --with-hwcodec` | `build\tefway\master\TefwayRemoteMaster\TefwayRemoteMaster.exe` (+ `data\`, DLLs) |
| Windows | same with `--profile client` | `build\tefway\client\TefwayRemoteClient\TefwayRemoteClient.exe` |
| Fedora | `python3 res/tefway/package.py --profile master --format rpm --build` | `build/tefway/master/rpm/RPMS/x86_64/tefway-remote-master-<versao>-1.x86_64.rpm` |
| Debian | `python3 res/tefway/package.py --profile master --format deb --build` | `build/tefway/master/tefway-remote-master_<versao>_amd64.deb` |
| AppImage | `python3 res/tefway/package.py --profile master --format appimage --build` | `build/tefway/master/TefwayRemoteMaster-x86_64.AppImage` |

Repeat each line with `--profile client`. Detailed prerequisites:
`docs/build/TEFWAY-REMOTE-{WINDOWS,FEDORA,DEBIAN,APPIMAGE}.md`.

## Pre-build checklist on a fresh host

1. `git submodule update --init --recursive` — `libs/hbb_common` is a submodule
   and the tree does not compile without it.
2. Rust **1.75**, Flutter **3.24.5**, vcpkg at baseline
   `9e593bb18ea69cc5095e012465dcd675a822ed0d`, `VCPKG_ROOT` exported, and the
   `vcpkg.json` dependencies installed for the host triplet (on Windows:
   `x64-windows-static-md`).
3. Generate the bridge (`flutter_rust_bridge_codegen` 1.80.1) and apply
   `ffi_bindgen_function_refactor` from `build.py`. Without this, nothing Flutter
   compiles — on any platform.
4. `python res/tefway/gen_icons.py` only if the owner supplies new artwork.

## Open item that blocks public distribution

**TEFWAY-REMOTE-AUTH-01 — technician authentication.** The gate asked that only
authorised Tefway technicians be able to connect. What exists today:

- the Client requires a local click for every session, so an attacker cannot
  take control silently;
- the identity of the *technician* is not authenticated at all — any RustDesk
  peer that knows the Client's ID can request a session, and the end user decides
  based on what they see.

Implementing this needs a server-side decision (tokens issued per technician,
verified before the login reaches the Client) plus a verifier in the client
binary. That is a separate piece of work with a backend component, and it is not
simulated here. Until it lands, treat these builds as **experimental**: they are
safe against silent takeover, but they cannot distinguish a Tefway technician
from any other peer. Do not advertise them as "only Tefway can connect".

## Other known limits

- Icon sizes 256 and 512 are upscaled from the 128 px ICO. A square vector mark
  would be sharper; drop it into `assets/` and re-run `gen_icons.py`.
- No installer/MSI branding pass, no code signing, no notarisation.
- The AppImage has no systemd unit, so it cannot serve sessions from the login
  screen (upstream limitation of that packaging format here).
- Fedora's `build.py` branch packages the Sciter build, so the Fedora document
  drives cargo/Flutter directly instead of `build.py --flutter`.
- `src/ui.rs` (deprecated Sciter UI) still reads
  `hbb_common::config::is_incoming_only()`; the Tefway builds are Flutter-only,
  and the enforcement that matters is in `client.rs` / `server/connection.rs`.

## Success criteria for the build phase

1. Both profiles build on all four targets with the pinned toolchain.
2. Both binaries show the Tefway icon and product name; window title reads
   `Tefway Remote Master` / `Tefway Remote Client`.
3. Client shows its ID, cannot connect out from any UI path, and asks for
   approval on every session, including reconnects.
4. Master refuses every incoming login.
5. Network settings show `remote.tefway.com.br` and the Tefway key, read-only.
6. A session survives the relay (port 21117) with the Tefway public key, on both
   products.
