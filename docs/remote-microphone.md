# Remote microphone (Android → Linux)

This fork can publish the Android controller's microphone as a virtual input on the Linux computer being controlled. Applications such as a voice recorder or Codex can select **RustDeskRemoteMicrophone Virtual Microphone**. The audio uses the authenticated RustDesk desktop session; OpenDroidMic and Tailscale are not required.

## Building

Both devices need this fork. The receiver is opt-in to keep older Rust/Sciter builds compatible:

- Linux requires PipeWire, its development headers, Clang and Rust 1.80 or newer for the `pipewire` bindings. Use `--features flutter,remote-microphone` in the normal Rust library build, then follow the repository's Flutter packaging instructions.
- The packaging script accepts `python3 build.py --flutter --remote-microphone` with the usual build prerequisites. It does not supply dependencies or generate Flutter bridge files itself.
- Android uses the existing Flutter/JNI API, so no new bridge function is needed. Follow the normal Android build with the Flutter version pinned in the workflow. Android 11+ is required by the reused capture path.
- `remote-microphone` is only needed on the Linux receiver. A Linux build without that feature reports the microphone capability as unavailable.

## Using

1. Connect from Android to the PC normally.
2. Tap the microphone button on the mobile session toolbar and grant microphone access if requested. After granting permission, tap again to start.
3. Select the new virtual microphone in the PC application's input selector.
4. Tap again to mute; tap once more to resume. The dropdown contains Stop and the optional automatic-start preference for this PC.
5. Capture continues when switching apps and locking the screen while the session remains active. Android shows a foreground microphone notification with a Stop action.

Automatic start is off by default and is stored on the tablet separately for each RustDesk peer. Another session cannot take over the same capture/source. Voice call and remote microphone modes are mutually exclusive while transmitting.

The virtual source persists while silenced. During a transport reconnect it is retained silently for up to 20 seconds and reused only after an authorized connection with the same peer identity. After that interval applications may need to select it again. Closing the view releases Android capture immediately.

## Local validation

`cargo run --example remote_microphone_probe --features remote-microphone,linux-pkg-config` publishes an artificial 440 Hz tone, silence, and another tone. Record the source to verify it without using a physical microphone. The example also checks that a short authorized reconnect can reuse the same source instance.

The shared workspace's `specs/001-remote-microphone/quickstart.md` describes validation with a real tablet, two wifi networks, relay and Codex. Network latency and background behavior must be measured on the actual devices. Audio is never persisted by this feature; receiving applications may record it independently.

The PipeWire source construction is adapted from OpenDroidMic under its MIT license, preserved in `src/remote_microphone_LICENSE`. The UDP protocol and discovery code are not included.
