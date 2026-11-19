# Windows remote-only audio

In **Settings → General → Redirect audio to remote client**, choose:

- **Normal**: the existing system-audio capture behavior. Local and remote playback follow their volume settings.
- **Remote only**: applications using the Windows default output play into a VB-CABLE virtual playback device. RustDesk captures that device with WASAPI loopback and sends the existing Opus audio stream. Mac clients need no protocol change.

Install [VB-CABLE from its publisher](https://vb-audio.com/Cable/) on the Windows host, then click **Refresh** and select its playback endpoint, normally **CABLE Input**. The driver is a separate prerequisite and is not redistributed in RustDesk. Follow the driver's installation and restart instructions. RustDesk does not install an unsigned driver or enable Windows test signing.

Selecting Remote only applies immediately, including before a connection starts. RustDesk saves the current default playback devices and mute settings, mutes the other currently active outputs, then selects the virtual endpoint for the console, multimedia and communications roles. The virtual output is unmuted and set to full endpoint volume; application and client volumes still apply.

Disconnecting, disabling remote audio, or closing RustDesk **does not switch back to physical speakers or unmute them**. The virtual output remains selected and consumes audio without playing it on physical hardware. This also means one of several clients disconnecting cannot restore local playback underneath the others. Choose Normal explicitly on the same Windows user account to restore the previous devices, mute states and virtual-device volume. If a change fails, restore information is retained and the settings page displays the error.

## Scope and limits

- Windows host settings only; the existing Normal, voice-call microphone, Mac playback and wire-protocol paths are preserved.
- Apps pinned to a specific physical device must be changed to the Windows default output to be heard remotely. Older apps may need playback restarted after changing the default device.
- The saved settings cover endpoints present when Remote only is enabled. Manually changing Windows output/mute settings, attaching a new physical audio device, exclusive hardware output, or another program routing the cable to speakers can override the intended local silence. This is audio routing, not an access-control boundary.
- Normal restores the original devices rather than choosing an arbitrary replacement. Reconnect an unavailable original output before restoring. Restore before uninstalling the virtual driver.
- The selected endpoint is saved by ID. The pinned CPAL API exposes only names for stream creation, so RustDesk rejects ambiguous names and missing devices instead of falling back to physical-speaker capture.
- Default-device selection uses the Windows PolicyConfig COM interface, which is not a documented public API. Failures are reported before the capture setting is accepted.

## Verification

1. Start in Normal and note the Windows default output and mute settings.
2. Select CABLE Input and Remote only. Confirm Windows uses the cable and other active outputs are muted.
3. Connect from the Mac with remote audio enabled. Play a video using the default output. The Mac should receive audio; the Windows speakers should stay silent.
4. Disconnect while the video continues. Confirm the cable remains selected and the physical outputs remain muted. Repeat with reconnect and with two remote clients.
5. Select Normal. Confirm the previous output and mute settings return. Repeat after closing and reopening the settings UI.
6. Try an unavailable/removed virtual device. The operation must show an error rather than select a physical device silently.

Build the Rust library and Flutter package together. Installing only the Flutter EXE or replacing only the Rust DLL is insufficient for the new UI bridge. This feature needs no changes to WindowInjection.dll.
