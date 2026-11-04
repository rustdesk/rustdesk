# Gate Report — TEFWAY-REMOTE-UX-02

Branch: `feat/tefway-remote-ux-02` (from `feat/tefway-remote-portable-linux-01`)
Scope delivered: UX-02A (design tokens/audit), UX-02B (Master console), UX-02C
(Client surface, part 1), UX-02E (portable notices), plus the documentation of
UX-02D/F/G.
Status: **implementation of the surfaces complete and statically verified;
visual and cross-platform functional validation not performed in this
environment.** No merge, release or production change.

## 1. Design audit (UX-02A) — findings that shaped the implementation

| Finding | Consequence |
| --- | --- |
| Home is a two-pane layout: 200 px ID/password board + `ConnectionPage` (peer search, `OnlineStatusWidget`) | Replaced for Tefway builds by a single-surface shell; stock layout untouched for non-Tefway builds |
| Client reuses the incoming-only layout (280 px pane, ID board, one-time password board, help cards) | Client now has its own narrow card: ID, availability, guidance, footer |
| `buildHelpCards()`/`buildInstallCard()` render a full-width pink→salmon gradient with install/update/download calls-to-action | Not reachable from any Tefway surface; Tefway notices are small contextual cards |
| Navigation: `DesktopTabPage` tab bar (Home, Settings) + one core window per session window | Kept as-is; session windows are the real session unit |
| Status sources: `mainGetConnectStatus()` (`status_num`, `video_conn_count`), `stateGlobal.svcStatus`, `rustDeskWinManager` window lists | Every Tefway state is derived from these; no state is synthesised from "a window opened" |
| `bind.isIncomingOnly()` / `isOutgoingOnly()` already separate the two products | Client/Master divergence comes from the Rust profile, not from UI flags |
| Portable dependencies that could break: `url_launcher` links to rustdesk.com, `google_fonts` (network), update banner | Tefway surfaces launch no URL, load no network font, show no update banner |
| Master/Client difference matrix | Master: connect out, sessions, no incoming. Client: ID only, approval per session, no outgoing |

## 2. Changes

New files:

```
flutter/lib/tefway/design.dart                      tokens + theme
flutter/lib/tefway/strings.dart                     PT-BR product copy
flutter/lib/tefway/widgets.dart                     shared components
flutter/lib/tefway/notices.dart                     portable notices
flutter/lib/tefway/session_registry.dart            sessions opened by this Master window
flutter/lib/tefway/home.dart                        profile dispatcher
flutter/lib/tefway/pages/master_home_page.dart      Master console
flutter/lib/tefway/pages/client_home_page.dart      Client assistance screen
docs/design/TEFWAY-REMOTE-DESIGN-SYSTEM.md
docs/design/TEFWAY-REMOTE-UX-FLOWS.md
docs/gates/TEFWAY-REMOTE-UX-02.md                   this report
```

Modified files (regression surface, each line justified):

| File | Change | Why it is unavoidable |
| --- | --- | --- |
| `flutter/lib/desktop/pages/desktop_home_page.dart` | one import, one `if (isTefwayBuild()) return const TefwayHomePage();` at the top of `build()` | the home tab is owned by this widget; the branch is the smallest hook that swaps the surface without restructuring the existing layout |
| `flutter/lib/main.dart` | theme/themeMode selection in the two `GetMaterialApp` constructors, one import line | branding must reach dialogs, settings and session windows, which read the app theme |
| `flutter/lib/common.dart` | `imcomingOnlyHomeSize` for the Client profile; one import line | the incoming-only window sizes itself from this constant, so a 280 px window would clip the new Client card |

Existing runtime paths preserved when the feature is off: a stock build renders
exactly the previous layout, help cards, sizes and theme. `TefwaySessionRegistry`
is only written by the Master page. No Rust file was touched.

## 3. Contracts preserved

- Rust + Flutter architecture unchanged; no protocol change; no dependency added.
- `src/tefway.rs` untouched: Master still refuses incoming sessions, Client is
  still incoming-only, approval mode is still `click`, permanent password cannot
  be set, logon-screen password stays disabled.
- Client still cannot start an outgoing connection: its home has no connect
  field, and `isOutgoingOnly`/`isIncomingOnly` enforcement is unchanged.
- Consent remains a local click in the core approval window; the Tefway UI adds
  no auto-accept, trust store, timer or password path.
- OS notifications (Windows, Wayland/PipeWire) untouched.
- No screenshot capture, recording, credential logging or analytics added. The
  notices contain no PAN/PIN/CVV handling because nothing captures those.
- RustDesk AGPL attribution kept in the footer of both surfaces.
- Portable builds unaffected: no install step, no service enablement, no
  registry/metadata change in this gate.

## 4. Verification performed

Static analysis, in the project's pinned builder image
(`localhost/tefway-linux-builder:bookworm-rust1.99.1-flutter3.44.8`,
Flutter 3.44.8):

```
flutter pub get          → exit 0
flutter analyze lib      → 357 issues, 0 errors, 0 warnings
```

All 357 remaining entries are `info` deprecation notices that already existed in
the tree (e.g. `withOpacity`, `ColorScheme.background`, `RawKeyEvent`). No new
error or warning was introduced: filtering the analyzer output for
`lib/tefway/`, `lib/main.dart` and `lib/common.dart` returns no findings from
this change, apart from the pre-existing `withOpacity` inside
`flutter/lib/tefway/branding.dart` (file not modified by this gate).

Not performed, and therefore not claimed:

- **Visual validation / screenshots.** This host has no Flutter SDK and no
  display; the required before/after screenshots for the ten states in the gate
  (Master without connection, awaiting authorization, one and two sessions,
  Client awaiting, request received, active, two sessions, permission notices,
  network errors) were **not** captured. `flutter analyze` is not evidence of
  visual quality.
- **Functional tests on Fedora KDE and Windows 11** (Master→Client, Client side
  approval, refusal, reconnect, two simultaneous sessions, session end, Master
  refusing incoming, Client unable to start outgoing, portable run). The core
  behaviour was not modified, but that is an argument, not a test.
- **Scale/window matrix** (1280×720, 1440×900, 1920×1080 at 100/125/150/200 %).
  The new surfaces scroll and use flexible rows, but no measurement was taken.
- No Rust tests were run: no Rust file changed in this gate.

## 5. Remaining work

UX-02C (part 2) — session authorization window:

- The approval UI is still the core connection-manager window
  (`src/server/connection_page` → `DesktopServerPage`): owner-coloured gradient
  header, `Accept`/`Cancel` buttons, permission grid. It must be restyled to the
  Tefway consent layout ("Solicitação de suporte remoto", capabilities list,
  Recusar / Permitir acesso), keeping the existing per-session click rule.
- Two simultaneous requests already work in the core (one client per connection);
  the visual separation of two pending requests is part of that work.

UX-02D — session experience:

- Session toolbar (`flutter/lib/desktop/widgets/remote_toolbar.dart`) still uses
  the inherited visual language. Target/destination labelling, connection state
  and end-session grouping need to move to the design tokens.
- The Client has no per-session end control outside the approval window: the core
  exposes `cmCloseConnection(connId)` only inside that window, so the Client home
  truthfully reports the count and points at the window instead of simulating a
  button.

UX-02F — branding finish:

- Light theme for Tefway is not defined; the build is always dark.
- Window/tray/AppImage icons and metadata were produced in the previous gate and
  were not re-reviewed here.
- The main window still shows the inherited tab bar (Home + Settings) above the
  Tefway home. It is the only entry point to Settings, so it was kept; a discreet
  in-header settings entry would let it be hidden.

UX-02G — QA closeout: the screenshots and cross-platform functional matrix above.

## 6. Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Master session list is window-based | Two sessions sharing one remote window are listed together and end together | Panel states this in one line; the alternative would be new cross-window state, which this gate avoids |
| Session liveness polled every second | Up to 1 s of stale list; negligible CPU for a handful of windows | Pruning is derived from the core's own window list, not from a timer guess |
| Client window sized through the incoming-only constant | A future Client layout change must update that constant | Single constant, referenced by the window sizing path |
| Always-dark Tefway theme | Users who prefer light mode get a dark app | Deliberate; a second validated palette is UX-02F |
| No functional test evidence | Regressions in the swapped branch would only appear at runtime | Branch isolated, stock path untouched, `analyze` clean; functional matrix is the next gate step |

## 7. Commits

Single commit on `feat/tefway-remote-ux-02` adding the Tefway design system,
Master/Client surfaces, portable notices and these documents. Nothing was pushed,
merged or published.
