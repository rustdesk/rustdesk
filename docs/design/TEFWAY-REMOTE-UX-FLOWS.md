# Tefway Remote — UX Flows

Companion to `TEFWAY-REMOTE-DESIGN-SYSTEM.md`. Describes what each surface does,
which core fact drives each state, and what it deliberately does not do.

## 1. Navigation map

```
DesktopTabPage (main window)
├── Home tab  ──►  TefwayHomePage            (Tefway builds only)
│                  ├── Master: TefwayMasterHomePage
│                  └── Client: TefwayClientHomePage
├── Settings tab (existing DesktopSettingPage)
└── Session windows (core: one sub-window per session window)
    └── RemotePage + RemoteToolbar (existing, unchanged)
```

The Tefway home replaces the inherited two-pane RustDesk home (ID board + peer
list). It is a single branch in `_DesktopHomePageState.build()`:
`if (isTefwayBuild()) return const TefwayHomePage();`. Everything else in
`DesktopHomePage` — timers, multi-window method handler, ID fetch — is untouched,
so session plumbing is unaffected.

The stock RustDesk layout, its help cards and the installation flow remain in the
tree for non-Tefway builds. They are never rendered by a Tefway build because the
home that would show them is not built.

## 2. Master — technician console

States, and the real fact behind each one:

| UI state | Source |
| --- | --- |
| Pronto para conectar | `mainGetConnectStatus().status_num == 1` |
| Servidor indisponível | `status_num` not 1, or the call throws |
| ID inválido | local validation: empty, non-numeric, fewer than 6 digits, or own ID |
| Solicitando conexão | `_connecting` while `newRemoteDesktop` is awaited |
| Conexão estabelecida | a session window exists and is listed by `getAllSubWindowIds()` |
| Em primeiro plano | the window id is in `rustDeskWinManager.getActiveWindows()` |
| Sessão encerrada | the window id disappears from `getAllSubWindowIds()` and the card is pruned |

Deliberately not shown: "conectado" for a window that merely opened (the panel
says "Sessão aberta"), and any per-session handshake state — the main window has
no API for the handshake inside a session window, and inventing one would be
worse than a coarse but true label.

Actions per session card are scoped to that session:

- **Abrir** → `WindowController.fromWindowId(id).show()` (real focus).
- **Encerrar** → confirmation, then `WindowController.fromWindowId(id).close()`.

"Close tab" and "disconnect" are not conflated: the Tefway Master home has no
tabs. Each session is a window; closing it ends the session, which the panel
states in one line under the list.

No incoming connection is accepted by the Master. That rule is enforced in
`src/tefway.rs` / `src/server/connection.rs`, not in this UI.

## 3. Client — assisted customer

- Identification: `gFFI.serverModel.serverId`, refreshed every second, formatted
  with the project's `formatID` and copied as raw digits.
- Availability comes from the same `mainGetConnectStatus()` document:
  `status_num` for server state, `video_conn_count` for running sessions.
- While `video_conn_count > 0` the screen shows "Atendimento ativo" with the
  count, plus the note that the session is ended in the approval window the core
  opened locally.
- Not shown: outgoing connect field, permanent password editor, unattended
  settings, RustDesk installation card, connection-manager entry points, peer
  list.

Consent is unchanged and stays where the core enforces it: every new session
opens the local approval window and needs a click. The Client home never accepts
anything on the customer's behalf.

## 4. Notices (UX-02E)

`flutter/lib/tefway/notices.dart` builds the advisory list from live conditions:

| Condition | Notice |
| --- | --- |
| Windows (any profile) | UAC: elevated windows may need extra permissions |
| `mainCurrentIsWayland()` | Wayland capture/input limits |
| `mainIsLoginWayland()` (and not current Wayland) | Login screen not capturable |
| `isSelinuxEnforcing()` | SELinux may block capture/input |
| `mainGetError()` non-empty | Core error shown as a critical notice with retry |

Each notice is a small card with "Saiba mais" expanding an inline explanation.
No notice promises UAC control that is not implemented, and none installs a
service to work around a privilege limit.

Removed for Tefway builds: new-version banner, download/update links, install
invitation, lower-version upgrade card, and the pink/orange gradient container
those used.

## 5. Future integration points (not implemented)

Reserved space only, no request, no placeholder data: service context (TEF, PIN
pad, POS, communications, network, operating system) would come from Tefway Tools
through an integration that does not exist yet. Nothing in the current UI claims
to know the customer's business, and no technician identity is asserted —
"técnico autenticado" is not displayed anywhere because technician
authentication is not implemented.
