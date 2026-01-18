# Repository Guidelines

## Project Structure & Module Organization

- `src/`: main Rust application (client/server logic, platform glue).
- `libs/`: Rust workspace crates (e.g., `libs/hbb_common`, `libs/scrap`, `libs/enigo`, `libs/clipboard`).
- `flutter/`: Flutter UI for desktop + mobile; `flutter/test/` contains Dart tests.
- `res/`: runtime resources and packaging specs; `docs/`: user/dev docs and contribution info.
- Packaging/build helpers live in `build.py`, plus platform folders like `flatpak/`, `appimage/`, `fastlane/`.

## Build, Test, and Development Commands

- Clone with submodules: `git clone --recurse-submodules …` (or `git submodule update --init --recursive`).
- Rust (legacy Sciter UI): `VCPKG_ROOT=/path/to/vcpkg cargo run` (expects Sciter library under `target/debug/` per `README.md`).
- Rust release build: `cargo build --release`.
- Flutter desktop build: `python3 build.py --flutter` (add `--release` for optimized builds).
- Flutter dev loop: `cd flutter && flutter pub get && flutter run`.
- Tests: `cargo test` (Rust) and `cd flutter && flutter test` (Dart).

## Coding Style & Naming Conventions

- Rust: follow `rustfmt` defaults; prefer `cargo fmt` before PRs. Keep modules and filenames snake_case.
- Flutter/Dart: use standard Dart formatting (2-space indent); run `dart format .` (or `flutter format .`) and keep files `lower_snake_case.dart`.
- Config/state lives in `libs/hbb_common/src/config.rs`; prefer extending existing config types rather than adding new ad-hoc files.

## Testing Guidelines

- Add tests alongside the code you change when feasible: Rust `#[test]` unit tests or integration tests under `tests/` (if introduced), and Flutter tests as `flutter/test/*_test.dart`.
- Keep tests deterministic and platform-aware (many features are OS-specific).

## Commit & Pull Request Guidelines

- Commits in this repo commonly use prefixes like `feat:`, `fix(scope):`, and small focused “Update …” changes; keep commits independently buildable.
- PRs should be based on `master`, kept small, and include a clear description, linked issues, and screenshots for UI changes.
- Use DCO sign-off (`git commit -s`) as required by `docs/CONTRIBUTING.md`.

## Security & Configuration Tips

- Do not commit secrets/keys or server credentials; prefer local env vars and documented config paths.
- Avoid committing build artifacts (`target/`, `flutter/build/`, `flutter/.dart_tool/`).

## Agent-Specific Instructions

- 默认使用中文与我对话（除非我明确要求使用其他语言）。
