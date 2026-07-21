# Implementation Notes

## RustDesk-Herbin 1.4.9 baseline

- Upstream baseline: official RustDesk tag `1.4.9`.
- Product identity remains isolated as `RustDesk-Herbin`, bundle ID
  `com.herbin.rustdesk`, URL scheme `rustdesk-herbin://`, and independent macOS
  config and launchd namespaces.
- The abandoned Windows and macOS shortcut-remapping experiments have been
  removed. RDH now uses upstream keyboard handling without a custom keymap file,
  built-in remap, or compatibility fallback.

## macOS remote-click activation fix

- The controlled Mac resolves the visible application under the cursor before a
  remote left-button-down event and asks AppKit to activate regular applications.
- Dock-owned windows and non-regular overlay applications are ignored.
- Activation remains best effort: failure is logged at debug level and the mouse
  click is still delivered.
- High-volume protocol, injection, focus-transition, and delayed-settle tracing
  from the diagnosis builds has been removed.

## Build and distribution

- macOS builds remain CI-first through `.github/workflows/codex-macos-herbin.yml`.
- Until a Developer ID certificate is available, artifacts are ad-hoc signed and
  are not notarized. The workflow verifies signature integrity but does not claim
  Gatekeeper acceptance.
- Artifact versions use `<upstream>-rdh.<revision>` while the application keeps
  the upstream protocol/application version.
- The upgrade rehearsal workflow never merges or installs automatically. It only
  checks the latest official release, rehearses the merge in an ephemeral runner,
  and runs the RDH static invariants.

## macOS user-server memory recovery

- The RDH `--server` now contains a low-frequency RSS watchdog with a 1 GiB default
  threshold. It is active only when the exact RDH launchd job is supervising the
  process.
- Restart requires two idle over-limit samples five minutes apart plus a final
  30-second check. Any active incoming connection or memory recovery resets the
  candidate restart.
- Recovery exits only the user server with a nonzero status so the existing
  launchd `KeepAlive` policy relaunches it. It never unloads or restarts the root
  service and therefore does not require an administrator prompt.
- `rdh-memory-restart-threshold-mib=0` disables the watchdog. Invalid values disable
  it explicitly instead of silently falling back.
- This mitigates the long-running leak but does not identify or fix its allocation
  source; heap profiling remains a separate follow-up.

No public compatibility layer is retained for the removed shortcut mapping.
