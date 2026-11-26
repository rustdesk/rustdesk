# Desktop MCP server

Enable the MCP server in desktop Settings → General. Choose **Export MCP client config**, save the JSON to a new file in a private local folder, and import its `mcpServers.rustdesk` entry into a client supporting Streamable HTTP. RustDesk creates the file with restricted permissions and keeps the configuration off the system clipboard. The endpoint defaults to `http://127.0.0.1:21119/mcp` and requires a Bearer token. It accepts loopback Host/Origin values only. The server is disabled by default.

## Token storage and regeneration

The Bearer token is stored as a plain-text local option, `mcp-server-token`, in `RustDesk_local.toml` in RustDesk's configuration directory. It is not stored in the operating system credential store. Keep that file and exported client configurations private because both contain the token. It authenticates access to this controller's MCP service, not to a remote machine.

**Regenerate token** stops the MCP server and closes existing server connections. RustDesk writes a new token, verifies it against both the saved and effective settings, and restarts the service if enabled. After success, export a fresh configuration for each client that should retain access. If verification fails, the service stays stopped and reports an error. The previous token may still work after you restart RustDesk. Correct the local settings and regenerate again to revoke it. Remote RustDesk passwords are separate.

## Connection and takeover approval

Two settings control separate prompts:

- **Automatically approve new connections and initial agent control** (`allow-mcp-auto-approve-connect`): skip the controller's prompt when opening a new remote session. Approval includes the new session's initial agent control; the connection is not read-only. The peer's password, two-factor authentication and incoming acceptance requirements still apply.
- **Automatically approve agent takeover of existing sessions** (`allow-mcp-auto-approve`): skip the prompt for `request_control` on a session already open under human control. It does not approve new connections.

Both default to off. For existing installations, the original `allow-mcp-auto-approve` value continues to govern takeover only; the new connection setting remains off until explicitly enabled. New-connection approval can therefore reappear after an upgrade even if the old combined setting was enabled. Taking control back always cancels the agent grant; these settings do not prevent human takeover.

An unanswered connection or takeover prompt expires after 60 seconds and reports an approval timeout, distinct from an explicit denial. Tool calls wait for at most about 25 seconds at a time; if approval is still pending, call the same tool again to keep waiting on that request.

## Screenshots and pointer coordinates

`get_session` and `list_sessions` report `screenshot_supported` and `screenshot_displays` for each ready session. Choose a display index from `screenshot_displays` for `screenshot` or `screenshot_after_ms`. These indices include the monitors shown in other windows of the same connection. To capture another monitor, select it in a remote window. The default display index is `0`. On older peers, the controller relies on the monitor selections reported by the windows. If a monitor is added or capture changes during a request, a screenshot may still time out. Reselect the monitor and retry.

`screenshot` accepts an optional `scale` greater than zero and at most one, for example `{"session_id":"…","scale":0.5}`. The default, `1`, returns the original PNG. Scaling happens on the controller after capture: it reduces returned image bytes and model image-processing work, not the remote capture or transfer time.

The result reports the returned image's `width` and `height`, the original `remote_width` and `remote_height`, and `coordinate_scale_x` / `coordinate_scale_y`. Mouse and scroll coordinates always use **original remote display pixels**, measured from that display's top-left corner. Multiply coordinates observed in the scaled image by these factors to map them back. Use the reported factors rather than simply dividing by `scale`, because integer image dimensions are rounded. Regions/cropping are not supported.

RustDesk maps those original pixels to the selected monitor's input coordinates, including per-monitor scaling on Wayland. Screenshots attached through `screenshot_after_ms` use the original resolution. If an input succeeds and its screenshot is unavailable, the tool reports both outcomes.

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
