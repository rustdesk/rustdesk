# RustDesk, unattended custom build

This repository is a fork of [RustDesk](https://github.com/rustdesk/rustdesk), the open-source remote desktop written in Rust. It exists to produce one thing: a RustDesk client that is pre-configured for our own rendezvous/relay server and installs for unattended access with no setup on the target machine.

Everything else, the protocol, screen capture, codecs and the Flutter UI, is upstream RustDesk and is tracked unchanged. Bugs and feature requests that are not specific to this build belong in the upstream tracker.

## What this build changes

The customizations are deliberately small so upstream merges stay cheap.

| Change | Where |
| --- | --- |
| Rendezvous server, relay server and the server public key are baked in; the API server is left blank. | `apply_build_defaults()` in `src/common.rs` |
| Incoming sessions use password approval with the permanent password only, at full access. The permanent password is preset (salted hash plus salt) and cannot be changed from the UI. | `apply_build_defaults()` in `src/common.rs` |
| The connection-manager window is hidden for incoming sessions, without needing a signed custom-client config. | `hide_cm` handling in `src/ipc.rs` |
| The Windows driver payload (virtual display driver, remote printer driver and adapter) is downloaded, checksum-verified and bundled by `build.py` itself, so local builds and CI produce the same installer. | `stage_windows_drivers()` in `build.py` |
| The remote printer is installed by default. It can still be turned off in the installer, with `printer=0` on a silent install, or with `INSTALLPRINTER=0` on the MSI. | `flutter/lib/desktop/pages/install_page.dart`, `src/platform/windows.rs`, `res/msi/Package/Fragments/ShortcutProperties.wxs` |
| The release workflows request `contents: write`, so this fork can publish its own releases (GitHub gives forks a read-only token by default). | `.github/workflows/flutter-nightly.yml`, `.github/workflows/flutter-tag.yml` |

To point the build at a different server or password, edit `apply_build_defaults()` and rebuild. The `password` value uses RustDesk's preset-password storage format: `00` followed by base64 of SHA-256(password + salt); `salt` is the matching salt.

## Building

### GitHub Actions (the normal way)

1. On the fork, open the **Actions** tab and enable workflows if GitHub is still asking.
2. Run **Flutter Nightly Build** with *Run workflow*. It publishes a `nightly` pre-release containing `rustdesk-<version>-x86_64.exe` (self-extracting installer), `rustdesk-<version>-x86_64.msi`, plus the unsigned `rustdesk-unsigned-windows-x86_64` artifact. Pushing a tag such as `1.4.9-1` produces the same through **Flutter Tag Build**.
3. Scheduled (cron) runs never fire on forks; use *Run workflow* or a tag.

Binaries are unsigned unless the signing secrets are configured, so expect a SmartScreen prompt on first run.

### Locally on Windows

Toolchain versions are pinned in `.github/workflows/flutter-build.yml` (Rust, Flutter, LLVM, vcpkg commit). In short: Rust 1.75, Flutter 3.24.5, LLVM 15, Python 3, and vcpkg with `libvpx libyuv opus aom` for the `x64-windows-static` triplet, with `VCPKG_ROOT` set.

```
python3 build.py --portable --flutter --hwcodec --vram
```

The result is `rustdesk-<version>-install.exe` in the repository root. `--skip-drivers` builds without the driver payload (offline development only). `python3 build.py --stage-drivers DIR` fetches only the driver payload into `DIR`, which is also a quick way to check that the pinned downloads still resolve.

### Other platforms

The server defaults apply on every platform; the driver payload is Windows-only. Linux, macOS, Android and iOS build exactly as upstream RustDesk does, see the [upstream build documentation](https://rustdesk.com/docs/en/dev/build/).

## The Windows driver payload

The installer directory carries, next to `rustdesk.exe`:

- `usbmmidd_v2/`: the Amyuni USB Mobile Monitor virtual display driver. The service installs it on first use through SetupAPI from `usbmmIdd.inf`. It backs privacy mode "Mode 2" and plugging in virtual displays, including on headless machines.
- `drivers/RustDeskPrinterDriver/` and `printer_driver_adapter.dll`: the RustDesk v4 printer driver and its adapter. The installer registers the "RustDesk Printer" on Windows 10 and later; print jobs sent to it arrive on the controlling side.
- `dylib_virtual_display.dll`: controller for the alternative RustDeskIddDriver. Present for parity, not the active implementation.

Why this used to break: `build.py` never fetched any of it. Only the upstream release workflow did, in a PowerShell step that ignored download and checksum failures. A local build, or a CI run with one failed download, shipped a client whose privacy mode and remote printer could not install. The payload is now staged by `build.py` in one place, sha256-pinned, and the build fails if any required file is missing.

## Installing on a target machine

- Interactive: run the `.exe` and click **Install**. The remote printer option is on by default.
- Silent: `rustdesk-<version>-x86_64.exe --silent-install` (append `printer=0` to skip the printer). MSI: `msiexec /i rustdesk-<version>-x86_64.msi /qn` (`INSTALLPRINTER=0` to skip).
- After installation the service starts, registers with the configured rendezvous server, and accepts sessions with the preset permanent password.

## Security

- Anyone who knows the preset permanent password has full control of every machine running this build. The repository holds only a salted hash of it, but rotating the password means a rebuild and a redeploy to every machine.
- This is a public repository. Treat `apply_build_defaults()` as sensitive and review it before every release.
- Use this software only on machines you own or are authorized to manage. Unauthorized access is illegal and is not supported.

## Repository layout

- `src/`: Rust core, connection handling, services (video, audio, input, clipboard, printer) and platform code. `src/common.rs` holds the build defaults.
- `flutter/`: Flutter desktop and mobile UI.
- `libs/hbb_common`: protocol, config and codecs (upstream submodule). `libs/scrap` screen capture, `libs/enigo` input, `libs/clipboard` file copy and paste, `libs/virtual_display` and `libs/remote_printer` driver control, `libs/portable` the self-extracting installer.
- `res/`: icons, Linux packaging and the MSI project (`res/msi`).
- `build.py`: build and packaging orchestration for all platforms.
- `.github/workflows/`: CI and release builds.

## License

AGPL-3.0, the same as upstream RustDesk. See [LICENCE](LICENCE). RustDesk is developed by Purslane Tech Pte. Ltd. and contributors.
