# Tabby vs Upstream: Fork Diff & Rebase Recommendation

Generated: 2026-04-29  
Merge base: `1abc897c` — "fix avatar fallback (#14458)" — 2026-03-05

---

## Divergence at a Glance

| | Commits | Date range |
|---|---|---|
| **Tabby (fork main)** | +34 ahead of base | 2026-03-05 → 2026-04-29 |
| **Upstream (rustdesk/master)** | +75 ahead of base | 2026-03-05 → 2026-04-29 |

---

## What Tabby Added (34 commits)

### New Custom UI Layer (`flutter/lib/custom/`)
All new files — zero overlap with upstream:

| File | Purpose |
|---|---|
| `custom/app_root.dart` | AppRoot with custom theme + dark mode |
| `custom/theme/app_theme.dart`, `tokens.dart` | AppTokens design tokens |
| `custom/screens/connect_screen.dart` | Connect screen with online/offline peer dots |
| `custom/screens/session_list_screen.dart` | Session list with bottom-nav shell |
| `custom/screens/remote_session_screen.dart` | Remote session wrapper |
| `custom/input/input_bridge.dart` | Input event bridge to RustDesk FFI |
| `custom/input/text_field_bridge.dart` | IME/text-field bridge |
| `custom/settings/settings_store.dart` | Wrapper around `mainGet/SetLocalOption` |
| `custom/strip/` (5 files) | PowerStrip keyboard: key cells, modifier state, default layout |

### Modifications to Upstream Files
These are the only files the fork touches that upstream also touched:

| File | Tabby's change |
|---|---|
| `flutter/lib/main.dart` | Added `CUSTOM_UI` flag to wire in custom screens |
| `flutter/lib/mobile/pages/remote_page.dart` | PowerStrip integration + two-finger gesture overrides |
| `flutter/lib/common/widgets/remote_input.dart` | Scroll redirect, gesture classifier, modifier timing |
| `flutter/lib/common.dart` | Minor routing change |
| `flutter/pubspec.yaml` / `pubspec.lock` | Added dependencies |

### iOS / Toolchain
- Custom Tabby app icons (all sizes replaced)
- `flutter/ios/exportOptions.plist` — Tabby bundle ID + signing
- `flutter/ios/Runner/Info.plist` — Tabby-specific config
- `rust-toolchain.toml` — pinned Rust 1.88.0 with iOS targets (`aarch64-apple-ios`)
- `.fvmrc` — Flutter 3.24.5 pin
- `scripts/build-ios.sh` — single-command iOS build

### Docs / Tooling
- `tabby-build-plan.md` (1405 lines) — project source of truth
- `SPIKE_NOTES.md` — Phase 0–3 recon + verification notes
- `UPGRADE.md` — upstream merge runbook
- `docs/tabby/deploy-testflight.md` — TestFlight deployment guide
- `.claude/skills/tabby-testflight/SKILL.md` — Claude skill
- `CLAUDE.md` — Extended with Tabby build context
- `.gitignore` additions (`logs/`, `.threadbase-uploads/`)
- `branding/` assets

---

## What Upstream Added (75 commits, not in fork)

### Security Fixes — High Priority

| Commit | Description | Files |
|---|---|---|
| `d4a1430` | **V-002 clipboard vulnerability** | `libs/clipboard/src/windows/wf_cliprdr.c` |
| `2f694c0` | **File transfer path traversal** (#14678) | `src/client/io_loop.rs`, `src/ui_cm_interface.rs` |
| `8dea347` | **Brute-force protection for one-time password** — rotates temp password after 10 failures (#14682) | `src/server/connection.rs` |
| `1705165` | **Store permanent password as hashed verifier** (#14619) | `src/server/connection.rs`, `src/ipc.rs`, `flutter/lib/common.dart` |
| `5ea6714` | Replace `unwrap()` in CLI password prompt | `src/` |

### iOS / iPad Fixes — Directly Relevant to Tabby

| Commit | Description |
|---|---|
| `590296b` | fix(iPad): mouse down detection for physical mouse (#14515) |
| `99b565e` | fix(iOS): preserve local pasteboard sync from Windows hosts (#14659) |
| `5b7ad33` | fix(iPad): keep touch gestures alive with external mouse (#14652) — **conflicts** |
| `b3f43f5` | fix(mobile): restore canvas offset after hiding soft keyboard (#14506) — **conflicts** |

### Flutter / Mobile Fixes

| Commit | Description |
|---|---|
| `c8ba99d` | flutter: fix shift/IME capitalization (#14695) — **conflicts pubspec** |
| `091f2c6` | impl(cm): `change_theme` and `change_language` callbacks (#14782) — **conflicts** |
| `1e9c4d0` | fix(mobile): disable deeplink by default (#14824) — **conflicts common.dart** |
| `0d3016f` | fix(flutter): reduce accidental horizontal trackpad scroll during vertical pan (#14460) |
| `b3f43f5` | fix(mobile): restore canvas offset after hiding keyboard (#14506) |
| `02da7132` | Fix note dialog not shown when closing reconnecting session (#14528) |
| `ac124c0` | Improve address book pull error handling (#14813) |
| `9f817714` | Stop retrying on restricted mobile access errors (#14797) |

### Features / Fixes (Rust + Backend)

| Commit | Description |
|---|---|
| `4e30ee8` | TCP proxy support (#14633) |
| `f02cd9c` | Fix Windows session-based logon + lock-screen detection (#14620) |
| `6cb3237` | fix(sciter): control side, privacy mode (#14880) |
| `5fd20f8` | Fix Safari OIDC flow (#14867) |
| `9d3bc7d` | Fix switch sides for macOS peers (#14661) |
| `bfd31d2` | Update `build.py` (#11341) |

### i18n (translations only — no logic)
Hindi, Malayalam, Gujarati, Arabic, Romanian, Belarusian, French, Japanese, Korean, Hungarian, Dutch, Polish, Russian, Turkish, Italian, Chinese (TW), German — new additions and corrections.

---

## Conflict Surface (5 files)

| File | Upstream change | Tabby change | Conflict complexity |
|---|---|---|---|
| `CLAUDE.md` | Deleted all content (replaced with agents.md pointer) | Extended with Tabby guidance | **Trivial** — keep Tabby's version |
| `flutter/pubspec.yaml` | Bumped IME dependency | Added custom deps | **Easy** — merge dep lists |
| `flutter/lib/common.dart` | Deeplink disable + password hash wiring | Minor routing change | **Low** — small, different hunks |
| `flutter/lib/common/widgets/remote_input.dart` | Touch gesture + theme callbacks | Scroll redirect + gesture classifier | **Medium** — same file, different code paths |
| `flutter/lib/mobile/pages/remote_page.dart` | iPad touch + canvas offset restore | PowerStrip injection + two-finger gesture | **Medium** — same widget build methods |

All other Tabby work lives in `flutter/lib/custom/` (new directory) — **zero upstream conflict**.

---

## Recommendation: Merge Upstream Now

**Yes, bring in upstream.** Use `git merge upstream/master` (not rebase) for these reasons:

### Why merge (not rebase)
- Rebasing 34 commits would replay every Tabby commit through 75 upstream commits, forcing conflict resolution at each step. A single merge commit resolves everything in one pass.
- Tabby's commit history is semantically meaningful (Phase 1 → Phase 3 progression, TestFlight builds); preserving it is cleaner than rewriting it.
- `UPGRADE.md` in the repo documents the merge workflow — use it.

### Why do it now
1. **Security**: Three meaningful security fixes upstream (path traversal, brute-force OTP, clipboard vuln, hashed password storage). These apply to the RustDesk server and Rust core — Tabby ships this server binary.
2. **iOS/iPad fixes**: `590296b` (mouse detection), `99b565e` (pasteboard sync), and `b3f43f5` (touch + external mouse) are directly in Tabby's feature domain. Picking them up before the gap widens is easier.
3. **Conflict surface is small and predictable**: 5 files, and 3 of them (`CLAUDE.md`, `pubspec.yaml`, `common.dart`) are low-effort resolutions. Only `remote_page.dart` and `remote_input.dart` need careful merging — and you already understand those files deeply from Phase 3 work.
4. **Gap grows over time**: 75 upstream commits in ~8 weeks. Every week without a merge adds ~10 more commits, increasing the chance that upstream refactors land in the conflict zone.

### Suggested workflow
```bash
# Follow UPGRADE.md runbook:
git fetch upstream
git checkout -b upgrade/upstream-$(date +%Y%m%d)
git merge upstream/master
# Resolve conflicts in:
#   CLAUDE.md                                  → keep Tabby's version
#   flutter/pubspec.yaml                       → merge dep lists
#   flutter/lib/common.dart                    → keep both hunks
#   flutter/lib/common/widgets/remote_input.dart → careful merge
#   flutter/lib/mobile/pages/remote_page.dart  → careful merge
flutter pub get && flutter build ios --debug
# Smoke-test: connect screen, session, power strip
```

### Risk: Low
- All Tabby-custom code is in `flutter/lib/custom/` — untouched by upstream.
- The two medium-complexity files (`remote_page.dart`, `remote_input.dart`) are well-understood; the upstream changes (canvas offset restore, external mouse coexistence) are additive to Tabby's gesture work, not contradictory.
