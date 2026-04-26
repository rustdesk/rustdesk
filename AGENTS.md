# RustDesk Guide

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## Development Commands

### Build

* `cargo run`
* `cargo build --release`
* `cargo build --features hwcodec`
* `python3 build.py --flutter`
* `python3 build.py --flutter --release`
* `python3 build.py --hwcodec`
* `python3 build.py --vram`

### Flutter

* `cd flutter && flutter run`
* `cd flutter && flutter test`
* `cd flutter && flutter build android`
* `cd flutter && flutter build ios`

### Test

* `cargo test`

---

## Project Layout

* `src/` Rust app
* `src/server/` audio / clipboard / input / video / network
* `src/platform/` platform-specific code
* `src/ui/` legacy Sciter UI (deprecated)
* `flutter/` current UI
* `libs/hbb_common/` config / proto / shared utils
* `libs/scrap/` screen capture
* `libs/enigo/` input control
* `libs/clipboard/` clipboard

---

## Build Notes

### Dependencies

* Requires `vcpkg`
* Set `VCPKG_ROOT`
* Legacy UI needs Sciter library

### Ignore

* `target/`
* `flutter/build/`
* `flutter/.dart_tool/`

### Feature Flags

* `hwcodec`
* `vram`
* `flutter`
* `unix-file-copy-paste`
* `screencapturekit`

### Config

All options are in `libs/hbb_common/src/config.rs`

---

## Rust Rules

* Avoid `unwrap()` / `expect()` in production code.
* Exceptions:

  * tests;
  * lock acquisition where failure means poisoning, not normal control flow.
* Otherwise prefer `Result` + `?` or explicit handling.
* Do not ignore errors silently.
* Avoid unnecessary `.clone()`.
* Prefer borrowing when practical.
* Do not add dependencies unless needed.
* Keep code simple and idiomatic.

---

## Tokio Rules

* Assume a Tokio runtime already exists.
* Never create nested runtimes.
* Never call `Runtime::block_on()` inside Tokio / async code.
* Do not hide runtime creation inside helpers or libraries.
* Do not hold locks across `.await`.
* Prefer `.await`, `tokio::spawn`, channels.
* Use `spawn_blocking` or dedicated threads for blocking work.
* Do not use `std::thread::sleep()` in async code.

If errors like:

* `Cannot start a runtime from within a runtime`
* `Cannot block the current thread from within a runtime`

Treat them as design issues and refactor flow.

---

## Editing Hygiene

* Change only what is required.
* Prefer the smallest valid diff.
* Do not refactor unrelated code.
* Do not make formatting-only changes.
* Keep naming/style consistent with nearby code.
