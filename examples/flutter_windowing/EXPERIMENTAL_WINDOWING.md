# Flutter 3.50.1 official desktop windowing experiment

Branch: `experimental/flutter-official-windowing`. Based on RustDesk's pinned
plugin commit `cddc2e64f8fefce4f80db87b20bcda2dd019bed6`.

This is a compatibility experiment, not a drop-in RustDesk build.
This branch contains the standalone example under examples/flutter_windowing.
The RustDesk application still selects its existing plugin.

## Opt in

Use the official Flutter **3.50.1** tag
`ae9b586f34a8443573f82ef57c3bf914a908cbf7` (Dart 3.14.0).
Keep that tag detached (`[user-branch]`), as tested here; the 3.50.1 tool
marks windowing available only under main/master channel settings. An ordinary
stable-channel checkout can reject the feature even with the environment flag.
Set `FLUTTER_WINDOWING=true` **for the build process**. This exposes the
experimental feature without changing the user's global Flutter configuration.

Run `experimental_example/tool/run.ps1 -FlutterSdk <sdk-path>` for the demo,
or add `-SelfTest` for the native executable checks. A temporary unused drive
letter avoids MSBuild's long-path failure; the mapping is removed after exit.
The script runs until the demo is closed. Self-test mode checks both the
headless engine startup and an existing implicit main view in the same build.
No vcpkg dependencies are required by this isolated plugin example.

An application must explicitly import `experimental_windowing.dart`, install
`ExperimentalWindowingBackend`, supply a per-window widget builder, and call
`runWidget(backend.root)`. The standalone Windows demo starts a headless `flutter::FlutterEngine`.
The adoption path can instead use `keepImplicitMainView: true` with an
existing runner. The root mounts its implicit FlutterView as window 0 and
adds official Window views for children. Main window native controls continue
through the existing runner/plugin. The native self-test verifies this layout
with one embedded FlutterView and two official child windows.
Flutter's verified API is the internal
`package:flutter/src/widgets/_window.dart`: `WindowController`, `Window`,
`WindowControllerDelegate`, `destroy`, `activate`, `setMaximized`,
`setMinimized`, and `setFullscreen`. These names are taken from this pinned SDK,
not presumed stable. Even patch releases may break this experiment.

## Minimal compatibility seam

`src/backend.dart` provides the optional backend contract. Existing public
`DesktopMultiWindow` methods and `WindowController` factories check it before
entering their original implementation. With no explicit installation the
registry is null, so the old native channels and multi-engine code still run.
Existing method names/signatures and ID 0 for main are retained.

The only SDK migration edits in the existing implementation are the Dart upper
bound `<4.0.0` and `describeEnum(edge)` -> `edge.name` (same channel string).
No existing native legacy C++ source is modified. The CMake
`DESKTOP_MULTI_WINDOW_OFFICIAL_ONLY` option defaults OFF. Only the standalone
example enables it to avoid linking RustDesk-specific auxiliary plugins. That
harness native stub explicitly rejects accidental legacy calls. A normal
RustDesk build retains the full original native target and its dependencies.

## Runtime semantics and limits

* `createWindow` stages an ID and arguments; `show` constructs the official
  native controller/view. Unlike legacy, hidden creation does not start a new
  engine or execute `main(args)`.
* One engine/isolate serves every official view. Widget state, text controllers,
  handlers, listener lists, and `ExperimentalWindowContext.state` can be kept
  separate. Dart globals/statics, plugin instances, service singletons, and FFI
  registrations remain shared. The executable deliberately verifies both the
  separate widget counters and the shared global variable.
* Wrap each window's handler registration and UI callbacks with `window.run`.
  Flutter does not execute widget callbacks in a dedicated window Zone by
  itself. Unscoped calls are attributed to main ID 0. Async work started inside
  `run` keeps the target window identity across await.
* Messages use the StandardMessageCodec to copy arguments/results and preserve
  the serialized value boundary. They are in-process dispatch, not independent
  isolate MethodChannels. Closed IDs fail with StateError; absent handlers fail
  with MissingPluginException. Arbitrary exception envelope parity, pending-call
  cancellation on close, reentrancy stress, and large texture transfers have
  not been established.
* Maximize, unmaximize, minimize, restore/show, fullscreen, title, and native
  close interception were exercised on Windows. The Win32 compatibility channel
  supplies hide/show, foreground activation, centering, caption visibility, and
  outer-frame coordinates for
  the HWND returned by Flutter. It validates the process and platform thread
  before acting. Windows may still deny foreground activation. The self-test
  checks visibility and geometry against the actual window and reports the
  foreground result separately.
* On Windows, the custom drag and resize entry points post native non-client
  pointer messages to the SDK-owned window. Dragging the empty tab strip uses
  the same entry point. Interactive drag and resize still need manual verification.
  Movable, frame autosave, XID, and initial native background color still return
  UnsupportedError. The native caption toggle does not implement legacy custom
  non-client drawing or its event ordering.
  Frame coordinates are Win32 screen pixels. Mixed DPI and multiple monitors
  have not been verified.
* Close, focus/blur, maximize/unmaximize, minimize/restore, fullscreen, and resize
  events are adapted from controller state/lifetime. Windows also forwards
  native move, moved and resized events to the listeners of the associated
  window. Event ordering/count parity and
  listener mutation during callbacks remain unverified.
* Closing main requests application exit. The release executable was launched
  and exited with code 0 after an external WM_CLOSE on main.
* Windows results do not cover multiple monitors, DPI changes, suspend/resume,
  accessibility, GPU textures, remote sessions, performance, or long sessions.

## RustDesk integration boundary

The inspected RustDesk commit is
`51f94394ce61e1f3d26229565c326b2862cbdd1f`.
Its `flutter/lib/main.dart` re-enters main for each legacy child and sets
`kWindowId`, `kWindowType`, `desktopType`, and global model/FFI state. A backend
swap cannot preserve that isolation. The TCP forwarding preview uses explicit
window identity, per-window `StateGlobal` and window-local message callbacks.
Remote desktop texture and input routing still require their own migration.

## Verification

The official debug executable completed 44 checks, plus a separately recorded
foreground-focus observation. The release build compiled, opened its native main
window, and exited after system close. Three mocked channel tests verify the
default legacy routing, controller payloads, and incoming/outgoing messages.

The new implementation/example files analyze without diagnostics. Pre-existing
legacy lint warnings are outside this change. Logs and machine-readable runtime
results accompany the delivered experiment.

Sources: [Flutter announcement](https://flutter.dev/blog/desktop-windowing-apis),
[pinned SDK API](https://github.com/flutter/flutter/blob/ae9b586f34a8443573f82ef57c3bf914a908cbf7/packages/flutter/lib/src/widgets/_window.dart),
[Windows implementation](https://github.com/flutter/flutter/blob/ae9b586f34a8443573f82ef57c3bf914a908cbf7/packages/flutter/lib/src/widgets/_window_win32.dart).

The original native legacy backend also built against 3.50.1 in a separate smoke
harness, with its RustDesk-pinned window_size and texture renderer dependencies.
A final run completed 12 checks (including deliberate reproduction of the
existing fullscreen query mismatch): messages, separate globals, hide/show,
outer frame coordinates, maximize/minimize, title-bar requests, close prevention,
and destruction. The Dart method `isFullScreen` does not match native Windows
`isFullscreen`; this pre-existing bug is left unchanged. One intervening run
exited with 0xC0000005 after creating two windows. A later run passed, so legacy
stability under 3.50.1 is **not established**. No cause is asserted. Earlier
immediate show queries also demonstrated that ShowWindowAsync needs polling.

## RustDesk client preview

The Windows x64 and arm64, Linux x64, and macOS arm64 preview jobs use the
pinned Flutter SDK and apply
`.github/patches/apply_flutter_3.50_source_patches.sh`. That script selects the
local plugin and installs the preview bootstrap. Build with
`FLUTTER_WINDOWING=true` and launch RustDesk with `--experimental-windowing` to
route TCP forwarding and file transfer sessions through official child windows
in the main engine. The Windows preview also routes RDP forwarding and remote
desktop, camera and terminal through that backend. The preview retains the
runner's primary FlutterView
and uses native title bars for the official session windows.

The Windows client CI smoke tests open TCP forwarding, RDP forwarding,
remote desktop, camera, or terminal alongside file transfer in the same process.
They check for two visible `FLUTTER_HOST_WINDOW` windows, close one, and verify
that the other stays visible.
The tests also open remote desktop and camera windows for the same loopback
peer and check that the camera window stays open when remote desktop closes.
The automated checks do not require a successful peer connection. For a live
test, start a RustDesk peer on a test desktop and enable direct IP access.
Supply its temporary test password in `RUSTDESK_TEST_PEER_PASSWORD`, then run
`test_rustdesk_preview.ps1 -Executable <path> -Session connect -PeerEndpoint <host:port> -RequireVideoFrame`.
The script checks that a decoded video texture frame reaches the Dart session.
This passed from the Windows x64 preview to RustDesk 1.5.3 on a local X11 test
desktop. Visible rendering on an unlocked Windows desktop, input delivery,
camera capture, terminal I/O, and monitor switching still need verification.

## Upstream acceptance

The experiment workflow runs the native Windows x64 and arm64 tests and builds a standalone
macOS and Linux app. The Linux standalone job opens two GTK windows in a
headless Weston Wayland session to check
message routing, per-window state, visibility, and view destruction. The
hosted Xvfb check hit GLX BadAccess. The Wayland check does not cover X11 window
placement or graphics. The macOS job runs the same lifecycle checks against
native Cocoa windows. Full Flutter CI builds the integrated preview for Windows
x64, Windows arm64, Linux x64, and macOS arm64. The remaining jobs use their current
platform SDKs.
Participants can use the standalone Linux result to diagnose the plugin while
the full Linux client still uses the existing backend.

The current Windows x64 build uses Flutter 3.24.5 and a separately provided
engine to retain Windows 7 support. The experiment needs Flutter 3.50.1.
The supported Windows and macOS versions must be resolved before changing the
production toolchain. A successful Windows 11 example run does not establish
support on older systems.

Before replacing the existing backend, migrate texture/input routing and the
remaining session types. Verify two simultaneous connections with close,
reopen, tab reuse, visibility and persisted geometry. Extend the integrated
builds and runtime checks to the remaining desktop targets. GTK positioning on
Wayland depends on the compositor and needs separate runtime verification.
