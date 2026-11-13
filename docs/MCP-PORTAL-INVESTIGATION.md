# Wayland portal approval and first-connection reset

## Local reproduction

The dedicated Ubuntu 26.04.1 ARM64 / GNOME 50.1 Wayland peer was run with its system RustDesk service temporarily stopped and a single user `--server` process. The macOS controller used the previously verified local Wayland-text-fix build and a permanent peer password, submitted through MCP. Neither binary contained a portal timeout fix. The peer input/server build was the 1.5.3 package from run 43158980474; relevant source paths were compared with tag 1.5.1.

Two distinct failure sequences were captured:

| Peer local time (+08:00) | Event |
| --- | --- |
| 13:02:24.214576 | Connection #1738 opened |
| 13:03:15.234586 | Portal Start succeeded and returned streams |
| 13:03:15.728273 | Peer closed #1738 with `Timeout` |
| 13:03:16.734912 | Reconnection #1739 opened |
| 13:03:16.807946 | Portal Start succeeded again |

The controller log independently recorded processing PeerInfo and then `Reset by the peer`. Peer and controller clocks were not calibrated closely enough to compare subsecond timestamps across machines; the **493.687 ms** Start-to-Timeout interval above uses only the peer clock. The same GUI session UUID subsequently reached `ready` on the new connection.

A longer wait captured the other ordering:

| Peer local time (+08:00) | Event |
| --- | --- |
| 13:05:33.182142 | Connection #1162 opened |
| 13:06:34.190953 | Connection #1163 opened before approval completed |
| 13:06:42 | Portal approval completed; old #1162 reported `Reset by the peer` at 13:06:42.187385 |

The two incoming connection openings were **61.009 seconds** apart. Earlier isolated waits also repeatedly produced approximately 61-second reconnect spacing without a server PID change. Only one MCP `connect` initiated each case; later reconnects were native RustDesk behavior. The existing session ID was reused, not a newly opened MCP window.

The temporary service stop removes service-driven process replacement as a necessary cause. It does not prove that all service-managed instances or transports behave identically.

## Cause of the captured post-approval Timeout

`src/server/connection.rs` records `last_recv_time` before awaiting `on_message`. During login, that handler sets `authorized = true`, clears `unauthorized_id`, then waits for `display_service::update_get_sync_displays_on_login()`. This includes the portal interaction. The same connection task cannot process its ordinary receive and heartbeat branches while awaiting that handler.

After portal completion, login sends PeerInfo and returns to the main loop. The overdue receive watchdog can run before an incoming reply is read and see the receive timestamp from before the portal wait. In the captured #1738 case, this closes an otherwise newly initialized connection with `Timeout`. The other direct `Timeout` paths (unauthorized eviction and login deadline) are disabled once authorization has reached this stage. This identifies the stale receive watchdog by the source path and exact close reason; the internal `last_recv_time` itself was not instrumented in the installed binary.

The ordinary controller also has a receive watchdog. Its condition is 30 seconds, checked on a 30-second interval, so the detected disconnect need not occur exactly 30 seconds after the last packet. Approximately 60 seconds plus the ordinary reconnect delay is consistent with the measured 61-second incoming-connection spacing. A controller can abandon its old connection while the peer remains blocked in portal initialization; after approval the peer resumes reading and only then observes EOF. Approval is when that older closure is observed, not necessarily when it was initiated.

Relevant authorization, display-wait and timeout code also exists in stock 1.5.1 source. These paths were not introduced by the MCP integration. This is source attribution, not a claim that a separately identified stock controller binary passed through a complete runtime A/B.

## Repair boundary

There are two requirements for a complete ordinary-peer repair:

- While waiting for portal interaction, continue exchanging the existing bidirectional `TestDelay` heartbeat and process actual incoming activity. This prevents the controller watchdog from expiring and avoids letting the recent-session receive timestamp age solely because the peer is blocked.
- After waiting, use actual receive freshness when returning to the main loop. A successful portal response or an outbound PeerInfo is not evidence of received traffic. Returning to an already-stale watchdog must not immediately close the newly initialized connection.

No protocol extension is inherently required: ordinary clients already reply to `TestDelay` before PeerInfo. However, an implementation must preserve non-heartbeat message order and handle remote close, EOF, local cancellation and send errors while a shared portal operation is pending. Merely adding a detached heartbeat task does not work: existing outgoing channels are drained by the very main loop that is blocked. Globally increasing the timeout, extending password-cache lifetime, fabricating a fresh receive timestamp, or adding MCP retries would mask only parts of the behavior.

A narrower bounded liveness probe after a successful Linux Wayland login could fix the post-wait close, but would not fix controller timeout during a longer unanswered picker or establish temporary-password reuse. It must be presented as a partial peer fix, not a complete portal solution.

## Remaining verification and scope

The original tester's one-time-password failure has not been reproduced: these local cases deliberately used a permanent password. The earlier ordinary-GUI attempt stopped at `needs_password`; stale VM observations were not a valid portal-wait control. A later attempted quick-approval control did not establish an effective portal Start response within the intended interval and is not counted as a successful control.

No functional portal patch is included in this MCP PR. The captured failure has been narrowed to existing ordinary connection/login behavior, with a concrete peer-side repair boundary. Any implementation should be isolated and tested as an ordinary Linux peer correction, including successful delayed approval on the same connection, a silent/closed controller, cancellation, and preservation of early non-heartbeat messages. A clean ordinary-controller comparison and a temporary-password case remain useful before claiming the original incident fully explained.

Private diagnostics retain both sides' logs. This record omits credentials and portal restore tokens. No message has been posted to testers or maintainers on behalf of this investigation.
