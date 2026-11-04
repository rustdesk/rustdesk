# Gate Report — TEFWAY-REMOTE-UX-02

Branch: `feat/tefway-remote-ux-02` (from `feat/tefway-remote-portable-linux-01`).
Scope delivered: UX-02A (tokens + audit), UX-02B (Master console), UX-02C (Client
surface and incoming-session consent), UX-02E (portable notices), plus a real
Linux build and runtime pass for both profiles.
Status: **surfaces implemented, both portable builds produced from this branch,
static analysis clean, home screens verified on a real X server. One surface —
the incoming consent card — is compiled but was not exercised end-to-end, because
a single machine cannot produce two distinct RustDesk identities (see §5).**
No merge, release or production change.

## 1. Design audit (UX-02A) — findings that shaped the implementation

| Finding | Consequence |
| --- | --- |
| Home is a two-pane layout: 200 px ID/password board + `ConnectionPage` (peer search, `OnlineStatusWidget`) | Replaced for Tefway builds by a single-surface shell; stock layout untouched for non-Tefway builds |
| Client reuses the incoming-only layout (280 px pane, ID board, one-time password board, help cards) | Client now has its own 440 px card: ID, availability, guidance, notices, footer |
| `buildHelpCards()`/`buildInstallCard()` render a full-width pink→salmon gradient with install/update/download calls-to-action | Not reachable from any Tefway surface; Tefway notices are small contextual cards |
| Approval window (`DesktopServerPage`) shows a cyan→blue gradient header, a grey permission grid and grey Accept/Cancel buttons | Unauthorized state now renders `TefwayConsentCard` in Tefway builds; accept/refuse logic untouched |
| Navigation: `DesktopTabPage` tab bar (Home, Settings) + one core window per session window | Kept as-is; session windows are the real session unit |
| Status sources: `mainGetConnectStatus()` (`status_num`, `video_conn_count`), `stateGlobal.svcStatus`, `rustDeskWinManager` window lists | Every Tefway state derives from these; no state is synthesised from "a window opened" |
| `bind.isIncomingOnly()` / `isOutgoingOnly()` already separate the two products | Client/Master divergence comes from the Rust profile, not from UI flags |
| Portable hazards: `url_launcher` links to rustdesk.com, `google_fonts` (network), update banner | Tefway surfaces launch no URL, load no network font, show no update banner |

## 2. Changes

New files:

```
flutter/lib/tefway/design.dart                      tokens + theme
flutter/lib/tefway/strings.dart                     PT-BR product copy
flutter/lib/tefway/widgets.dart                     shared components
flutter/lib/tefway/notices.dart                     portable notices
flutter/lib/tefway/consent.dart                     incoming-session consent card
flutter/lib/tefway/session_registry.dart            sessions opened by this Master window
flutter/lib/tefway/home.dart                        profile dispatcher
flutter/lib/tefway/pages/master_home_page.dart      Master console
flutter/lib/tefway/pages/client_home_page.dart      Client assistance screen
docs/design/TEFWAY-REMOTE-DESIGN-SYSTEM.md
docs/design/TEFWAY-REMOTE-UX-FLOWS.md
docs/design/screenshots/{client,master}-home.png    runtime captures (see §4)
docs/gates/TEFWAY-REMOTE-UX-02.md                   this report
```

Modified files (regression surface, each change justified):

| File | Change | Why it is unavoidable |
| --- | --- | --- |
| `flutter/lib/desktop/pages/desktop_home_page.dart` | one import, one `if (isTefwayBuild()) return const TefwayHomePage();` at the top of `build()` | the home tab is owned by this widget; the branch is the smallest hook that swaps the surface without restructuring the existing layout |
| `flutter/lib/desktop/pages/server_page.dart` | two imports, one `if` in `buildConnectionCard`, one new top-level function `buildTefwayConsentCard` | the approval card is built by this file; the new function keeps the Tefway layout beside the existing `checkClickTime` guard instead of moving server logic into a widget |
| `flutter/lib/main.dart` | theme/themeMode selection in the two `GetMaterialApp` constructors, one import line | branding must reach dialogs, settings and session windows, which read the app theme |
| `flutter/lib/common.dart` | `imcomingOnlyHomeSize` for the Client profile; one import line | the incoming-only window sizes itself from this constant, so a 280 px window would clip the new Client card |

Existing runtime paths preserved when the feature is off: a stock build renders
exactly the previous layout, help cards, sizes, consent card and theme.
`TefwaySessionRegistry` is only written by the Master page. No Rust file was
touched.

## 3. Contracts preserved

- Rust + Flutter architecture unchanged; no protocol change; no dependency added.
- `src/tefway.rs` untouched: Master still refuses incoming sessions, Client is
  still incoming-only, approval mode is still `click`, permanent password cannot
  be set, logon-screen password stays disabled.
- The Client still cannot start an outgoing connection: its home has no connect
  field, and `isOutgoingOnly`/`isIncomingOnly` enforcement is unchanged.
- Consent remains a local click: the consent card calls the same
  `sendLoginResponse` / `cmCloseConnection` / `cmSwitchPermission` paths as the
  stock window, all wrapped in `checkClickTime`. No auto-accept, trust store,
  timer or password path was added.
- OS notifications (Windows, Wayland/PipeWire) untouched.
- No screenshot capture of the session, no recording, no credential logging and
  no analytics added.
- RustDesk AGPL attribution kept in the footer of both surfaces.
- Portable builds unaffected: no install step, no service enablement, no
  registry/metadata change in this gate.

## 4. Verification performed

### 4.1 Static analysis and builds

Builder: `localhost/tefway-linux-builder:bookworm-rust1.99.1-flutter3.44.8`
(Rust 1.99.1, Flutter 3.44.8), rootless Podman.

| Step | Result |
| --- | --- |
| `flutter pub get` | exit 0 |
| `flutter analyze lib` | 357 issues, **0 errors, 0 warnings** (all remaining are pre-existing `info` deprecations) |
| `docker/tefway-linux/build.sh --profile client --skip-image` | `BUILD_EXIT=0`, AppImage sha256 `191e2d35a18754e8d80d88726ade835ede169ee8dab3db58558f0dc777770fd2` |
| `docker/tefway-linux/build.sh --profile master --skip-image` | `BUILD_EXIT=0`, AppImage sha256 `a335b9f73b907249906223ecd69886c680b3bfee817bbaa23c6c0af11ad500c8` |

Both builds were made from this branch's working tree. Artifacts stay in
`build/tefway/{client,master}/` (untracked).

### 4.2 Runtime pass (Fedora 44, Xvfb 1440x900, normal user, no installation)

Both AppImages were launched with `APPIMAGE_EXTRACT_AND_RUN=1`; neither installed
anything, required root, or enabled a service.

- **Client** (`docs/design/screenshots/client-home.png`, window 440x720):
  product header "Tefway Remote / Assistência ao cliente", "Seu ID de
  atendimento" `525 313 762`, "Copiar ID", green "Aguardando atendimento —
  Ninguém está conectado ao seu computador.", the authorization sentence, "Este
  aplicativo não inicia conexões para outros computadores.", and the live
  Wayland and SELinux notices with "Saiba mais" expanders. The ID matched
  `--get-id` from the previous gate, and the notices matched the host's real
  session (KDE/Wayland + SELinux enforcing), so both are wired to live state, not
  to constants.
- **Master** (`docs/design/screenshots/master-home.png`, window 800x600): header
  "Central do técnico · Master" with green chip "Servidor conectado", card "Nova
  conexão" with the ID field and primary "Conectar", section "Sessões ativas"
  with the "0 conexões" chip and the honest empty state, one line explaining that
  each session is its own window, and the footer "Suporte Tefway · Servir por
  Amor · Versão 1.5.1 · Baseado no RustDesk · AGPL-3.0".
- Both windows render in the Tefway palette with no gradient, no install card and
  no update banner.

### 4.3 Not verified

- **Incoming consent card end-to-end.** A Master launched on the same machine
  could not reach the Client: both binaries derive the same machine identity, so
  the connection is refused as a self-connection before the approval window ever
  opens, and RustDesk exposes no CLI or environment override for the ID or the
  config directory (checked in `libs/hbb_common` and `src/cli.rs`). The card
  compiles and the accept/refuse callbacks are the stock ones, but **no
  screenshot of it exists and no click was performed.** Validating it needs a
  second machine, or a container with an isolated `/etc/machine-id`.
- **Windows 11** run: none in this session. The Windows-only UAC notice and the
  block-input capability row therefore remain unrendered.
- **Cross-platform / session matrix**: two simultaneous sessions, refusal,
  reconnect, disconnect, Client-as-target with a real technician, portable run on
  Fedora via the AppImage desktop entry.
- **Scale matrix** (1280×720, 1440×900, 1920×1080 at 100/125/150/200 %). The
  surfaces scroll and use flexible rows, but no measurement was taken. On the
  440x720 Client window the footer sits below the fold and the content scrolls;
  that is by design for a window this narrow, not verified as intentional
  layout at larger scales.
- No Rust tests were run: no Rust file changed in this gate.

## 5. Remaining work

UX-02C — close the consent validation gap:

- Exercise the card with a second machine (or an isolated client identity) and
  capture: pending request, adjusted permissions, refusal, and the session
  starting after "Permitir acesso". Until then the surface is unproven at
  runtime.

UX-02D — session experience:

- Session toolbar (`flutter/lib/desktop/widgets/remote_toolbar.dart`, and the
  authorized connection-manager panel) still uses the inherited visual language.
  Target/destination labelling, connection state and end-session grouping need
  the design tokens.
- The Client home reports the session count and points at the approval window;
  it has no per-session end control because the core exposes
  `cmCloseConnection(connId)` only from that window.

UX-02F — branding finish:

- No light theme for Tefway; the build is always dark.
- Tray/AppImage icons and metadata come from the previous gate and were not
  re-reviewed here.
- The main window keeps the inherited tab bar (Home + Settings) above the Tefway
  home. It is the only entry point to Settings, so it was kept; a discreet
  in-header settings entry would let it be hidden.

UX-02G — QA closeout: everything listed in §4.3.

## 6. Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Consent card never exercised | A layout or callback mistake would only appear in front of a real customer | Stock callbacks and `checkClickTime` reused verbatim; `analyze` clean; validation is the first item of the next round |
| Master session list is window-based | Two sessions sharing one remote window are listed together and end together | Panel states this in one line; the alternative would be new cross-window state |
| Session liveness polled every second | Up to 1 s of stale list; negligible CPU for a handful of windows | Pruning derives from the core's own window list, not a timer guess |
| Client window sized through the incoming-only constant | A future Client layout change must update that constant | Single constant referenced by the window sizing path |
| Always-dark Tefway theme | Users who prefer light mode get a dark app | Deliberate; a second validated palette is UX-02F |
| Visual evidence is one host, one screen size | Layout could break on Windows or at 150 % scaling | Captures committed with the report so the review sees exactly what was validated |

## 7. Commits

Two commits on `feat/tefway-remote-ux-02`:

1. `5cc0104b2` — design system, Master/Client surfaces, portable notices, docs.
2. Consent card, `server_page.dart` hook, docs and runtime screenshots.

Nothing was pushed, merged or published. Build artifacts remain untracked under
`build/tefway/`.
