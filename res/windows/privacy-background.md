# Windows Privacy Mode 1 backgrounds

RustDesk now controls the Windows Privacy Mode 1 background from its own Display settings. On Windows, open **Settings → Display → Privacy mode background** and choose:

- **Black**: an opaque black local screen.
- **Desktop wallpaper and taskbar**: the current Windows wallpaper plus the real Windows taskbar. The taskbar clock and notification area keep updating, so the visible result follows a newly created virtual desktop more closely than a static image.

The choice is saved as the local `privacy-mode-background` option. When Privacy Mode 1 starts, RustDesk passes the choice to its Windows privacy-window broker. No PowerShell command, extra configuration file, or modified controller client is required. The Mac controller can use the same mode as any other controller; `Ctrl+P` continues to exit through the existing Windows keyboard hook.

The desktop option captures the wallpaper path from the active Windows user when the privacy window is created. It does not copy desktop icons or application windows. The existing Privacy Mode 1 z-order leaves the real taskbar available, while the overlay covers the original desktop contents. Multi-monitor coverage and DPI scaling continue to use the existing privacy-window geometry.

If the wallpaper cannot be loaded, Privacy Mode 1 falls back to pure black. Privacy Mode 2 is unchanged. This remains an overlay rather than a Windows security lock screen or a separate Windows virtual desktop.

While the privacy windows are shown, `MagShowSystemCursor(FALSE)` suppresses the
local system cursor. The overlay keeps its existing layered, transparent,
non-activating styles; mouse injection and hit testing are unchanged. Hiding the
windows, receiving a session-lock notification, or closing the privacy window
restores cursor rendering and releases the magnification runtime. Unlocking hides
the cursor again only if Privacy Mode 1 is still active. These calls run on the
privacy-window thread, outside `DllMain`.

The connection lifecycle is synchronized with Windows locking. When a session
ends, RustDesk posts the hide request and waits for the privacy broker process
to exit before it calls `LockWorkStation()`. If a new connection arrives while
the session is already locked, the broker keeps its windows hidden and waits
for `WTS_SESSION_UNLOCK`; disconnecting that connection cancels the pending
display before the broker is stopped.

## Build

The RustDesk workflow builds the pinned `RustDeskTempTopMostWindow` revision with
`res/privacy_mode_background.patch` and `res/windows/privacy_background.h`.
Both x64 and ARM64 Windows DLL jobs use the same source. The Rust and Flutter
settings select the background at runtime; the controller does not need a custom
build.

For a local DLL build, clone `https://github.com/rustdesk-org/RustDeskTempTopMostWindow`,
checkout `ecd8d6a139eee76845ea66423fb739af450fda90`, apply the patch with `git apply`,
and copy `res/windows/privacy_background.h` to `WindowInjection/`. Build the DLL
with the installed Visual Studio C++ toolset and place it beside the RustDesk
executable. The regular RustDesk build downloads this artifact automatically.

## Validation

The Windows workflow runs `privacy_background_test.cpp` for the black, wallpaper,
reload and missing-image paths, and rebuilds `WindowInjection.dll`. A hardware
check should still cover a real Windows host controlled from macOS: enter Mode 1,
verify the local taskbar clock continues to update, test remote input, exit with
`Ctrl+P`, disconnect while Privacy Mode is active, and exercise mixed-DPI and
multi-monitor layouts. The existing capture exclusion, input hook and Mode 2
paths are unchanged.

For cursor validation, move over buttons and text from the controller while Mode
1 is active: the host cursor should stay hidden, and the controller should retain
the appropriate cursor shape. Check left/right clicks, dragging and scrolling,
then check cursor restoration after `Ctrl+P` and disconnect. Test lock/unlock on
the actual host as well; synthetic session messages do not validate the secure
desktop transition. Unexpected broker termination also needs a runtime check.

Reference: https://github.com/rustdesk/rustdesk/discussions/2919
