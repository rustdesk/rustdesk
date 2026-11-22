# Windows remote-only audio

In **Settings -> General -> Redirect audio to remote client**, install the signed VB-CABLE driver, click **Refresh**, and select its playback endpoint, normally **CABLE Input**. RustDesk does not install an unsigned driver or enable Windows test signing.

The selected endpoint is stored by Windows endpoint ID. The worker validates that it is an active VB-Audio render endpoint and that CPAL can open it. If the endpoint is missing or ambiguous, RustDesk reports an error and keeps the current system output unchanged.

## Behavior

| Situation | Behavior |
| --- | --- |
| Normal | RustDesk does not change the Windows default output or mute state. |
| Remote only, no connection | The local output is left unchanged. The device selector is available for auditioning. |
| First remote video/audio connection | The worker saves the console, multimedia, and communications default endpoint IDs, every active endpoint's volume and mute state, and the selected cable volume, then switches all three roles to the selected virtual endpoint. |
| Last remote connection disconnects | The saved physical output is restored with the exact default endpoint, volume, and mute state it had before the connection. |
| Switch to Normal while connected | The worker immediately restores the saved physical output and clears the switched state. No manual reconnect or Windows sound-panel action is required. |
| Change the selected endpoint while connected | The worker switches to the new virtual endpoint immediately and keeps the original physical output unchanged in the saved state. |
| Audition with no connection | **Audio preview** switches temporarily to the selected virtual endpoint. **Stop audio preview** restores the physical output, and a 15-second timeout restores it automatically. |
| Multiple remote connections | The server reports the authoritative remote connection count. The worker switches on the first connection and restores only after the last connection closes. |
| RustDesk crash, service restart, or stale switched state | The state file is checked when the worker starts. If it says the output is switched while the count is zero, RustDesk restores the saved output and clears the switched flag. |
| Saved physical output is unavailable | RustDesk selects the first active non-VB-Audio render endpoint, leaves that fallback endpoint's current volume and mute state unchanged, and logs the fallback. |

## Implementation

The server answers AudioConnCount(None) from its AUTHED_CONNS table. The per-session --server process starts the worker and receives the authoritative count over IPC, so the worker remains active when the main Flutter window is closed and only the tray remains. A normal UI process uses the worker only when no --server process is available. A dedicated rustdesk-audio-routing worker owns COM, PolicyConfig, WASAPI endpoint enumeration, and all endpoint changes. Commands are serialized through a standard channel, so connection callbacks, Flutter calls, preview timeout recovery, and device changes cannot overlap.

The Windows service runs as LocalSystem in Session 0 and cannot change the interactive user's default render endpoint. The service therefore only launches the per-session --server process; endpoint switching is intentionally kept there. If no interactive user session exists, no default endpoint is changed. When the per-session process starts, it initializes the worker and restores any stale state file left by a crash or forced exit.

The persistent state is JSON at Config::path("audio-output-state.json"). Its switched flag, original_defaults, original_mutes, and original_volumes arrays are the recovery contract; virtual volume is kept beside them. The existing audio-output-restore local option is read only as a compatibility migration path.

The selected endpoint is also passed to the server audio capture path. WASAPI loopback captures the virtual endpoint and the existing Opus transport carries the stream to the Mac client. Normal mode and voice-call microphone routing remain unchanged.

## Verification checklist

1. With Normal selected, play local audio and confirm the default endpoint and mute state do not change.
2. Select a cable, click **Audio preview**, confirm local speakers are silent, then click **Stop audio preview** and raise the physical volume.
3. Repeat preview without clicking Stop and confirm the 15-second timeout restores the physical output.
4. Select Remote only with no active connection and confirm local playback remains unchanged.
5. Connect one Mac client, play a browser video, and confirm the Mac hears it while the Windows speakers remain silent.
6. Connect a second client, disconnect the first, and confirm the Windows output remains routed to the cable.
7. Disconnect the last client and confirm the original physical endpoint, volume, and mute state are restored exactly.
8. While connected, change the cable in the dropdown and confirm the new cable is used without losing the original physical endpoint.
9. While connected, select Normal and confirm the physical endpoint returns immediately without the previous Reconnect the previous audio output error.
10. Remove or disable the selected cable and confirm the UI shows an unavailable-device error without falling back to speakers.
11. During Remote only audio, enter and exit privacy mode repeatedly and confirm the privacy-mode crash regression remains fixed.
12. Close the main window and leave only the tray, then connect and disconnect; confirm switching and restoration still occur.
13. Lock the Windows session, connect and disconnect, then unlock; confirm the endpoint state is restored without requiring the settings window.
14. While connected, terminate the main UI process, reconnect the UI, and confirm the per-session worker keeps routing and then restores after disconnect.
