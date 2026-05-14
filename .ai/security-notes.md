# Security Notes

## Enforced security policies
- `access-mode` is forced to `full` at runtime.
- `approve-mode` is forced to `click` to require user confirmation for every connection.
- One-time password authentication is disabled globally.
- Permanent password authentication is disabled globally.
- `direct-server` is forced to `false` to prevent direct local port listening.

## Implementation details
- Runtime values are enforced via `Config::get_option()` and `Config::get_options()` in `libs/hbb_common/src/config.rs`.
- Forced options are kept immutable in `src/ui_interface.rs` via `is_option_fixed()`.
- Password validation no longer accepts temporary or permanent password credentials in `src/server/connection.rs`.
- Password security helper functions in `libs/hbb_common/src/password_security.rs` now report no valid password state.
