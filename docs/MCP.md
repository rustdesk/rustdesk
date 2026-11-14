# Desktop MCP server

Enable the MCP server in desktop Settings → General. Copy its client configuration into an MCP client supporting Streamable HTTP. The endpoint defaults to `http://127.0.0.1:21119/mcp`; it requires a Bearer token and accepts loopback Host/Origin values only. The server is disabled by default.

## Token storage and regeneration

The Bearer token is stored as a plain-text local option, `mcp-server-token`, in `RustDesk_local.toml` in RustDesk's configuration directory. It is not stored in the operating system credential store. Keep that file and copied MCP client configurations private; both contain the token. It authenticates access to this controller's MCP service, not to a remote machine.

**Regenerate token** stops the MCP server, generates and saves a replacement token, then restarts the service if enabled. The old token no longer authenticates new requests, and existing server connections are closed. Copy the updated client configuration to every MCP client that should retain access. Remote RustDesk passwords are separate and are not changed by this operation.

## Connection and takeover approval

Two settings control separate prompts:

- **Automatically approve new connections and initial agent control** (`allow-mcp-auto-approve-connect`): skip the controller's prompt when opening a new remote session. Approval includes the new session's initial agent control; the connection is not read-only. The peer's password, two-factor authentication and incoming acceptance requirements still apply.
- **Automatically approve agent takeover of existing sessions** (`allow-mcp-auto-approve`): skip the prompt for `request_control` on a session already open under human control. It does not approve new connections.

Both default to off. For existing installations, the original `allow-mcp-auto-approve` value continues to govern takeover only; the new connection setting remains off until explicitly enabled. New-connection approval can therefore reappear after an upgrade even if the old combined setting was enabled. Taking control back always cancels the agent grant; these settings do not prevent human takeover.

An unanswered connection or takeover prompt expires after 60 seconds and reports an approval timeout, distinct from an explicit denial. Tool calls wait for at most about 25 seconds at a time; if approval is still pending, call the same tool again to keep waiting on that request.

## Screenshot scaling

`screenshot` accepts an optional `scale` greater than zero and at most one, for example `{"session_id":"…","scale":0.5}`. The default, `1`, returns the original PNG. Scaling happens on the controller after capture: it reduces returned image bytes and model image-processing work, not the remote capture or transfer time.

The result reports the returned image's `width` and `height`, the original `remote_width` and `remote_height`, and `coordinate_scale_x` / `coordinate_scale_y`. Mouse and scroll coordinates always use **original remote display pixels**, measured from that display's top-left corner. Multiply coordinates observed in the scaled image by these factors to map them back. Use the reported factors rather than simply dividing by `scale`, because integer image dimensions are rounded. Regions/cropping are not supported.

Screenshots attached through `screenshot_after_ms` remain at original resolution.

## Remote session actions

`remote_action` runs `ctrl_alt_del`, `lock_screen` or `os_password`. It requires current agent control, keyboard permission and a non-view-only session, and shares the input queue and native grant checks with the input tools. Taking control back cancels queued actions.

- `ctrl_alt_del`: Windows requires the peer's reported SAS support (normally an installed/elevated service); on Linux this is the desktop's Ctrl+Alt+Del shortcut, not Windows SAS.
- `lock_screen`: request the existing Windows, macOS or Linux lock action.
- `os_password`: use the OS password already saved for this peer in the session toolbar. It is not accepted as a tool argument or returned to the agent. Windows/macOS peers version 1.5.1 or newer are required; older and other platforms are refused. It sends the existing `OSLogin` message, whose peer handler checks the login/lock screen before input. It never falls back to typing a password into an arbitrary focused application. Show the sign-in/unlock UI first; this action does not wake or click it for you.

An `ok` result means the action was sent, not that locking or login succeeded. The OSLogin handler silently ignores the request when the peer is not locked, and a fork reporting a supported version but lacking that handler may ignore it too. Check the resulting screen. The saved-password operation reuses the existing peer handler and its input scheduling; it does not add a new execution acknowledgement.

## Text input

Prefer `paste_text` for long text, code and PowerShell prompts when clipboard sharing and application paste are available. It replaces the remote clipboard. Use `type_text` when the application needs per-character input or cannot accept paste. A successful tool response confirms sending, not that the target application has consumed the text: inspect the application before submitting or sending more.

In `ssh.exe` running in the classic Windows console, prefer `type_text`: Ctrl+V can reach the remote terminal as literal `^V`. For applications with a different paste shortcut (including Linux terminals), use `type_text`, or set `paste: false` and invoke the application's paste shortcut or right-click paste separately.

The `modifiers` array on keyboard, mouse and scroll tools accepts `ctrl`, `shift`, `alt` and `command`; `win` and `meta` are aliases for `command` (the Windows/Command key). Aliases are normalized before sending and naming the same modifier more than once does not press it twice.

On Wayland, `type_text` sends text containing non-ASCII characters as one complete batch to avoid clipboard replacement between asynchronous pastes. It ignores `delay_ms` for that batch and normalizes CRLF/CR to LF; line breaks are pasted as text rather than sent as Enter keys. Some terminals execute pasted newlines immediately. To review a command first, input one line without a line break, verify it, then press Enter separately.
