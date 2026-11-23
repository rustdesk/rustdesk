# Windows remote-only audio

In **Settings -> General -> Redirect audio to remote client**, install the signed VB-CABLE driver. RustDesk automatically selects the usable VB-CABLE render endpoint, preferring the two-channel **Speakers (VB-Audio Virtual Cable)** endpoint and falling back to **CABLE In 16 Ch (VB-Audio Virtual Cable)** or another supported VB-Audio endpoint. RustDesk does not install an unsigned driver or enable Windows test signing.

The selected endpoint is stored by Windows endpoint ID for compatibility with older configurations, but the current UI no longer exposes an endpoint selector. The worker validates that the automatically selected endpoint is an active VB-Audio render endpoint and that CPAL can open it. If no usable endpoint is found, RustDesk reports an error and keeps the current system output unchanged.

## Behavior

| Situation | Behavior |
| --- | --- |
| Normal | RustDesk does not change the Windows default output or mute state. |
| Remote only, no connection | The local output is left unchanged. The preferred VB-CABLE endpoint is selected automatically for the next remote connection. |
| First remote video/audio connection | The worker saves the console, multimedia, and communications default endpoint IDs, every active endpoint's volume and mute state, and the selected cable volume, then switches all three roles to the selected virtual endpoint. |
| Last remote connection disconnects | The saved physical output is restored with the exact default endpoint, volume, and mute state it had before the connection. |
| Switch to Normal while connected | The worker immediately restores the saved physical output and clears the switched state. No manual reconnect or Windows sound-panel action is required. |
| VB-CABLE endpoint changes | The worker re-detects the preferred usable endpoint and keeps the original physical output unchanged in the saved state. |
| Multiple remote connections | The server reports the authoritative remote connection count. The worker switches on the first connection and restores only after the last connection closes. |
| RustDesk crash, service restart, or stale switched state | The state file is checked when the worker starts. If it says the output is switched while the count is zero, RustDesk restores the saved output and clears the switched flag. |
| Saved physical output is unavailable | RustDesk selects the first active non-VB-Audio render endpoint, leaves that fallback endpoint's current volume and mute state unchanged, and logs the fallback. |

## Implementation

The server answers AudioConnCount(None) from its AUTHED_CONNS table. The per-session --server process starts the worker and receives the authoritative count over IPC, so the worker remains active when the main Flutter window is closed and only the tray remains. Settings status and mode changes are also sent to that worker over the local IPC channel. A normal UI process uses the worker only when no --server process is available. A dedicated rustdesk-audio-routing worker owns COM, PolicyConfig, WASAPI endpoint enumeration, and all endpoint changes. Commands are serialized through a standard channel, so connection callbacks, Flutter calls, and device changes cannot overlap.

The Windows service runs as LocalSystem in Session 0 and cannot change the interactive user's default render endpoint. The service therefore only launches the per-session --server process; endpoint switching is intentionally kept there. If no interactive user session exists, no default endpoint is changed. When the per-session process starts, it initializes the worker and restores any stale state file left by a crash or forced exit.

The persistent state is JSON at Config::path("audio-output-state.json"). Its switched flag, original_defaults, original_mutes, and original_volumes arrays are the recovery contract; virtual volume is kept beside them. The existing audio-output-restore local option is read only as a compatibility migration path.

The selected endpoint is also passed to the server audio capture path. WASAPI loopback captures the virtual endpoint and the existing Opus transport carries the stream to the Mac client. Normal mode and voice-call microphone routing remain unchanged.

## Verification checklist

1. With Normal selected, play local audio and confirm the default endpoint and mute state do not change.
2. Select Remote only with no active connection and confirm local playback remains unchanged.
3. Connect one Mac client, play a browser video, and confirm the Mac hears it while the Windows speakers remain silent.
4. Connect a second client, disconnect the first, and confirm the Windows output remains routed to the cable.
5. Disconnect the last client and confirm the original physical endpoint, volume, and mute state are restored exactly.
6. While connected, confirm RustDesk uses the preferred two-channel VB-CABLE endpoint automatically, without a device dropdown.
7. While connected, select Normal and confirm the physical endpoint returns immediately without the previous Reconnect the previous audio output error.
8. Remove or disable VB-CABLE and confirm Remote only is disabled with an install prompt; RustDesk must not fall back to speakers.
9. Close the main window and leave only the tray, then connect and disconnect; confirm switching and restoration still occur.
10. Lock the Windows session, connect and disconnect, then unlock; confirm the endpoint state is restored without requiring the settings window.
11. While connected, terminate the main UI process, reconnect the UI, and confirm the per-session worker keeps routing and then restores after disconnect.
