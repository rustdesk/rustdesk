# MCP validation record

Source tested: `82a1fbb1` (implementation `2ca89c32`, with the terminal-paste description corrected in `82a1fbb1`). Upstream master `420f91f6` is included through merge `0d2a4f57`. The later merge `79d02fc2` adds only upstream commit `d46c7c52`, a signing-URL expression correction in `.github/workflows/flutter-build.yml`; it does not change application sources.

## Mouse-modifier review follow-up

After the package/VM runs below, review found that MCP mouse input still used the human Ctrl/Command swap preference. The MCP sender now emits explicit modifiers directly while retaining the existing native ownership check; human mouse input keeps its original sender. A native regression covers Control and Meta on mouse-down and wheel events with swapping enabled, and confirms the human path still swaps them. It failed before the fix (`Meta` instead of `Control`); all 19 native MCP tests passed locally after the fix with `cargo test --locked --features flutter --lib flutter_mcp::tests -- --test-threads=1`. The package CI and VM results below predate this follow-up; no rebuilt package or fresh VM validation is claimed for it.

## Wayland Unicode follow-up, 2026-11-13

The delayed Unicode corruption was reproduced locally over the full MCP connection from a macOS ARM64 controller to Ubuntu 26.04.1 ARM64 / GNOME 50.1 Wayland. The unchanged peer runs the previously tested 1.5.3 package; its `rdp_input.rs` and `input_service.rs` match the local 1.5.1 tag. ASCII at 60 ms was exact, a single Unicode line at zero delay was exact, Unicode at 60 ms was reordered/duplicated, and zero-delay multiline Unicode also lost/repeated text. A separate component probe using the pinned arboard clipboard library reproduced next-character substitution and last-character duplication. The `MissingProtocol` / X11 fallback message originates in clipboard listening; it does not mean the entire Wayland desktop or keyboard input fell back to X11.

The controller now uses the peer's existing `is_wayland` metadata. For a Wayland call containing any non-ASCII character, it sends the entire normalized text as one `Seq` instead of repeatedly replacing the clipboard for individual characters or lines. `delay_ms` is ignored for this batch, and CRLF/CR become LF inside the pasted text; these line breaks are not sent as Enter key events. ASCII calls and non-Wayland peers keep their existing behavior. The tool description explicitly documents this contract and the possibility of terminal execution on pasted newlines.

A local macOS application build with the changed Dart code and the native library from CI run 43177802239 passed actual remote buffer comparisons for single-line Unicode requested at 60 ms, multiline/blank-line Unicode at 20 ms with mixed CRLF/CR/LF, multiline Unicode at zero delay, and ASCII multiline input at 20 ms. GNOME Text Editor 50.1 also saved all characters and blank lines correctly; its extra implicit final newline was separately confirmed using an ASCII control. [Before/after text evidence](mcp-validation/wayland-input.json) records the exact results. Forty-five focused Dart MCP tests pass, including a regression confirmed to fail before the batching change; analysis of all MCP modules in the isolated build reports no issues. No new packaged CI build is claimed for this follow-up.

This is a controller compatibility fix for the within-call clipboard replacement race, not a general acknowledgement protocol for peer paste completion. An already queued batch cannot be interrupted, and a later independent input call can still race asynchronous application delivery. Check application content before sending more text. The first-connect reset after portal approval remains unresolved.

## Second feedback round, 2026-11-13

This round adds optional controller-side screenshot scaling with explicit original-pixel coordinate mapping, separate new-connection and existing-session takeover approval settings, distinct approval-timeout messages, and PeterLinuxOSS's `remote_action` contribution adapted to the guarded `OSLogin` path. [MCP usage notes](MCP.md) document token storage/regeneration, approval migration, scaling, and remote-action semantics.

Local combined verification: 63 Dart MCP/approval/input/shortcut tests passed; native MCP tests passed 22/22, including the three remote-action tests. The screenshot test decodes an actual scaled PNG, checks its pixels and rounded dimensions, and confirms mouse coordinates remain original remote pixels. Approval tests use virtual time to exercise repeated `connect` calls through the 60-second expiry and distinguish explicit control cancellation from expiry. Analysis of MCP modules and constants reports no issues. Local Dart tests use ignored bridge development stubs; production bridges and packages must be regenerated in CI. No live OS unlock pass is claimed before testing those packages.

The saved-password action sends only `OSLogin` under the existing native grant lock; unsupported platforms/versions and absent saved passwords are rejected. Tests verify that Windows/macOS produce an OSLogin message and no ordinary password key events. Linux Ctrl+Alt+Del retains Control+Alt even with the human Ctrl/Meta swap enabled. This reuses the existing peer lock-screen guard and does not add login-success acknowledgement or strengthen that handler's scheduling guarantees.

### First-connect portal reset investigation

Source investigation found that the ordinary controller receive watchdog can expire while login waits for the Wayland portal. The native reconnect can therefore create a second peer connection before the first portal request returns. The peer only observes the old connection's EOF after its blocked login handler resumes. Authorization also rotates temporary passwords before portal initialization finishes, and recent-session reuse expires without receives. These facts explain a plausible timing chain, but do not prove the original tester's exact incident.

A fresh local MCP run with the restore token removed captured #1818 opening at 11:18:05, #1819 opening at 11:19:06 while approval was pending, then #1818 reporting `Reset by the peer` at 11:19:43 after sharing was selected. The original UI session later reached `ready`. The repeated MCP `connect` call reused the same session ID. A subsequent ordinary-GUI comparison was confounded by surviving native retries and user server/connection-manager restarts; it did not provide a valid cold-state A/B. These observations do not establish an MCP duplicate-open defect, nor a complete reproduction of the original one-time-password failure.

No speculative change was made to shared receive timeouts, portal teardown or session caching. The first-connect reset remains unresolved and requires a clean matched manual-versus-MCP trace. The local diagnostics are retained separately; credentials and portal restore tokens are not included in this record.

## Packages and CI

[Fork run 43158980474](https://github.com/faithleysath/rustdesk/actions/runs/43158980474) built commit `b8d1f239`: the tested source plus a test-only desktop workflow. No release publishing, signing or protocol changes were involved. Both Flutter bridge variants were regenerated by upstream's `bridge.yml`.

| Platform | Result |
| --- | --- |
| Windows x64, Flutter 3.24.5 | Package built; native MCP 18/18, full Rust library 271/271, clipboard 2/2, Dart MCP/shortcut 55/55 passed |
| Windows ARM64, Flutter 3.44.9 | Package built; used in the VM checks below |
| macOS ARM64, Flutter 3.44.9 | Package built; native MCP 18/18, full Rust library 273/273, clipboard 22/22, Dart MCP/shortcut 55/55 passed; full Dart suite 217 passed, one existing upstream skip |
| Linux ARM64 | DEB/RPM built; DEB used in the VM checks below |

The Windows and macOS test counts overlap: the focused MCP tests are also in the full library suite. The skipped Dart test is the existing web-clipboard activation test in `local_flutter_shortcuts_test.dart`.

Downloaded packages were checked against their artifact ZIP entry size and CRC, then SHA-256 before installation:

- Windows ARM64 MSI, artifact `13919736864`, 24,908,384 bytes: `6492043e959309888ad5ee3647995796dac02df433c4252f0a78135c496be4e6`.
- Linux ARM64 DEB, artifact `13918822258`, 23,688,708 bytes: `e8fec0a1f9f3896f80bd85bacfb6a8c3c36b2ec53ca6c5072a0d07dc91dca6dd`.

The installed Windows executable hashes to `1a930e4a2a9186faec6330ffb03041721e3b81b096df6b91bd3f6406ce92f128`; the installed Linux executable hashes to `835c4bb58cd9d8740bf71b142b11b25651ab415c88ceb4830ae318fa3d7fa980`.

## VM checks, 2026-11-12

The controller was Ubuntu 26.04.1 ARM64 with a GNOME Wayland session, running the tested DEB. The peer was Windows 11 24H2 ARM64, build 26100.4349. VMPal executed setup/observation commands and simulated local user actions; remote input under test went through RustDesk MCP. Each controller exposed 14 tools after installation.

### PowerShell text

With a baseline `79be65e8b` Ubuntu controller and the official RustDesk 1.5.3 Windows peer, Windows PowerShell 5.1.26100.4202 / PSReadLine 2.0.0 accepted `MCP baseline café 日本語` exactly. Thus the reported Windows Server empty-input problem was **not reproduced in this Windows 11 environment**.

With the tested packages, `paste_text` inserted `MCP paste café 日本語 😀` exactly into the same PowerShell prompt. A PSReadLine F8 handler recorded `GetBufferState` without submitting the buffer. The first capture immediately after the tool call was empty; a later capture after observing the completed paste matched exactly. This confirms why the tool describes asynchronous delivery and requires checking application content before submission. No test text was executed as a command.

The captured strings and timestamps are in [the text evidence](mcp-validation/text-evidence.json). Pasted newlines were deliberately excluded: a terminal may execute them immediately.

### Ownership and held input

After `release_control`, `paste_text`, mouse input and `disconnect` returned tool errors; screenshots still succeeded. `request_control` with auto-approval restored agent input. An invalid screenshot `display` string returned an argument error; dispatcher regression tests separately establish that input is not dispatched first.

A Windows `GetAsyncKeyState` logger observed a held Up key and left mouse button return to the released state after `release_control`. The [peer-side CSV](mcp-validation/agent-input-release.csv) records those transitions. The Windows clock was approximately 250 ms behind the Ubuntu clock, so cross-machine timestamps are approximate; event order on each machine is preserved. The shortest key transition lasted about 13 ms, while a separate mouse hold lasted about 11.5 seconds before release.

### Boundaries of this record

- Linux Legacy/Map human-key handoff and ordinary UI closure with held agent input remain unverified. After removing duplicate GUI processes, a final single-instance run connected successfully and returned to human mode. The keyboard-mode toolbar did not open its menu in the bounded GUI attempt, so neither keyboard path nor the UI-close scenarios was exercised. The test stopped with all observed keys/buttons released; it is not counted as a pass.
- Multi-monitor sibling-window closure, concurrent close/takeover/shutdown and actual Ctrl/Command swap behavior were covered by native regressions where applicable, but not by fresh VM fault injection.
- The initial Windows-to-Ubuntu Unicode comparison was inconclusive. The later macOS-to-Ubuntu run above reproduced the corruption and verified the within-call compatibility fix. The clipboard listener fallback was investigated separately; it is not evidence of a complete Wayland input fallback. The first-connect portal reset still needs a reliable reproduction.
- The original Windows Server / PSReadLine environment, clipboard-disabled runtime behavior, takeover during the paste delay, second-display coordinates and light/dark takeover-banner appearance have not been accepted in these VM runs. Native clipboard permission checks and the input queue cancellation behavior have automated coverage.
- These fork build results do not replace upstream CI approval or maintainer acceptance of the remaining runtime scope.
