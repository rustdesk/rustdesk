# Implementation Notes

## Windows-to-macOS shortcut remap

- The implemented target is `Alt+Tab` on the Windows control side producing the same remote behavior as the current `Ctrl+Tab` workflow on the macOS peer.
- This intentionally differs from the original handoff wording that explored `Ctrl+Tab` to `Command+Tab`; the user clarified that `Ctrl+Tab` should remain unchanged.
- The first version is fork-local and controlled by `ENABLE_WINDOWS_TO_MACOS_ALT_TAB_REMAP`, defaulting to enabled in this fork.
- The remap is limited to Windows builds, macOS peers, and Tab press/release events while Alt is held. It emits a legacy `ControlKey::Tab` event with `ControlKey::Control`, preserving `Shift`.
- Follow-up Windows testing showed that a single Tab event with a Control modifier can leave the remote app switcher waiting for a modifier release, and the original Alt press can remain visible on the macOS side. The remap now emits a complete tap sequence: release remote Alt, press Control, send Control+Tab down/up, release Control, and release Alt again as cleanup.
- Local macOS test execution is blocked by missing native build dependency `libyuv` under `/opt/homebrew/Cellar/libyuv`; Windows GitHub Actions remains the authoritative build validation path for the installable artifact.

## RustDesk-Herbin branding and namespace

- This fork now uses `RustDesk-Herbin` as its default app name so it can be installed beside upstream RustDesk instead of replacing it.
- The bundle identifier is `com.herbin.rustdesk`; Windows resources use `rustdesk-herbin.exe` as the original filename.
- Because config paths are derived from `APP_NAME`, the fork uses a separate configuration namespace from upstream RustDesk, including `RustDesk-Herbin.toml`, `RustDesk-Herbin2.toml`, a separate peer config directory, service name, URL protocol, firewall rule name, start-menu folder, and install directory.
- The GitHub Actions Windows x64 workflow must pass `--app-name RustDesk-Herbin` to the MSI preprocessor and publish `rustdesk-herbin-*` artifacts.
