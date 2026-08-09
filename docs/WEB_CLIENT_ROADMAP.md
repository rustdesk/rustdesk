# RustDesk OSS Web Client Roadmap

Upstream RustDesk removed the open-source web client in favor of a closed-source Pro offering. This roadmap tracks our effort to revive and maintain the web client against the OSS rendezvous server (hbbs/hbbr).

## Architecture Overview

```
Flutter Dart UI (93% shared with desktop/mobile)
       |
   web/bridge.dart (JS interop, 131 methods stubbed)
       |
   JS bridge: globals.js (setByName/getByName dispatch)
       |
   connection.ts (WebSocket + protobuf to hbbs/hbbr)
```

The web client shares ~68K lines of Dart with native clients. The web-specific layer is ~4K lines of Dart + ~1.6K lines of TypeScript/JavaScript. The JS layer reimplements the connection protocol over WebSocket; it does not compile Rust to wasm.

## Status

### Done (feat/revive-web-client)

- [x] Restore web client JS source from git history
- [x] Load server config (host/relay/key) from `config.json` instead of localStorage
- [x] Skip public server latency test and `refreshCurrentUser` for OSS server
- [x] Fix JS bridge: option handlers (`option:local`, `option:user:default`), `jsonfyForDart` double-encoding
- [x] Remove Firebase analytics
- [x] Fix JS build for modern Node.js/TypeScript
- [x] Make server settings UI read-only on web
- [x] Force English locale for web client
- [x] Add Dockerfile and docker-compose for containerized builds
- [x] Remove untested wss:// support (to revisit later)

### Phase 1: Testing Infrastructure

Establish test coverage as a foundation for opinionated development.

- [ ] **JS unit tests (vitest)**: Set up vitest in `flutter/web/js/`. Priority test targets:
  - `getrUriFromRs()` / `getDefaultUri()` — URI construction logic
  - `jsonfyForDart()` — payload serialization
  - `getByName`/`setByName` option handlers — defaults, read/write, server config blocking
  - `loadConfig()` — config.json parsing and fallback behavior
- [ ] **Flutter widget tests**: Extend existing `server_settings_dialog_test.dart` for `readOnly` parameter. Add tests for web-specific UI behavior (read-only server settings, close-only dialog).
- [ ] **CI gating**: Add `flutter test` and `npm test` (vitest) steps to the web build job in `flutter-build.yml`
- [ ] **E2E smoke test**: Playwright test that loads the web client, verifies it renders, and attempts a connection to a local hbbs/hbbr

### Phase 2: Connection Reliability

- [ ] Improve error handling for connection failures (clear user-facing messages)
- [ ] Handle server unreachable / timeout gracefully
- [ ] Reconnection logic on WebSocket drop
- [ ] Audit the 131 `UnimplementedError` stubs in `web/bridge.dart` — categorize as: needed, not-applicable-to-web, or deferred

### Phase 3: Features

- [ ] **WSS support**: Allow `wss://` endpoints for reverse proxy / TLS termination (was stripped from Phase 0, needs test infra first)
- [ ] **Auto-connect via URL params**: `?id=<peer_id>&pw=<password>` for embedded/kiosk use (inspired by MonsieurBiche fork)
- [ ] **Clipboard support**: Text clipboard sync between web client and remote
- [ ] **File transfer**: Basic upload/download (many bridge stubs to implement)
- [ ] **Mobile browser**: Responsive layout and touch input handling

### Phase 4: Operations

- [ ] CI/CD pipeline: auto-build Docker image on tag/release
- [ ] Helm chart or k8s manifests for deployment alongside hbbs/hbbr
- [ ] Health check endpoint or readiness probe
- [ ] Documentation: deployment guide, config.json reference, architecture diagram

## Non-Goals

- **Rust-to-wasm compilation**: The JS protocol reimplementation works. Porting to wasm would be a rewrite, not an incremental improvement.
- **Feature parity with native clients**: The web client is for quick remote access, not a full replacement. Features like screen recording, file manager, multi-monitor are out of scope.
- **Upstream merge**: This fork diverges intentionally from upstream's closed-source direction. We track upstream Dart/Flutter changes but own the web-specific layer.

## Community Forks

Reviewed for feature inspiration (none have strong traction):

| Fork | Stars | Status | Notable Features |
|------|-------|--------|-----------------|
| MonsieurBiche/rustdesk-web-client | 20 | Stale (Oct 2024) | WSS, auto-connect URL params, mobile fixes |
| pmietlicki/docker-rustdesk-web-client | 27 | Active | Docker packaging |
| linkzy/rustdesk-custom-web-client | 3 | New (2026) | Self-hosted relay focus |

## Key Files

| File | Purpose |
|------|---------|
| `flutter/web/js/src/connection.ts` | WebSocket connection to hbbs/hbbr |
| `flutter/web/js/src/globals.js` | JS bridge (setByName/getByName dispatch) |
| `flutter/lib/web/bridge.dart` | Dart-side bridge (131 stubs) |
| `flutter/lib/models/web_model.dart` | Web platform FFI |
| `flutter/lib/mobile/widgets/dialog.dart` | Server settings dialog (read-only on web) |
| `flutter/web/config.json` | Server config (host/relay/key) |
| `Dockerfile.web` | Containerized web build |
| `scripts/build-web.sh` | Build helper script |
