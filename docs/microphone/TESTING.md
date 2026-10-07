# Verification and installation gate

This is an experimental feature branch, not a supported release. Preserve a working remote-access installation before installing a test build, and do not rely on the new build for unattended access until the two-device test passes. Normal remote desktop connections continue using the existing server; only microphone forwarding requires this fork at both ends.

## Automated/local checks

- `cargo test -p base --lib --locked`: 15 existing tests passed on macOS arm64, 2026-10-07.
- Flutter bridge generation: succeeded with Flutter 3.24.5 and flutter_rust_bridge_codegen 1.80.1.
- Flutter analysis of the five changed UI files: zero errors/warnings, 42 informational existing-style/deprecation notices. This does not establish runtime correctness.
- `cargo check --locked --features flutter`: passed on macOS arm64.
- `cargo test --locked --features flutter --lib microphone_forwarding::tests`: both feature tests passed (exclusive capture and dedicated protocol response).
- `tests/microphone/run-macos-route.sh`: passed both the missing-driver failure path and, after installing BlackHole 2ch 0.7.1 and rebooting, the actual default-input switch/restore path.
- `cargo test --locked --features flutter --lib microphone_blackhole_opus_loopback -- --ignored --nocapture`: passed with BlackHole installed. This encodes a synthetic tone with Opus, sends it through the new dedicated output worker, and verifies decoded samples on BlackHole's input. It does not record the physical microphone or test a network connection.
- Windows/Linux routing modules passed a Rust syntax/type check on macOS; their actual operating-system behavior remains untested.
- macOS ARM64 release application build: passed with Flutter 3.24.5, selected Xcode SDK and Rosetta for the Flutter build tool. Local ad-hoc signature verification passed; the app launched and displayed both new settings. The experimental app was installed on the development Mac with the stable app backed up. macOS screen-recording permission must be renewed for the changed signature.
- Windows x64 portable application build: passed in the contributor fork (run 37548368381).
- User-reported Windows -> macOS microphone test: successful on 2026-10-07 after renewing macOS screen-recording/accessibility permissions for the custom app. This report does not establish that every acceptance case below passed; no automated two-device audio recording was captured.
- Android ARM64 build attempted but failed in hwcodec bindgen due to host-vs-NDK header selection. A build-environment fix is being handled separately in the fork; no APK or Android runtime success is claimed here.

## Required two-device acceptance test

Use a second physical device with a physical microphone. Keep the receiver's existing application installer and configuration backed up privately.

1. Record the receiver's original default audio input and speaker output. Install the platform's virtual device as described in [the installation guide](README.md). Build/install the same feature commit on both endpoints.
2. With receiving permission off, press the controller microphone button. It must report a refusal, keep the receiver's default input unchanged, and never play the microphone through its speakers.
3. Enable receiving permission. Press the microphone button. It must progress from pending to active only after the virtual output opens. Verify a recording application's input meter moves when speaking into the controller microphone. The receiver's speaker selection must not change.
4. Record a short spoken sentence on the receiver with its physical microphone muted/disconnected. Play it back after stopping forwarding to confirm the actual source. A successful toolbar indicator alone is insufficient.
5. Stop forwarding and then disconnect. Verify the previous input returns and no audio continues. While forwarding, manually choose another input and then disconnect: that manual choice must be preserved.
6. Revoke receiving permission while connected; forwarding must stop (within the one-second configuration check). Re-enable and reconnect; test again.
7. Turn on automatic forwarding, reconnect, confirm startup. Turn it off, reconnect, confirm no capture starts. The per-session mic button must override the current stream without changing this preference.
8. Connect a second controller while the first is forwarding. Its microphone request must be refused without disturbing the existing stream. Voice call and forwarding must not overlap.
9. Deny OS microphone permission, remove the virtual device, or connect to an unmodified RustDesk peer. The UI must give an error/timeout, not silently route to speakers. Pending requests expire after ten seconds.
10. Test outside the LAN (phone Wi-Fi off). Then test a forced relay session. Test both audio directions separately; forwarding does not need another network port.

Repeat controller/receiver pairs Windows→macOS, Android→macOS, macOS→Windows, Android→Windows, and desktop→Linux. Windows/Linux builds and actual devices are necessary to claim those platforms work; a Mac compiler check does not substitute for them.

## Recovery and known limits

- Force-killing the host process or an OS crash can bypass cleanup. Select your physical microphone in system sound settings manually. Linux users can additionally unload the modules listed by `pactl list modules short`; unload only the `rustdesk_microphone` modules created for this feature.
- Windows receiving currently relies on VB-CABLE plus AudioDeviceCmdlets in the logged-in user's context. SYSTEM/pre-login/multi-user receiving is unverified.
- iOS/web microphone sending and Android/iOS virtual microphone receiving are not implemented.
- Apps may retain the input device chosen when recording started; restart their voice session or select the virtual input in the app.
- The active indicator confirms the initial output path, not intelligible speech or successful operation of every downstream application. Hardware removal during a session requires stopping and restarting forwarding.
- macOS lid closure and sleep remain separate power-management concerns. No closed-lid guarantee is made.

## Regression surface

`libs/base/protos/message.proto` adds independent control and audio envelopes, so a stopped or stale microphone packet cannot fall through to the legacy speaker path. `src/client/io_loop.rs`, `src/server/connection.rs`, and `src/client.rs` add opt-in capture/routing and lifecycle handling; the default output path and old voice-call capture function remain unchanged when forwarding is off. `src/ui_session_interface.rs` and `src/flutter.rs` carry the toggle and state events. Five Flutter UI files add controls/permission handling. `build.rs` compiles the macOS CoreAudio helper. `libs/base/src/config/keys.rs` and language files register options and labels. No upstream submodule or server protocol changes are required.
