# TEFWAY-REMOTE-CLIENT-02 — Gate report

Branch: `feat/tefway-remote-client-02` (base `master` at `55786aaa5709ac0a450aabcf06f9d1fee3119908`).

Products built from this one tree: **Tefway Remote Master** (technician) and
**Tefway Remote Client** (end user). Same Rust + Flutter base, no duplicated
project.

## Gate 0 — Inventory and Git safety

- Repository valid, remote `git@github.com:tefway/rustdesk.git`, HEAD
  `55786aaa5`.
- Working tree at start: only untracked `assets/` (the owner's branding files).
  Preserved; nothing was reset, cleaned or discarded.
- Submodule `libs/hbb_common` was **not initialised** (empty directory). It was
  initialised with `git submodule update --init --recursive`, which is required
  before any Rust build; no submodule content was modified.
- Tooling on this host: rustc 1.99.1, Flutter 3.44.8 (local, newer than the CI
  pin 3.24.5), Python 3.11.9, CMake 4.2.1, git 2.56.

Assets verified before use:

| File | Result |
| --- | --- |
| `assets/logo.svg` | Valid SVG 1.1, Inkscape, `viewBox="0 0 795.59609 205.56962"`, pure paths (21 `<path>`, no `<text>`/`<image>`), 20 260 bytes. Wordmark, not a square icon. |
| `assets/favicon.ico` | Valid ICO, **single 128x128 32bpp frame**, 67 646 bytes. Usable as the square mark; too few frames for Windows as shipped. |

Neither original file was modified.

## Gate 1 — Branding

- Wordmark integrated in the Flutter UI: `flutter/assets/tefway_logo.svg` (byte
  copy of `assets/logo.svg`) and `flutter/assets/tefway_logo_dark.svg` (same
  artwork, the two brand blues lifted for dark theme). `loadLogo()` in
  `flutter/lib/common.dart` now renders the SVG through the `flutter_svg`
  dependency the project already uses; no framework upgrade.
- Surfaces using `loadLogo()` are branded automatically: desktop home left pane,
  mobile settings header.
- Product identity: `TefwayProfileBadge` (new
  `flutter/lib/tefway/branding.dart`) shows the product name and role in the
  desktop home pane. The Rust side sets `APP_NAME` per profile, which is what
  window titles read.
- Windows: `res/icon.ico`, `res/tray-icon.ico` and
  `flutter/windows/runner/resources/app_icon.ico` replaced with a multi-size ICO
  (16/24/32/48/64/128/256) generated from `assets/favicon.ico`;
  `res/32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png` regenerated.
  `build.rs` still compiles `res/icon.ico` into the Rust executable, and
  `Runner.rc` still compiles `app_icon.ico` into the Flutter runner, so the icon
  is applied where the build actually reads it.
- Linux: `res/tefway/icon-{16,32,48,64,128,256,512}.png`; the packages install
  them at `/usr/share/icons/hicolor/<size>x<size>/apps/<pkg>.png` with a
  per-profile desktop entry and systemd unit.
- Legal: RustDesk copyright and licence are untouched (`Cargo.toml` metadata,
  `LICENCE`, About card, `loadPowered` attribution). No notice was removed.

## Gate 2 — Master and Client profiles

Profile is a **compile-time** constant: `TEFWAY_PROFILE` is read by `build.rs`
and baked in with `cargo:rustc-env`, so no preference, CLI flag, IPC message or
FFI call selects it. Any other value builds stock RustDesk.

| Behaviour | Master | Client |
| --- | --- | --- |
| Start outgoing sessions | yes | **no** (`client.rs` `_start` bails; incoming-only) |
| Accept incoming sessions | **no** (`server/connection.rs` refuses the login) | yes, with click |
| Unattended access / permanent password | n/a | **no** (verification method and permanent-password change locked) |

The Client uses the project's existing `isIncomingOnly()` mechanism, extended so
the profile forces it: `src/tefway.rs::is_incoming_only()` is now what
`client.rs` (2 sites), `flutter_ffi.rs::is_incoming_only` and therefore the whole
incoming-only Flutter UI read. With no profile set the function returns exactly
`hbb_common::config::is_incoming_only()`, so stock builds are unchanged.

## Gate 3 — Mandatory consent

- Client forces the click-approval branch of `handle_login_request`
  (`src/server/connection.rs`): the condition is short-circuited by
  `tefway::requires_manual_approval()`, so the recent-session shortcut, the
  password path and the "no password" auto path are all unreachable for the
  Client. Approval is stored per connection, so a new session means a new dialog.
- Locked by config as well: `approve-mode=click`,
  `verification-method=use-temporary-password`, `enable-trusted-devices=N`,
  `disable-change-permanent-password=Y`, `allow-logon-screen-password=N`, all in
  the overwrite/builtin maps that the settings UI cannot write.
- Decline path is upstream behaviour: the CM reports a refusal, the connection is
  closed, nothing is granted. Disconnect revokes everything because the
  authorization lives on the connection.
- Bypass review: no CLI argument, FFI entry point or IPC message sets the
  profile; `approve-mode` cannot be written by `Config::set_option` because the
  overwrite map wins; trusted devices cannot skip approval on the Client.

## Gate 4 — Tefway server

`src/tefway.rs` is the single place the distribution values live:

| Setting | Value |
| --- | --- |
| Rendezvous (ID) | `remote.tefway.com.br` (default port 21116) |
| Relay | `remote.tefway.com.br` (default port 21117) |
| Public key | `3KBIfqldtiCmJkAl2O6rammBCLK4iRiTH3vQdRXoEEs=` |
| API server | empty — no API, and a stored value is overridden |

They are inserted into `OVERWRITE_SETTINGS`, which outranks the user config, so
they cannot be edited in the UI or a config file. No fallback to
`rs-ny.rustdesk.com` is possible: the public server is only used when
`custom-rendezvous-server` is empty, and it never is. The RustDesk server itself
was not touched, and no private key material is in the client.

## Gate 5 — Packaging

`res/tefway/package.py` (profiles × formats) plus templates in
`res/tefway/pkg/`. Per-profile package names, binaries and units:
`tefway-remote-master` / `tefway-remote-client`, exe names
`TefwayRemoteMaster` / `TefwayRemoteClient`. The two packages conflict with each
other and with `rustdesk`.

Documents: `docs/build/TEFWAY-REMOTE-{WINDOWS,FEDORA,DEBIAN,APPIMAGE}.md`.

## Gate 6 — Tests and review

See `docs/handoff/TEFWAY-REMOTE-RELEASE-HANDOFF.md` for the exact commands and
the result of each. Summary: Rust formatting clean, unit tests for the profile
policy added, static analysis of the Rust crate run on Windows. Flutter analysis
and any Flutter build are blocked on the git-ignored generated bridge, which CI
produces. No platform build was produced in this window.

## Known limits

1. Technician exclusivity is **not** implemented. Any peer that knows the ID can
   request a session; the end user still decides, but nothing proves the peer is
   a Tefway technician. Needs `TEFWAY-REMOTE-AUTH-01`.
2. Icon frames above 128 px are upscaled from the supplied ICO.
3. No installer/MSI branding pass, no code signing.

## Correções de closeout (após o gate)

A revisão de closeout do commit `1cf96f572` corrigiu bugs nos scripts de
empacotamento que impediam a geração dos artefatos experimentais:

- `res/tefway/package.py`:
  - `build()` copiava a DLL do virtual display para o diretório do bundle
    Windows em vez de um arquivo dentro dele (`IsADirectoryError` no Windows).
  - `package_deb()` gravava os arquivos de controle fora de `DEBIAN/` e chamava
    `dpkg-deb -b` no diretório errado, quebrando o Debian DEB e, por derivação,
    o AppImage.
- `res/tefway/pkg/{postinst.in,tefway-remote.service.in,tefway-remote.spec.in}`:
  o binário do bundle é renomeado de `rustdesk` para `<exe>` (TefwayRemoteMaster/
  TefwayRemoteClient), mas symlinks e o unit systemd ainda apontavam para
  `rustdesk`. Corrigido para `@BIN@`. O spec do RPM agora declara conflito com o
  outro pacote Tefway, igual ao .deb.
- `docs/build/TEFWAY-REMOTE-DEBIAN.md`: caminho do unit systemd ajustado para
  `<bin>`.

Nenhum build de pacote foi executado nestas correções. A geração de Windows EXE,
Fedora RPM, Debian DEB e Linux AppImage fica a cargo do próximo executor, usando
a toolchain validada em `docs/build/`.
