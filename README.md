# RustDesk Lite Support (Windows) — Incoming-Only, Locked Settings

This kit gives you a **reproducible way to build a customized RustDesk Windows client** that:
- Accepts **incoming connections only** (no outgoing control).
- **Locks/hides server settings & key** so end‑users cannot change them.
- Starts **minimized to tray** with a minimal UI (ID + password).
- Can be built **locally** or via **GitHub Actions**.

> You’ll build from the official RustDesk source with a tiny feature gate. No RustDesk fork history needed — apply the patch, build, done.

---

## Quick Start (GitHub Actions)
1. **Fork** `github.com/rustdesk/rustdesk` to your GitHub.
2. **Create a branch** `lite-support`.
3. **Apply the patch** from this repo: `rustdesk-lite.patch` (root of this kit).
4. **Commit** the files under `.github/workflows/windows-lite.yml` and `branding/` too.
5. Push the branch. GitHub Actions will produce an artifact **`rustdesk-lite-win-x64.zip`**.
6. Unzip and run `RustDeskLite.exe`.

## Quick Start (Local Windows)
- Prereqs: Visual Studio C++ build tools, Rust toolchain, Flutter SDK.
- Run: `scripts\build_rustdesk_lite.ps1 -Relay your.relay:21117 -ID your.id.server:21116 -API https://your.api:21114 -Key YOUR_KEY`  
  The script will clone source, apply the patch, embed your servers/key, and build a portable `RustDeskLite.exe`.

> You can also provide `-Host chat.zont.uk` and the script will auto‑fill RustDesk ports 21114/21115/21116/21117 for Pro/OSS typical layouts.

---

## What the Patch Does
- Adds **feature flag** `incoming_only` in Rust & Dart.
- Hides **Connect** pages, toolbar, and quick connect box in Flutter when `incoming_only` is on.
- Adds **compile‑time default servers/key** (override/user settings are ignored).
- Locks Network Settings page (hidden & blocked) and forces **read‑only config**.
- Adds Windows start flags: start **minimized**, **tray‑first**, disable **auto‑update** UI.
- Introduces `RustDeskLite.exe` launcher that sets the feature gate and passes safe args.

You still get full RustDesk update cadence; the patch is tiny and future‑proof.

---

## Files in this kit
- `rustdesk-lite.patch` — patch to apply to RustDesk repo root.
- `scripts/build_rustdesk_lite.ps1` — one‑shot local builder.
- `.github/workflows/windows-lite.yml` — CI workflow for GitHub Actions.
- `branding/` — icon and name tweaks (safe defaults).
- `config/RustDesk2.toml.sample` — optional runtime config (will be ignored by the locked build, but useful for testing).

---

## Limitations / Notes
- **True setting lock & custom client generation** are first‑class **RustDesk Pro Console** features. This kit implements them **in OSS** by compile‑time defaults, UI removal, and file ACL hardening.
- “Incoming‑only” is enforced by UI removal and a runtime guard: attempts to start an **outgoing** session are blocked.
- To **revert to full client**, just build without the feature flag (`--no-default-features` disables incoming_only additions).

---

## Uninstall / Cleanup
- Delete the app folder. If you installed the service, run `rustdesk.exe --uninstall-service` in elevated PowerShell.
- Remove firewall rules named `RustDeskLite-Restrict` if added by the script.

