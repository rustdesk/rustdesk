# Windows Privacy Mode 1 backgrounds

RustDesk now controls the Windows Privacy Mode 1 background from its own Display settings. On Windows, open **Settings → Display → Privacy mode background** and choose:

- **Black**: an opaque black local screen.
- **Desktop wallpaper and taskbar**: the current Windows wallpaper, an idle taskbar containing pinned shortcut icons, and a live notification area with its clock.

The choice is saved as the local `privacy-mode-background` option. When Privacy Mode 1 starts, RustDesk passes the choice to its Windows privacy-window broker. No PowerShell command, extra configuration file, or modified controller client is required. The Mac controller can use the same mode as any other controller; `Ctrl+P` continues to exit through the existing Windows keyboard hook.

The desktop option reads the user's wallpaper path and Windows `WallpaperStyle` / `TileWallpaper` settings. Fit preserves the whole image and the desktop background color in the remaining space; fill crops the image. Center, stretch, tile and span also follow their respective layouts. It does not copy desktop icons or application windows. Different wallpaper images on different monitors and slideshow updates have not been validated.

Privacy Mode 1 covers the real taskbar as well as the desktop. The DLL paints the taskbar background, a Windows start symbol, and icons loaded from the current user's pinned `.lnk` shortcuts. Shortcut targets or explicit icon resources supply the icons without shortcut arrows or running-state indicators. The application area never copies pixels from the real taskbar, so starting another application does not add its icon to the privacy background.

Only the `TrayNotifyWnd` region is copied through a cropped DWM thumbnail. DWM updates the clock and notification icons continuously; a one-second geometry check handles taskbar movement and shell recreation. Hiding or destroying the overlay releases its thumbnail, icon handles and timer. The real taskbar keeps its original window position and input behavior.

This is a rendered idle taskbar rather than a new Windows virtual desktop. Light/dark system themes and the Windows 11 alignment preference are read from Windows settings. Pinned shortcuts are read when the overlay starts; their enumeration order may differ from a manually rearranged native taskbar. Pins stored without `.lnk` shortcuts, search/widgets/task-view controls, custom taskbar styling and secondary-monitor notification areas are not fully reproduced. Auto-hidden or unavailable taskbars may remain hidden; the wallpaper still covers the screen. Multi-monitor coverage and DPI scaling use the existing privacy-window geometry.

Pinned-icon lookup runs on the privacy-window thread at first show. A shortcut
or icon target on an unavailable network location can stall this lookup and delay
hiding the overlay, session-lock handling, shutdown, and cursor restoration.

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
for `WTS_SESSION_UNLOCK`; an initial sign-in resumes on
`WTS_SESSION_DESKTOP_READY`, after Windows switches to the user's desktop.
The message pump preserves the deferral flag and checks whether its desktop
already receives input, so a queued request does not wait again after unlock.
Disconnecting that connection cancels the pending display before the broker is stopped.

## Build

The RustDesk workflow builds the pinned `RustDeskTempTopMostWindow` revision with
`res/privacy_mode_background.patch`, `res/windows/privacy_background.h` and
`res/windows/privacy_taskbar.h`.
Both x64 and ARM64 Windows DLL jobs use the same source. The Rust and Flutter
settings select the background at runtime; the controller does not need a custom
build.

For a local DLL build, clone `https://github.com/rustdesk-org/RustDeskTempTopMostWindow`,
checkout `ecd8d6a139eee76845ea66423fb739af450fda90`, apply the patch with `git apply`,
and copy both headers to `WindowInjection/`. Build the DLL
with the installed Visual Studio C++ toolset and place it beside the RustDesk
executable. The regular RustDesk build downloads this artifact automatically.

## Validation

The Windows workflow runs `privacy_background_test.cpp` for the black, wallpaper,
reload, fit/fill layout and missing-image paths, and rebuilds `WindowInjection.dll`. A hardware
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

Windows layout values: https://learn.microsoft.com/en-us/windows/win32/controls/themesfileformat-overview

Live thumbnails: https://learn.microsoft.com/en-us/windows/win32/dwm/thumbnail-ovw
