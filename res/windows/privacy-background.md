# Windows Privacy Mode 1 backgrounds

This customization applies to the Windows **controlled computer**, independent of
whether the controller runs macOS, Windows, Linux, Android or iOS. The controller
does not need a modified client. It changes the existing overlay's paint only;
capture exclusion, monitor coverage, input blocking, and local **Ctrl+P** exit
remain RustDesk's original implementation. A remote Ctrl+P is still sent to the
remote application, just as before; use the remote toolbar to exit privacy mode.

The checked-in Windows artifact ships with `Background=black`. If the config file
is missing, the DLL falls back to the original RustDesk privacy notice. The choices
are:

- `notice`: original notice, also used for missing or unknown configuration.
- `black`: fully opaque black, without text.
- `wallpaper`: a static background image, center cropped to fill each monitor.
  The Windows taskbar remains the real Shell taskbar above the privacy overlay,
  so its clock, notification area and taskbar state continue to update. Desktop
  icons and windows are hidden by the overlay. This keeps the visible result
  close to a newly created Windows virtual desktop without trying to clone the
  taskbar or its locale/DPI behavior. It is not a Windows security lock screen.

## Build

The RustDesk workflow builds the pinned `RustDeskTempTopMostWindow` revision with
`res/privacy_mode_background.patch` and `res/windows/privacy_background.h`.
Both the x64 and ARM64 Windows DLL jobs use the same source. No protocol, Rust,
Flutter bridge, or non-Windows target changes are required.

For a local build, clone `https://github.com/rustdesk-org/RustDeskTempTopMostWindow`,
checkout `ecd8d6a139eee76845ea66423fb739af450fda90`, apply the patch with `git apply`,
and copy `privacy_background.h` to `WindowInjection/`. In a Visual Studio C++
developer command prompt:

```bat
msbuild WindowInjection\WindowInjection.vcxproj /p:Configuration=Release /p:Platform=x64
```

Use `/p:PlatformToolset=v143` (VS 2022) or the installed toolset when v142 is not
installed. Build the DLL matching the installed RustDesk architecture. Exit
RustDesk and its service before replacing the DLL; keep the original DLL as a
backup. A full RustDesk rebuild is not necessary for this customization.

## Select a background on the controlled Windows computer

Run PowerShell as administrator if RustDesk is installed in Program Files:

```powershell
.\set-privacy-background.ps1 -Mode black -RustDeskDirectory 'C:\Program Files\RustDesk'
.\set-privacy-background.ps1 -Mode wallpaper -RustDeskDirectory 'C:\Program Files\RustDesk'
.\set-privacy-background.ps1 -Mode wallpaper -ImagePath 'D:\Pictures\background.jpg' -RustDeskDirectory 'C:\Program Files\RustDesk'
.\set-privacy-background.ps1 -Mode notice -RustDeskDirectory 'C:\Program Files\RustDesk'
```

Turn privacy mode off before changing the background, then re-enter Mode 1.
The tool exports the current user's wallpaper as `privacy-wallpaper.png` next to
the DLL, so the SYSTEM broker does not read the wrong user's wallpaper. The
configuration is a local `privacy-background.ini`; nothing is sent over the
connection. A missing or unreadable exported image falls back to solid black.
The wallpaper is a snapshot: rerun the tool after changing the desktop wallpaper.
All monitors show the same image with their own aspect ratio. Slideshow, spanning
wallpaper and different images per monitor are not reproduced. Because the real
taskbar is retained, the clock changes while Privacy Mode is active and a Mac
controller does not need any special support.

## Validation

`privacy_background_test.cpp` renders into an offscreen Windows bitmap and checks
the default notice path, black, wallpaper, reload, and missing-image fallback.
Run in a Visual Studio developer command prompt from `res/windows`:

```bat
cl /nologo /EHsc /std:c++14 privacy_background_test.cpp /link gdiplus.lib user32.lib gdi32.lib
privacy_background_test.exe
```

Before distributing, test an installed Windows host from a Mac controller:
enter Mode 1, verify only the local display shows the overlay, type/click remotely,
exit with the local Ctrl+P, and disconnect while privacy mode is active. Also
exercise multi-monitor / mixed DPI layouts, display hot-plug and UAC. This change
preserves existing behavior in those paths; an offscreen test cannot validate
capture-driver or keyboard-hook behavior on hardware. Mode 2 remains unchanged.

Reference: https://github.com/rustdesk/rustdesk/discussions/2919
