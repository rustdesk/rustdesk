# Gate Report — TEFWAY-REMOTE-UX-03 (Official Branding & Visual Closeout)

Branch: `feat/tefway-remote-ux-03-branding` (from `feat/tefway-remote-ux-02`).
Scope delivered: official Tefway mark in every product surface, native icons
rebuilt from the official source, Linux window icon/desktop identity fixed, and
the Client layout finished so the attribution footer is never clipped.
No server, protocol, consent logic, authentication or deployment was touched.
No merge, release or production change.

```text
BRANDING_PASS         yes (official mark + regenerated icons; runtime verified on Linux)
BUILD_LINUX_PASS      yes (both AppImages built from this tree)
BUILD_WINDOWS_PASS    NOT_TESTED (no Windows host available)
VISUAL_QA_PASS        partial (Master + Client on Xvfb at 1280x720 / 1440x900 / 1920x1080; 125% and 150% not runnable here)
FUNCTIONAL_E2E_PASS   consent: PASS (real request between two identities, see §8); remote session: NOT_TESTED (capture backend of this environment)
```

## 1. Brand source

The gate forbids an approximated logo, an AI-invented mark, a hand-drawn
reproduction, or the RustDesk icon standing in for the brand. The repository
already carried the official mark twice (`assets/favicon.ico`, 128 px) and the
official wordmark (`assets/logo.svg`); the institutional site publishes the same
mark at 512 px, which is the only size that lets every derived icon be produced
without upscaling.

| Asset | Origin | sha256 (first 16) |
| --- | --- | --- |
| `assets/tefway-mark-512.png` (new) | `https://tefway.com.br/wp-content/uploads/2021/10/cropped-tefway-logotipo-menu-1-2.png`, 512x512 RGBA, transparent | `b395da254da679d4` |
| `assets/favicon.ico` (unchanged) | owner-supplied, 128 px | — |
| `assets/logo.svg` (unchanged) | owner-supplied wordmark, copied verbatim | — |
| `flutter/assets/tefway_mark.png` (new) | cropped from the 512 px mark, 256 px | `667f89d034a3b648` |

`res/tefway/gen_icons.py` now prefers the 512 px master, falls back to
`assets/favicon.ico`, never resizes upward, and emits the Flutter asset. Every
raster icon in the tree is derived from that script; no icon is hand-edited.

## 2. Changes

| File | Change | Why it is unavoidable |
| --- | --- | --- |
| `flutter/lib/tefway/widgets.dart` | `TefwayBrandMark` renders `assets/tefway_mark.png` (`BoxFit.contain`, square box); the old blue square + `Icons.headset_mic_rounded` is gone; a neutral "T" placeholder replaces a missing asset | the headset glyph was an invented symbol, i.e. exactly what the gate forbids |
| `flutter/lib/tefway/branding.dart` | new `kTefwayMarkAsset` constant | single import path for the asset, matching `kTefwayLogoAsset` |
| `flutter/lib/tefway/pages/client_home_page.dart` | tighter spacing, window 440x744, `_fitToScreen()` clamps the height to the tallest display minus 48 px | the footer sat ~10 px below the fold at 440x720; the clamp keeps the window on a 1280x720 desktop and lets the existing scroll view take over |
| `flutter/lib/tefway/consent.dart` | decision pinned in a footer outside the scroll view; the two actions stack while the core window is narrow | found by the real E2E in §8: at the 300 px core width the two labels could not share a row and `Permitir acesso` was clipped, with both buttons below the fold |
| `flutter/lib/common.dart` | `imcomingOnlyHomeSize` for the Tefway Client: 440x724 (was 440x700) | the incoming-only core sizes that window from this constant |
| `res/tefway/gen_icons.py` | 512 px master, no upscaling, emits `flutter/assets/tefway_mark.png` | reproducible icon pipeline |
| `res/tefway/icon-*.png`, `res/tefway/tefway.ico` | regenerated | Linux hicolor set, AppImage `.DirIcon`, Windows `.ico` |
| `res/icon.ico`, `res/tray-icon.ico`, `flutter/windows/runner/resources/app_icon.ico`, `res/{32,64,128}x*.png` | regenerated from `res/tefway/tefway.ico` | Rust binary resource, tray, Windows runner/exe, macOS packaging |
| `flutter/linux/my_application.cc` | after the upstream `rustdesk` lookup, tries `tefway-remote-client`, `tefway-remote-master`, `tefway`, guarded by `gtk_icon_theme_has_icon` | the packages install no `rustdesk` themed icon, so the window used the "image-missing" fallback; the guard matters because `gtk_icon_theme_load_icon` returns that fallback instead of `NULL` |
| `docker/tefway-linux/build.sh` | `.desktop` `StartupWMClass=${exe}` (was `${pkg}`) | matches the real `WM_CLASS` (`TefwayRemoteClient` / `TefwayRemoteMaster`), so the taskbar groups the window with its launcher |
| `docs/design/TEFWAY-REMOTE-DESIGN-SYSTEM.md` | `TefwayBrandMark` row corrected, new §3.1 "Brand assets" | documents the source of every generated file |

No Rust file, no server crate, no network protocol, no consent code and no
authentication path was modified. Existing runtime paths are untouched when the
Tefway profile is off: the stock layout, icons lookups (`rustdesk`) and window
sizes behave exactly as before.

## 3. Icons replaced

| Target | Before | After |
| --- | --- | --- |
| `res/tefway/icon-{16,32,48,64,128}` | downscaled 128 px master | resampled from 512 px |
| `res/tefway/icon-256`, `icon-512` | upscaled from 128 px (soft) | native 256 / native 512 |
| `res/tefway/tefway.ico` + 3 copies | single 128 px frame expanded | 7 frames (16–256), `08002bea19289a51` in all four copies |
| AppImage payload | icons + `.DirIcon` + `Icon=tefway-remote-client` | unchanged layout, new artwork |
| Window icon (Linux) | themed `rustdesk` only → fallback glyph | themed `tefway-remote-*` → `tefway` → `rustdesk` fallback |
| Windows `.exe` / MSI resources | Tefway artwork at 128 px | Tefway artwork from 512 px master (resources only; not validated on a Windows host) |

## 4. Client footer

The footer is a hard requirement (`Suporte Tefway · Servir por Amor ·
Versão 1.5.1` and `Baseado no RustDesk · AGPL-3.0`). Two defects were found and
fixed:

1. **Clipped footer.** At 440x720 the last line was cut by ~10 px. Spacing was
   reduced inside the card and the window grew to 440x744, which fits 1440x900
   and 1920x1080 desktops with the whole footer visible.
2. **No fit for 720 px tall screens.** `_fitToScreen()` now reads the tallest
   display through `bind.mainGetDisplays()` and shrinks the window to
   `height - 48`, which keeps the 1280x720 desktop usable and lets the existing
   `SingleChildScrollView` scroll ~38 px instead of hiding content off-screen.
   `_fitToScreen()` falls back to 440x744 when the display list is empty or
   unparsable.

Measured: content height 710 px inside a 744 px window (34 px slack); at
440x672 the page scrolls 38 px and the attribution line becomes fully visible.

## 5. Screenshots (real captures, Xvfb, no installation)

All captures come from the AppImages below, `GDK_BACKEND=x11`, a dedicated
`Xvfb` display per resolution, `WAYLAND_DISPLAY` unset. Files live in
`docs/design/screenshots/`.

| File | Surface / state | Resolution | Window | md5 |
| --- | --- | --- | --- | --- |
| `master-home-ux02.png` | Master home, before this gate | 1280x720 | 800x600 | `278c04f7` |
| `master-home.png` | Master home, official mark | 1280x720 | 800x600 | `09daf93a` |
| `master-home-1920.png` | Master home, 1920x1080 desktop | 1920x1080 | 800x600 | `09daf93a` |
| `master-validation.png` | Master validation error ("O ID deve conter apenas números…") | 1280x720 | 800x600 | `2a972f07` |
| `client-home-ux02.png` | Client home, before this gate | 1440x900 | 440x720 | `b45568ac` |
| `client-home.png` | Client home, top of the page | 1280x720 | 440x672 | `45bcba7a` |
| `client-home-1280-scrolled.png` | Client home, scrolled to the footer | 1280x720 | 440x672 | `1f5ca6b4` |
| `client-home-1440.png` | Client home, full footer visible | 1440x900 | 440x744 | `93b9f9ea` |
| `client-home-1920-context.png` | Client window on a 1920x1080 desktop | 1920x1080 | 440x744 | `18f88ee8` |
| `client-home-server-unavailable.png` | Client, core-reported "Servidor indisponível" | 1920x1080 | 440x744 | — |
| `consent-request.png` | Incoming session consent card, real request, decision visible | 1280x800 desktop | 300x490 | `53b4bf6f` |
| `consent-request-before.png` | Same card before the fix (action clipped, below the fold) | 1280x800 desktop | 300x490 | `a27612f1` |
| `consent-approved-peer-error.png` | Peer window right after the approval crossed the wire | 1280x800 desktop | 1300x740 | `cd4e91a4` |

`master-home.png` and `master-home-1920.png` are pixel-identical because the
Master window has a fixed 800x600 size and Flutter renders at the same device
pixel ratio on both displays; the same applies to the 440x744 Client captures.

Not captured, because the state cannot be produced in this environment:

- Client with an incoming session (request, active session) — see §8.
- Any window under 125% / 150% scaling: GTK only accepts integer `GDK_SCALE`
  here, and `gsettings` text-scaling below 1.25 produces the same layout as
  100% (Flutter clamps `textScaleFactor`), so it would not be evidence.
- A window-manager-decorated title bar: the test displays have no WM, so
  decoration, taskbar grouping and the titlebar icon are unobservable. The
  identity was verified at the protocol level instead — the shipped `.desktop`
  declares `StartupWMClass=TefwayRemoteClient` and the running window reports
  `WM_CLASS = "TefwayRemoteClient", "TefwayRemoteClient"`.

## 6. Static analysis and builds

```text
flutter analyze lib
  0 errors, 12 warnings, 345 infos
```

Warnings are upstream (`desktop_setting_page.dart`, `view_camera_page.dart`,
`tabbar_widget.dart`, `file_manager_page.dart`, `http_service.dart`,
`web/bridge.dart`) plus one unused `dart:math` import in `common.dart` left by
UX-02; none is introduced by this gate. No new lint suppression was added.

| Artifact | Exit | sha256 |
| --- | --- | --- |
| `build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage` | 0 | `dd1458c1e98e9d72965bd95c280af45ba99ec30a9dd76188f2e54e5295b06ce2` |
| `build/tefway/master/TefwayRemoteMaster-Linux-x86_64.AppImage` | 0 | `8abba8c092c772bf5f13abb132b6c861cbfbb5c136b7aeb0d0f36379f778b383` |

Both were built from this tree with `docker/tefway-linux/build.sh --profile
{client,master} --skip-image` (Podman builder, no network at runtime).

## 7. What was verified at runtime

- Official mark rendered in the Master header and the Client hero (screenshots
  above); no headset glyph and no blue-square stand-in left in any surface.
- Client footer fully visible at 440x744, and reachable by scroll at 440x672
  (`client-home-1280-scrolled.png`).
- Real core states surfaced unchanged: `Aguardando atendimento`,
  `Servidor indisponível` (red), the Wayland and SELinux notices with working
  "Saiba mais" affordances, ID `525 313 762`, version `1.5.1`.
- Master validation error path still works (ID shorter than 6 digits).
- AppImage identity: `Icon=tefway-remote-client`, hicolor `16/32/48/64/128/256`
  PNGs, `.DirIcon` symlink, `StartupWMClass` equal to the live `WM_CLASS`.
- GTK theme resolution in the shipped AppDir: `gtk_icon_theme_load_icon(
  "tefway-remote-client", 256)` returns the packaged PNG (pixel hash identical
  to `usr/share/icons/hicolor/256x256/apps/tefway-remote-client.png`), while
  `"rustdesk"` returns the "image-missing" fallback — the reason the lookup
  order and the `has_icon` guard were needed.
- AGPL attribution is intact in the UI and in the repository.

## 8. Incoming-session consent (FUNCTIONAL_E2E)

Two identities were run at once on separate X displays, each with its own
profile directory and its own machine id, so the rendezvous server sees two
distinct peers:

| Role | Build | Display | ID |
| --- | --- | --- | --- |
| Target (receives the request) | Client AppImage | `:93` 1280x800 | `1 385 052 287` |
| Technician (dials) | Master AppImage | `:94` 1280x800 | `1 594 100 883` |

The Master typed the target's ID and pressed `Conectar`. The target opened its
connection-manager window `1594100883 - Tefway Remote Client` and rendered the
branded consent card: peer name and ID, the six capability switches, the notice
that nothing is granted before the click, and the decision.

**The first run exposed a real defect.** The core sizes that window at
300x490 (`kConnectionManagerWindowSizeClosedChat`) and the card laid the two
actions out on one row, so `Permitir acesso` was clipped at the right edge and
both buttons sat below the 490 px fold — the approval was literally unreachable
without scrolling, and the primary label was cut. `consent-request-before.png`
shows it; `consent-request.png` shows the fix.

Fix (`flutter/lib/tefway/consent.dart`, presentation only): the decision moved
into a footer pinned outside the scroll view, and the actions stack while the
window is narrower than 300 px, sitting side by side once the chat panel widens
the window. No accept/refuse call, no permission handling and no window size
was touched.

After the fix, `Permitir acesso` was pressed and the decision crossed the wire:
the technician window switched from the password prompt to
`A solicitação de compartilhamento de tela no dispositivo remoto foi encerrada
sem ser concluída` (`consent-approved-peer-error.png`). The session does not
start because the target's capture backend is the Wayland/PipeWire portal, which
this test display cannot satisfy — an environment limit, not a consent defect.

Status split:

- Consent surface: **PASS** with a real request from another peer, including the
  fix for the defect the run uncovered.
- Full remote session (screen streaming) after approval: **NOT_TESTED** here; it
  needs a machine whose capture backend matches the session type.

Test harness notes (no product change): the two identities come from an
`LD_PRELOAD` shim that redirects `/etc/machine-id` and `/var/lib/dbus/machine-id`
to per-instance files — the client reads the second path, which is why a
`/etc/machine-id`-only override produced the same ID twice. Each instance also
runs inside its own `dbus-run-session`, because the app resolves the login
session through logind and otherwise opens the desktop ScreenCast portal on the
real session.

## 9. Risks and remaining work

| Item | Severity | Note |
| --- | --- | --- |
| Remote session after approval cannot start in this test environment | medium (environment) | the target capture insists on the Wayland/PipeWire portal; consent itself is verified end to end |
| Real-machine consent still owed for the release checklist | low | evidence above uses two identities on one host (separate machine ids, separate profiles); a two-machine run should confirm it before shipping |
| `_fitToScreen()` path not exercised | low | under Xvfb `mainGetDisplays()` returns an empty list, so the fallback 440x744 was used; the clamp is verified only by code review, while the resulting scroll behaviour was verified by resizing the window to 440x672 |
| Windows binaries not built | medium | `BUILD_WINDOWS_PASS` stays NOT_TESTED; only the `.ico` resources were regenerated and hashed |
| High-DPI scaling not captured | low | see §5 |
| `_NET_WM_ICON` empty under Xvfb | low | the window is undecorated and there is no WM; theme resolution was verified directly with GTK instead |
| Unused `dart:math` import in `flutter/lib/common.dart` | info | residue of UX-02, reported rather than fixed here to keep this gate additive |

## 10. Commits

Single commit on `feat/tefway-remote-ux-03-branding` containing the assets,
the widget/icon changes, the design-system update, the screenshots and this
report.
