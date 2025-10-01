# RustDesk Code Style and Conventions

## Rust Code Style
- **Edition**: Rust 2021
- **Minimum Version**: 1.75
- **Formatting**: Uses rustfmt with `wrap_comments = true` (configured in `libs/enigo/rustfmt.toml`)
- **Standard Rust Conventions**: Follow standard Rust naming conventions
  - `snake_case` for variables, functions, modules
  - `PascalCase` for types, traits, enums
  - `SCREAMING_SNAKE_CASE` for constants

## Flutter/Dart Code Style
- **Linter**: Uses `package:lints/recommended.yaml`
- **Disabled Rules**:
  - `non_constant_identifier_names: false` - Allows non-constant identifier names
  - `sort_child_properties_last: false` - No enforcement of child property ordering
- Configuration in `flutter/analysis_options.yaml`

## General Guidelines
- Commits should be small and independently correct (compile and pass tests)
- Add tests for bug fixes and new features
- Use Developer Certificate of Origin sign-off (`git commit -s`)
- Branch from master and rebase before submitting PR

## Project-Specific Patterns
- Configuration centralized in `libs/hbb_common/src/config.rs`
- Platform-specific code isolated in respective directories
- Feature flags for conditional compilation (hwcodec, vram, flutter, etc.)
- Custom protocol implementation for remote desktop communication
