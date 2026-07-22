# macOS Window-Level Activation Design

## Context

RDH currently fixes remote clicks that interact with a background macOS application without making that application frontmost. Before sending a remote left-button-down event, it finds the topmost CoreGraphics window at the cursor, resolves the owning process, and calls `NSRunningApplication.activateWithOptions` when that process is not already frontmost.

This loses window identity. Two windows from the same application have different `CGWindowID` values but the same process ID. When that application is already frontmost, RDH returns early and never raises the specific background window that was clicked. The current two ChatGPT windows reproduce this case.

## Goals

- Raise the specific macOS window selected by a remote left click, including a background window belonging to the already-frontmost application.
- Apply the behavior to regular macOS applications generally, not only ChatGPT.
- Preserve the existing application-level activation behavior when window-level Accessibility data is unavailable.
- Use only public macOS APIs and the Accessibility permission RDH already requires.
- Keep the input path synchronous, stateless, and free of retries, sleeps, caches, or background workers.

## Non-Goals

- Supporting minimized windows, windows on another Space, or windows that are not present in the on-screen CoreGraphics list.
- Reordering Dock, desktop, menu-bar, system-security, or non-regular application windows.
- Using private APIs such as `_AXUIElementGetWindow`, `CGSOrderWindow`, or SkyLight interfaces.
- Adding application-specific behavior or allow/deny lists.
- Forcing `AXMain` or `AXFocused` attributes when `AXRaise` plus the following mouse-down event is sufficient.

## Evidence

The current two ChatGPT windows have different CoreGraphics window IDs and resolve to different `AXWindow` objects even though both have the same PID and title. Both expose the public `AXRaise` action.

A read-only probe of all currently visible regular applications found usable AX windows and `AXRaise` support for ChatGPT, WeChat, Chrome, Zed, Ghostty, Claude, Mentat, X, and Finder. All processes remained alive after the probe. A controlled `AXRaise` against the already-frontmost WeChat window returned success, preserved PID and window ID, and produced no immediate crash report.

Thirty direct `AXUIElementCopyElementAtPosition` probes completed successfully with a median of 0.106 ms, p95 of 4.204 ms, and one cold maximum of 39.426 ms. The controlled `AXRaise` completed in less than 1 ms.

## Considered Approaches

### 1. Public Accessibility hit testing and `AXRaise` — selected

Use `AXUIElementCopyElementAtPosition` at the same cursor coordinates, obtain the containing `kAXWindowAttribute`, compare it with the application's `kAXFocusedWindowAttribute`, and execute `kAXRaiseAction` only when the clicked window is not already focused.

This identifies same-PID windows without title or geometry matching, uses documented APIs, and avoids unnecessary `AXRaise` actions on ordinary clicks.

### 2. Match CoreGraphics and Accessibility windows by title and bounds

This uses public APIs but is ambiguous for identical titles and similar geometry. It also introduces matching tolerances and more failure modes without improving the verified case.

### 3. Map or reorder windows with private APIs

`_AXUIElementGetWindow` and `CGSOrderWindow` can operate directly on a CoreGraphics window ID, but Apple does not document or guarantee them. They create macOS-upgrade, signing, and crash risks that are inappropriate for a primary RDH build.

## Chosen Data Flow

The existing Rust call site remains unchanged and still invokes the macOS helper immediately before `en.mouse_down(MouseButton::Left)`.

Inside the existing Objective-C++ helper:

1. Read the on-screen CoreGraphics window list with `kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements`.
2. Preserve the current alpha, bounds, cursor-containment, and Dock exclusion rules to resolve the target owner PID.
3. Before changing application or window order, call `AXUIElementCopyElementAtPosition` at the same coordinates.
4. Obtain the hit element's containing `kAXWindowAttribute` and verify its PID equals the CoreGraphics owner PID.
5. Create the owning application AX element and read `kAXFocusedWindowAttribute`.
6. Resolve `NSRunningApplication` and retain the existing regular-application policy check.
7. If the owner is not the frontmost application, call `activateWithOptions` as RDH does today.
8. If the target AX window is valid and differs from the application's previously focused AX window, perform `kAXRaiseAction` on that exact target window.
9. Release all retained Core Foundation objects before returning; do not cache AX elements across clicks.
10. Continue with the existing remote mouse-down event regardless of whether the optional window-level operation succeeds.

Capturing the target AX window before activation is required. Activating an application can change z-order, so a later coordinate hit test could resolve to a different window.

## Failure Handling

- If CoreGraphics finds no eligible owner, retain the existing no-op behavior.
- If the owner application is unavailable, terminated, or non-regular, retain the existing skip/failure behavior.
- If AX hit testing, `kAXWindowAttribute`, PID validation, or `kAXFocusedWindowAttribute` fails, perform only the existing application-level activation when needed. Do not guess by title or bounds.
- If the target is already the application's focused window, do not call `AXRaise`.
- If `AXRaise` fails, report the failure through the existing return/logging path and still deliver the mouse event.
- Do not retry, sleep, or switch to a private API fallback.

This makes unsupported or partially accessible applications behave no worse than the current RDH build.

## Testing Strategy

### Automated red-green contract

Extend the fork-specific macOS activation test before production changes so it fails until the implementation:

- uses `AXUIElementCopyElementAtPosition`;
- obtains `kAXWindowAttribute` and `kAXFocusedWindowAttribute`;
- validates the AX window owner PID against the CoreGraphics owner PID;
- compares target and focused windows before raising;
- uses `kAXRaiseAction` without any private API markers;
- preserves activation and window handling before the mouse-down call.

Run the focused fork test first and observe the expected failure, then implement the smallest production change that satisfies it.

### Build verification

- Run the focused fork test and relevant formatting/static checks locally.
- Use the existing `.github/workflows/codex-macos-herbin.yml` CI workflow for the macOS application build rather than a local full build.
- Verify bundle identity, source commit, architecture, signing mode, and existing RDH watchdog/LaunchAgent contracts from the CI artifact.

### Live acceptance

Install the candidate only while the official RustDesk rescue channel is available. Verify:

1. Remote-click each of the two current ChatGPT windows in turn and confirm the clicked window becomes frontmost within ChatGPT.
2. Remote-click between different applications and confirm application activation still works.
3. Confirm repeated clicks inside the already-focused window do not change window order unexpectedly.
4. Confirm WeChat remains healthy after ordinary remote clicks and that its single focused window does not receive unnecessary `AXRaise` actions.
5. Confirm RDH `--server` persistence, memory watchdog configuration, and official RustDesk processes remain unchanged.

## Rollback

Keep the currently installed RDH build and official RustDesk rescue channel available until live acceptance passes. If the candidate regresses focus, input latency, application stability, or background service health, restore the previous RDH application and LaunchAgent state using the established bounded rollback procedure.
