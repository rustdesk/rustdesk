# Implementation Notes

## Objective
- Implement Etapa 1: substitute rendezvous_server, relay_server, and key with hardcoded values.
- Avoid runtime override by .toml values, executable-based license parsing, or dynamic fallback paths.

## Files changed
- `libs/hbb_common/src/config.rs`
  - Added `HARD_CODED_RENDEZVOUS_SERVER`, `HARD_CODED_RELAY_SERVER`, and `HARD_CODED_KEY` constants.
  - Replaced `Config::get_rendezvous_server` and `Config::get_rendezvous_servers` fallback chain with fixed hardcoded rendezvous server.
  - Added `Config::get_relay_server` returning a fixed hardcoded relay server.
- `src/common.rs`
  - Simplified `get_key` to always return the hardcoded key, ignoring executable license and config option sources.
- `src/rendezvous_mediator.rs`
  - Locked relay selection to `Config::get_relay_server` and ignored provided fallback values.
- `libs/hbb_common/src/websocket.rs`
  - Updated WebSocket relay port resolution to use the hardcoded relay server rather than config option.

## Risks / Notes
- The hardcoded placeholder values are not valid server endpoints or key material; they satisfy the Etapa 1 requirement but must be replaced with actual runtime values before real deployment.
- This stage preserves compilation and keeps the existing structure, while removing dynamic server/key selection logic in the targeted paths.

## Etapa 3-5 updates
- Forced `access-mode` to `full` and `approve-mode` to `click` at runtime through `Config::get_option()`.
- Disabled numeric one-time passwords and permanent passwords by returning `false` from `password_security` policy functions.
- Prevented logon-screen password fallback by ignoring `allow_permanent_password` in `validate_password()`.
- Forced `direct-server` to `false` and blocked runtime writes for these forced options.
- Updated Flutter option fixedness detection to treat these forced security settings as immutable.
