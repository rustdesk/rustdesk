# Tefway Remote — Windows x64 build (NOT VERIFIED)

Status: **NOT VERIFIED** — this document describes the validated build procedure,
but the Windows EXE artifacts could not be produced on the BUILD-01 host because
`flutter build windows --release` requires symlink support (Developer Mode or
Administrator rights), which the reduced-privilege build session does not have.
See `docs/gates/TEFWAY-REMOTE-BUILD-01.md`, Gate 3.

This document becomes `VERIFIED` only after `TefwayRemoteMaster.exe` and
`TefwayRemoteClient.exe` are actually produced, launched, and the Master → Client
consent flow is executed on a real Windows machine.

## Products

- `build\tefway\master\TefwayRemoteMaster\TefwayRemoteMaster.exe`
- `build\tefway\client\TefwayRemoteClient\TefwayRemoteClient.exe`

Each directory is the complete portable package: the exe plus `data\`, the DLLs
(including `librustdesk.dll` and `dylib_virtual_display.dll`) and the Flutter
assets. **Not a single self-contained exe**: ship the directory (zip it), not the
exe alone.

## Environment (validated on the BUILD-01 host)

| Tool | Version |
| --- | --- |
| Rust / cargo | 1.99.1 (`x86_64-pc-windows-msvc`) |
| Flutter | 3.44.8 (stable) |
| `flutter_rust_bridge_codegen` | 1.80.1 (pinned by `.github/workflows/bridge.yml`) |
| Python | 3.11.9 |
| vcpkg | `C:\vcpkg`, baseline `9e593bb18ea69cc5095e012465dcd675a822ed0d` (`vcpkg.json`) |
| LLVM/libclang | `C:\Program Files\LLVM\bin\clang.exe` (bindgen) |
| Visual Studio | Build Tools 2022 17.14.8 (C++ workload) |

vcpkg dependencies (`x64-windows-static`): `opus`, `libyuv`, `libvpx`, `aom`.

## One-time: generate the gitignored bridge

`flutter/lib/generated_bridge.dart` and `src/bridge_generated*.rs` are gitignored
and produced by code generation with the pinned codegen. Do not copy them from
another build.

```powershell
flutter_rust_bridge_codegen --rust-input ./src/flutter_ffi.rs `
  --dart-output ./flutter/lib/generated_bridge.dart `
  --c-output ./flutter/macos/Runner/bridge_generated.h
cp ./flutter/macos/Runner/bridge_generated.h ./flutter/ios/Runner/bridge_generated.h
```

## Build commands

From the repository root (Windows):

```powershell
# Master (technician station: starts sessions, never accepts them)
python res\tefway\package.py --profile master --format windows --build

# Client (end-user station: shows its ID, accepts each session by hand)
python res\tefway\package.py --profile client --format windows --build
```

`--build` runs, in order: the virtual-display dylib, `cargo build --locked
--features flutter --lib --release`, `flutter build windows --release`, copies
`dylib_virtual_display.dll` and the build manifest into the bundle. To package an
already-built bundle, drop `--build`.

## Profile isolation (Gate 1)

`build.rs` bakes `TEFWAY_PROFILE` into the binary and aborts on any value other
than `master`/`client` (unset → stock RustDesk, never packaged as Tefway).
`package.py` cross-checks `target/release/tefway-build-manifest.json` against the
requested profile, rejects bundles without a matching marker, and writes a
sibling `<artifact>.manifest.json` evidence file per package (profile, version,
commit, OS, arch, resources, server identity, SHA-256). No secrets.

## Verification checklist (to be completed on a real Windows machine)

- [ ] `cargo check --locked --features flutter --lib --release` passes.
- [ ] Both EXEs produced at the paths above.
- [ ] Each EXE launches (`RUNTIME_PASS`).
- [ ] Logo SVG renders in light and dark theme.
- [ ] Tefway icon on the exe, window title bar, taskbar, Alt+Tab.
- [ ] Product name correct ("Tefway Remote Master" / "Tefway Remote Client").
- [ ] No missing assets.
- [ ] Master → Client connect: Client shows accept dialog; decline grants nothing.
- [ ] Reconnect after disconnect: Client asks again (no stored approval).
- [ ] Master refuses incoming login attempts.
- [ ] Client cannot start a session by any path.
- [ ] Rendezvous/relay/key point to `remote.tefway.com.br` / Tefway key only.

## Known issues

- Unsigned builds raise SmartScreen; code signing and MSI branding are out of
  scope for this gate.
- The 256/512 px icon frames are upscaled from the 128 px source ICO.
