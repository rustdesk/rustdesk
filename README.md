# Tabby

A touch-optimized iOS remote desktop client, forked from [RustDesk](https://github.com/rustdesk/rustdesk).

Tabby replaces the standard RustDesk mobile UI with a custom interface designed for iPad and iPhone — dark-themed, gesture-first, and built around the **PowerStrip** keyboard for sending modifier key combinations to remote desktops.

Distributed via TestFlight. See `docs/tabby/deploy-testflight.md` for the deployment procedure, or invoke the `tabby-testflight` skill.

---

## What's custom

Everything under `flutter/lib/custom/` is Tabby-specific:

- **Connect screen** — direct ID entry with online/offline status on peer cards
- **Session list** — recent connections with bottom-navigation shell
- **PowerStrip keyboard** — on-screen strip for modifier keys (Ctrl/Alt/Cmd/Shift/Esc/Enter), arrows, and macros; adapts labels to the remote OS (Windows/macOS/Linux); supports left-handed mode and collapse
- **Input bridge** — IME integration, two-finger pan-vs-pinch classifier, modifier state tracking
- **App theme** — Material Design 3, dark-mode-only, custom `AppTokens`
- **SettingsStore** — Tabby-specific settings under the `tabby:` namespace

The Rust backend (`src/`, `libs/`) and Flutter FFI bindings are inherited unchanged from upstream RustDesk.

---

## Build

### Requirements

- Flutter 3.41.8 (use system `flutter`, not `fvm flutter` — FVM 3.24.5 is incompatible with current deps)
- Xcode with iOS SDK
- Rust with `aarch64-apple-ios` target

### iOS

```sh
cd flutter
flutter pub get
flutter build ipa --release --export-options-plist=ios/exportOptions.plist --no-pub
```

### TestFlight upload

```sh
xcrun altool --upload-app --type ios \
  -f flutter/build/ios/ipa/*.ipa \
  --apiKey G7S8Q6D6Z9 \
  --apiIssuer e84dd3bd-1e5d-4db4-8b57-46637e2510ff
```

API key: `~/.appstoreconnect/private_keys/AuthKey_G7S8Q6D6Z9.p8` (recover from 1Password if missing).

### Bump build number

Edit `flutter/pubspec.yaml`: increment `+N` in `version: 1.x.y+N`, then run `flutter pub get`.

---

## Project structure

```
flutter/lib/
  custom/          # Tabby UI — screens, PowerStrip, input bridge, theme
  mobile/          # Upstream RustDesk mobile (largely superseded by custom/)
  desktop/         # Upstream RustDesk desktop (unused on iOS)
  models/          # Shared state/models (upstream)
  common/          # Shared widgets (upstream)
src/               # Rust backend (upstream RustDesk)
libs/              # Core Rust libraries (upstream RustDesk)
docs/tabby/        # Tabby-specific docs (deployment, architecture)
```

---

## Upstream

This repo tracks [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk). Merges from upstream are done periodically — check git log for `chore: merge tabby/upgrade-*` commits.
