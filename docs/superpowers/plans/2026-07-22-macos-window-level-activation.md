# macOS Window-Level Activation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a remote left click raise the exact macOS window under the cursor, including a background window owned by the already-frontmost application.

**Architecture:** Keep the existing CoreGraphics owner-PID hit test and Rust call site. Before changing z-order, use public Accessibility hit testing to retain the exact `AXWindow`, compare it with the application's focused AX window, activate the application only when needed, and call `AXRaise` only when the target window differs from the focused window.

**Tech Stack:** RustDesk Rust input service, Objective-C++ AppKit/CoreGraphics/ApplicationServices bridge, Python fork-contract test, GitHub Actions macOS 14 candidate build.

## Global Constraints

- The deployment target remains macOS 12.3.
- Use only public APIs: `AXUIElementCopyElementAtPosition`, `kAXWindowAttribute`, `kAXFocusedWindowAttribute`, and `kAXRaiseAction`.
- Do not use `_AXUIElementGetWindow`, `CGSOrderWindow`, SkyLight, title matching, or geometry matching.
- Do not add dependencies, permissions, caches, retries, sleeps, background workers, or per-click info logging.
- Preserve current Dock exclusion, regular-application policy, application-level fallback, and mouse-down ordering.
- Keep RDH bundle IDs, launchd labels, process targets, configuration, and artifacts isolated from official RustDesk.
- Do not perform a local full application build; use `.github/workflows/codex-macos-herbin.yml`.
- Do not replace the installed RDH candidate until the official RustDesk rescue route is connected and verified.
- Keep `implementation-notes.md` untracked and update it only for real ambiguities, deviations, tradeoffs, or open questions.

---

### Task 1: Add the failing window-level activation contract

**Files:**
- Create, untracked: `implementation-notes.md`
- Modify: `tests/test_herbin_branding.py:76-89`
- Test: `tests/test_herbin_branding.py`

**Interfaces:**
- Consumes: the existing `macos_mm` source string loaded by `tests/test_herbin_branding.py`.
- Produces: a source-level contract requiring public AX hit testing, exact-window comparison, `AXRaise`, private-API absence, and activation ordering.

- [ ] **Step 1: Record the implementation baseline without staging it**

Create `implementation-notes.md` with exactly:

```markdown
# Implementation Notes

- Spec: `docs/superpowers/specs/2026-07-22-macos-window-level-activation-design.md`
- Ambiguities or deviations: None at implementation start.
- Tradeoffs: Use public Accessibility hit testing and fail back to existing app activation; no private API fallback.
- Open questions: None.
```

Confirm it remains untracked:

```bash
git status --short implementation-notes.md
```

Expected: `?? implementation-notes.md`.

- [ ] **Step 2: Extend the fork contract before production changes**

Immediately after the existing `activateWithOptions` assertions in `tests/test_herbin_branding.py`, add:

```python
    required_window_activation_markers = (
        "#import <ApplicationServices/ApplicationServices.h>",
        "AXUIElementCopyElementAtPosition",
        "kAXRoleAttribute",
        "kAXWindowRole",
        "kAXWindowAttribute",
        "AXUIElementGetPid",
        "kAXFocusedWindowAttribute",
        "CFEqual",
        "AXUIElementPerformAction",
        "kAXRaiseAction",
    )
    for marker in required_window_activation_markers:
        assert marker in macos_mm

    private_window_api_markers = (
        "_AXUIElementGetWindow",
        "CGSOrderWindow",
        "SkyLight",
    )
    for marker in private_window_api_markers:
        assert marker not in macos_mm

    assert_in_order(
        macos_mm,
        "AXUIElementCopyElementAtPosition",
        "activateWithOptions",
    )
    assert_in_order(
        macos_mm,
        "activateWithOptions",
        "AXUIElementPerformAction",
    )
```

- [ ] **Step 3: Run the focused test and verify RED**

Run:

```bash
python3 tests/test_herbin_branding.py
```

Expected: FAIL with an `AssertionError` because `#import <ApplicationServices/ApplicationServices.h>` or `AXUIElementCopyElementAtPosition` is absent from `src/platform/macos.mm`.

Do not change or weaken the test after observing this expected failure.

---

### Task 2: Raise the exact clicked AX window

**Files:**
- Modify: `src/platform/macos.mm:1-190`
- Modify: `docs/rdh-upgrade-runbook.md:6-13,100-113,151-164`
- Test: `tests/test_herbin_branding.py`

**Interfaces:**
- Consumes: `MacActivateApplicationAtPoint(double x, double y) -> int32_t`, called through the unchanged Rust FFI wrapper before `en.mouse_down(MouseButton::Left)`.
- Produces: `MacAccessibilityWindowAtPoint(double x, double y, pid_t expectedPid) -> AXUIElementRef`, returning a retained exact AX window or `NULL`; `MacAccessibilityWindowNeedsRaise(AXUIElementRef targetWindow, pid_t ownerPid) -> bool`; unchanged external C ABI and return-sign convention.

- [ ] **Step 1: Import the public Accessibility declarations**

Add after the AppKit import in `src/platform/macos.mm`:

```objective-c
#import <ApplicationServices/ApplicationServices.h>
```

- [ ] **Step 2: Add exact-window AX helpers before `MacActivateApplicationAtPoint`**

Insert the following after `MacWindowOwnerPidAtPoint`:

```objective-c
static AXUIElementRef MacAccessibilityWindowAtPoint(double x, double y, pid_t expectedPid) {
    AXUIElementRef systemWide = AXUIElementCreateSystemWide();
    if (systemWide == NULL) {
        return NULL;
    }

    AXUIElementRef hitElement = NULL;
    AXError hitError = AXUIElementCopyElementAtPosition(systemWide, x, y, &hitElement);
    CFRelease(systemWide);
    if (hitError != kAXErrorSuccess || hitElement == NULL) {
        if (hitElement != NULL) {
            CFRelease(hitElement);
        }
        return NULL;
    }

    AXUIElementRef windowElement = NULL;
    CFTypeRef roleValue = NULL;
    AXError roleError = AXUIElementCopyAttributeValue(hitElement, kAXRoleAttribute, &roleValue);
    if (roleError == kAXErrorSuccess && roleValue != NULL &&
        CFGetTypeID(roleValue) == CFStringGetTypeID() &&
        CFEqual(roleValue, kAXWindowRole)) {
        windowElement = hitElement;
        CFRetain(windowElement);
    }
    if (roleValue != NULL) {
        CFRelease(roleValue);
    }

    if (windowElement == NULL) {
        CFTypeRef windowValue = NULL;
        AXError windowError =
            AXUIElementCopyAttributeValue(hitElement, kAXWindowAttribute, &windowValue);
        if (windowError == kAXErrorSuccess && windowValue != NULL &&
            CFGetTypeID(windowValue) == AXUIElementGetTypeID()) {
            windowElement = (AXUIElementRef)windowValue;
        } else if (windowValue != NULL) {
            CFRelease(windowValue);
        }
    }
    CFRelease(hitElement);

    if (windowElement == NULL) {
        return NULL;
    }

    pid_t windowPid = -1;
    if (AXUIElementGetPid(windowElement, &windowPid) != kAXErrorSuccess ||
        windowPid != expectedPid) {
        CFRelease(windowElement);
        return NULL;
    }
    return windowElement;
}

static bool MacAccessibilityWindowNeedsRaise(AXUIElementRef targetWindow, pid_t ownerPid) {
    if (targetWindow == NULL) {
        return false;
    }

    AXUIElementRef applicationElement = AXUIElementCreateApplication(ownerPid);
    if (applicationElement == NULL) {
        return false;
    }

    CFTypeRef focusedWindowValue = NULL;
    AXError focusedWindowError = AXUIElementCopyAttributeValue(
        applicationElement, kAXFocusedWindowAttribute, &focusedWindowValue);
    CFRelease(applicationElement);
    if (focusedWindowError != kAXErrorSuccess || focusedWindowValue == NULL ||
        CFGetTypeID(focusedWindowValue) != AXUIElementGetTypeID()) {
        if (focusedWindowValue != NULL) {
            CFRelease(focusedWindowValue);
        }
        return false;
    }

    bool needsRaise = !CFEqual(targetWindow, focusedWindowValue);
    CFRelease(focusedWindowValue);
    return needsRaise;
}
```

Ownership contract: `MacAccessibilityWindowAtPoint` returns either `NULL` or one retained `AXUIElementRef`; the caller must release the non-null result exactly once. `MacAccessibilityWindowNeedsRaise` does not retain the caller's target.

- [ ] **Step 3: Replace only the body of `MacActivateApplicationAtPoint`**

Use:

```objective-c
extern "C" int32_t MacActivateApplicationAtPoint(double x, double y) {
    @autoreleasepool {
        int32_t targetPid = MacWindowOwnerPidAtPoint(x, y);
        if (targetPid <= 0) {
            return 0;
        }

        NSRunningApplication *application =
            [NSRunningApplication runningApplicationWithProcessIdentifier:targetPid];
        if (application == nil || application.terminated) {
            return -1;
        }
        if (application.activationPolicy != NSApplicationActivationPolicyRegular) {
            return 0;
        }

        AXUIElementRef targetWindow =
            MacAccessibilityWindowAtPoint(x, y, targetPid);
        bool shouldRaiseWindow =
            MacAccessibilityWindowNeedsRaise(targetWindow, targetPid);
        int32_t frontmostPid = MacFrontmostApplicationPid();

        bool activationSucceeded = true;
        if (targetPid != frontmostPid) {
            activationSucceeded =
                [application activateWithOptions:(NSApplicationActivationOptions)0];
        }

        AXError raiseError = kAXErrorSuccess;
        if (shouldRaiseWindow) {
            raiseError = AXUIElementPerformAction(targetWindow, kAXRaiseAction);
        }
        if (targetWindow != NULL) {
            CFRelease(targetWindow);
        }

        if (!activationSucceeded || raiseError != kAXErrorSuccess) {
            return -targetPid;
        }
        return targetPid != frontmostPid || shouldRaiseWindow ? targetPid : 0;
    }
}
```

Do not change `src/platform/macos.rs` or `src/server/input_service.rs`; their FFI and ordering already satisfy the design.

- [ ] **Step 4: Update the durable upgrade contract**

In `docs/rdh-upgrade-runbook.md`, change patch-contract item 2 to:

```markdown
2. A macOS controlled-side workaround that activates the regular application and,
   when public Accessibility data is available, raises the exact window under the
   cursor immediately before a remote left-button-down event.
```

Add these source-boundary checks after the existing activation check:

```markdown
- exact-window activation uses only public Accessibility APIs and validates the AX
  window PID against the CoreGraphics owner PID;
- the focused AX window is not raised again, and unsupported Accessibility paths
  fall back to the existing application activation without title/bounds guessing;
- `_AXUIElementGetWindow`, `CGSOrderWindow`, and SkyLight remain absent;
```

Add this runtime-acceptance item after cross-application clicking:

```markdown
- click between two windows of the same application and confirm the clicked window
  becomes frontmost without changing the menu-bar application;
```

- [ ] **Step 5: Run GREEN verification**

Run:

```bash
python3 tests/test_herbin_branding.py
python3 -m py_compile tests/test_herbin_branding.py
git diff --check
git diff -- src/platform/macos.mm tests/test_herbin_branding.py docs/rdh-upgrade-runbook.md
```

Expected: both Python commands exit 0, `git diff --check` prints nothing, and the diff contains no unrelated changes, private API marker, retry, delay, cache, or per-click info log.

- [ ] **Step 6: Update implementation notes if the code deviated**

If the exact implementation differs from the approved design or code above, replace the `Ambiguities or deviations` line in untracked `implementation-notes.md` with a concise reason. Otherwise leave `None at implementation start` unchanged.

- [ ] **Step 7: Commit the tested implementation without the notes file**

```bash
git add src/platform/macos.mm tests/test_herbin_branding.py docs/rdh-upgrade-runbook.md
git diff --cached --check
git commit -m "Raise clicked macOS window before remote input"
```

Expected: the commit contains exactly the three listed files; `implementation-notes.md` remains untracked.

---

### Task 3: Build and verify the rdh.5 candidate in CI

**Files:**
- Read: `.github/workflows/codex-macos-herbin.yml`
- Read: `docs/rdh-upgrade-runbook.md`
- Artifact: `rustdesk-herbin-1.4.9-rdh.5-aarch64-adhoc`

**Interfaces:**
- Consumes: pushed branch `fork/rdh/1.4.9` and workflow inputs `source_ref=rdh/1.4.9`, `rdh_revision=5`.
- Produces: a verified ad-hoc-signed arm64 DMG, checksum, metadata, and GitHub Actions run URL tied to the implementation commit.

- [ ] **Step 1: Re-run source verification immediately before push**

```bash
python3 tests/test_herbin_branding.py
git diff --check
git status --short --branch
git log -4 --oneline --decorate
```

Expected: tests pass; only `implementation-notes.md` is untracked; the implementation commit is HEAD.

- [ ] **Step 2: Push without force**

```bash
git push fork rdh/1.4.9
```

Expected: a normal fast-forward update; never use `--force`.

- [ ] **Step 3: Dispatch rdh.5**

```bash
gh workflow run codex-macos-herbin.yml \
  --repo Herbin-s/rustdesk \
  --ref master \
  -f source_ref=rdh/1.4.9 \
  -f rdh_revision=5

gh run list \
  --repo Herbin-s/rustdesk \
  --workflow codex-macos-herbin.yml \
  --event workflow_dispatch \
  --limit 3 \
  --json databaseId,status,conclusion,createdAt,url,headSha
```

Record the newly created run ID as `RUN_ID` and its URL in `implementation-notes.md`. The run's `headSha` identifies the `master` revision that supplied the workflow file, not the separately checked-out `source_ref`; prove the candidate source later from `rdh-build-metadata.txt` instead.

- [ ] **Step 4: Wait for the exact run and inspect failures if any**

```bash
gh run watch "$RUN_ID" --repo Herbin-s/rustdesk --exit-status
gh run view "$RUN_ID" \
  --repo Herbin-s/rustdesk \
  --json status,conclusion,url,headSha,jobs
```

Expected: `status=completed`, `conclusion=success`, and no failed job. Do not use `headSha` as candidate-source evidence. If the run fails, inspect only that run with `gh run view "$RUN_ID" --log-failed`; do not install anything.

- [ ] **Step 5: Download and verify the artifact**

```bash
ARTIFACT_ROOT="$HOME/Library/Caches/RustDesk-Herbin/rdh.5-run-$RUN_ID"
mkdir -p "$ARTIFACT_ROOT"
gh run download "$RUN_ID" \
  --repo Herbin-s/rustdesk \
  --name rustdesk-herbin-1.4.9-rdh.5-aarch64-adhoc \
  --dir "$ARTIFACT_ROOT"

cd "$ARTIFACT_ROOT"
shasum -a 256 -c rustdesk-herbin-1.4.9-rdh.5-aarch64.dmg.sha256
cat rdh-build-metadata.txt
test "$(awk -F= '$1 == "source_commit" {print $2}' rdh-build-metadata.txt)" = \
  "$(git -C /Users/herbin/.config/superpowers/worktrees/rustdesk/rdh-1.4.9 rev-parse HEAD)"
```

Expected: checksum `OK`, `upstream_version=1.4.9`, `rdh_revision=5`, `signature=ad-hoc`, `notarized=false`, and exact source commit equality.

- [ ] **Step 6: Inspect the mounted candidate without installing**

```bash
MOUNT_ROOT="$HOME/Library/Caches/RustDesk-Herbin/rdh.5-mount-$RUN_ID"
mkdir -p "$MOUNT_ROOT"
hdiutil attach -nobrowse -readonly \
  -mountpoint "$MOUNT_ROOT" \
  "$ARTIFACT_ROOT/rustdesk-herbin-1.4.9-rdh.5-aarch64.dmg"

CANDIDATE_APP="$MOUNT_ROOT/RustDesk-Herbin.app"
test "$(defaults read "$CANDIDATE_APP/Contents/Info" CFBundleIdentifier)" = "com.herbin.rustdesk"
test "$(defaults read "$CANDIDATE_APP/Contents/Info" CFBundleName)" = "RustDesk-Herbin"
codesign --verify --deep --strict --verbose=4 "$CANDIDATE_APP"
file "$CANDIDATE_APP/Contents/MacOS/RustDesk-Herbin"
strings "$CANDIDATE_APP/Contents/Frameworks/liblibrustdesk.dylib" | \
  rg 'AXRaise|AXFocusedWindow|AXWindow|activateWithOptions:'
hdiutil detach "$MOUNT_ROOT"
```

Expected: arm64 executable, valid ad-hoc deep signature, isolated bundle identity, and all four window-activation strings present.

---

### Task 4: Install transactionally and perform live acceptance

**Files:**
- Replace after rescue gate: `/Applications/RustDesk-Herbin.app`
- Preserve: `/Applications/RustDesk.app`
- Preserve: `/Library/LaunchAgents/com.carriez.RustDesk_server.plist`
- Preserve: `/Library/LaunchDaemons/com.carriez.RustDesk_service.plist`
- Reuse: `/Library/LaunchAgents/com.herbin.RustDesk-Herbin_server.plist`
- Backup: `$HOME/Library/Caches/RustDesk-Herbin/rollback-rdh.4-<timestamp>/RustDesk-Herbin.app`

**Interfaces:**
- Consumes: the verified rdh.5 DMG and a proven official RustDesk rescue connection.
- Produces: installed rdh.5 with a launchd-supervised `--server`, exact-window focus behavior, and a bounded rollback path.

- [ ] **Step 1: Stop at the rescue gate**

Have the user connect through official RustDesk from the controller and confirm the official route is interactive. Then record both products without changing them:

```bash
UID_VALUE="$(id -u)"
OFFICIAL_SERVER_PID="$(launchctl print "gui/$UID_VALUE/com.carriez.RustDesk_server" | awk '/pid =/ {print $3; exit}')"
OFFICIAL_SERVICE_PID="$(pgrep -f '^/Applications/RustDesk.app/Contents/MacOS/service$' | head -1)"
RDH_SERVER_PID="$(launchctl print "gui/$UID_VALUE/com.herbin.RustDesk-Herbin_server" | awk '/pid =/ {print $3; exit}')"
printf 'official_server=%s official_service=%s rdh_server=%s\n' \
  "$OFFICIAL_SERVER_PID" "$OFFICIAL_SERVICE_PID" "$RDH_SERVER_PID"
```

Do not continue unless all three values are non-empty and the user confirms the official connection.

- [ ] **Step 2: Stage the candidate and prepare the rollback directory before stopping RDH**

```bash
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
ROLLBACK_ROOT="$HOME/Library/Caches/RustDesk-Herbin/rollback-rdh.4-$TIMESTAMP"
STAGE_ROOT="$(mktemp -d "$HOME/Library/Caches/RustDesk-Herbin/rdh.5-stage.XXXXXX")"
mkdir -p "$ROLLBACK_ROOT"
codesign --verify --deep --strict --verbose=4 /Applications/RustDesk-Herbin.app

MOUNT_ROOT="$HOME/Library/Caches/RustDesk-Herbin/rdh.5-install-$RUN_ID"
mkdir -p "$MOUNT_ROOT"
hdiutil attach -nobrowse -readonly \
  -mountpoint "$MOUNT_ROOT" \
  "$ARTIFACT_ROOT/rustdesk-herbin-1.4.9-rdh.5-aarch64.dmg"
ditto "$MOUNT_ROOT/RustDesk-Herbin.app" "$STAGE_ROOT/RustDesk-Herbin.app"
hdiutil detach "$MOUNT_ROOT"
codesign --verify --deep --strict --verbose=4 "$STAGE_ROOT/RustDesk-Herbin.app"
```

Expected: the current and staged applications both verify before RDH is interrupted. The current application remains at `/Applications/RustDesk-Herbin.app` until the transaction moves it into the empty rollback directory.

- [ ] **Step 3: Replace only RDH and bootstrap its existing LaunchAgent**

Run this transaction from the official rescue connection:

```bash
UID_VALUE="$(id -u)"
RDH_LABEL="com.herbin.RustDesk-Herbin_server"
RDH_PLIST="/Library/LaunchAgents/com.herbin.RustDesk-Herbin_server.plist"

launchctl bootout "gui/$UID_VALUE/$RDH_LABEL"
pkill -f '^/Applications/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin( |$)' 2>/dev/null || true
mv /Applications/RustDesk-Herbin.app "$ROLLBACK_ROOT/RustDesk-Herbin.app"
mv "$STAGE_ROOT/RustDesk-Herbin.app" /Applications/RustDesk-Herbin.app
launchctl bootstrap "gui/$UID_VALUE" "$RDH_PLIST"

RDH_READY=0
for attempt in {1..20}; do
  if launchctl print "gui/$UID_VALUE/$RDH_LABEL" 2>/dev/null | rg -q 'state = running'; then
    RDH_READY=1
    break
  fi
  sleep 0.5
done
test "$RDH_READY" = 1
```

The bounded readiness poll is required because an immediate launchd check previously caused a false rollback. Do not unload, restart, or replace any `com.carriez` job or official application file.

- [ ] **Step 4: Roll back immediately if readiness fails**

If Step 3 exits nonzero, run only:

```bash
UID_VALUE="$(id -u)"
RDH_LABEL="com.herbin.RustDesk-Herbin_server"
RDH_PLIST="/Library/LaunchAgents/com.herbin.RustDesk-Herbin_server.plist"

launchctl bootout "gui/$UID_VALUE/$RDH_LABEL" 2>/dev/null || true
test -d /Applications/RustDesk-Herbin.app
FAILED_ROOT="$HOME/Library/Caches/RustDesk-Herbin/failed-rdh.5-$TIMESTAMP"
mkdir -p "$FAILED_ROOT"
mv /Applications/RustDesk-Herbin.app "$FAILED_ROOT/RustDesk-Herbin.app"
mv "$ROLLBACK_ROOT/RustDesk-Herbin.app" /Applications/RustDesk-Herbin.app
launchctl bootstrap "gui/$UID_VALUE" "$RDH_PLIST"
launchctl print "gui/$UID_VALUE/$RDH_LABEL" | rg 'state = running|pid ='
```

Then reconnect through restored RDH and stop. Do not delete the failed candidate or rollback evidence.

- [ ] **Step 5: Verify runtime isolation and loaded implementation**

```bash
UID_VALUE="$(id -u)"
NEW_RDH_SERVER_PID="$(launchctl print "gui/$UID_VALUE/com.herbin.RustDesk-Herbin_server" | awk '/pid =/ {print $3; exit}')"
test -n "$NEW_RDH_SERVER_PID"
test "$NEW_RDH_SERVER_PID" != "$RDH_SERVER_PID"

launchctl print "gui/$UID_VALUE/com.herbin.RustDesk-Herbin_server" | \
  rg 'state = running|pid =|XPC_SERVICE_NAME'
plutil -extract KeepAlive raw /Library/LaunchAgents/com.herbin.RustDesk-Herbin_server.plist
codesign --verify --deep --strict --verbose=4 /Applications/RustDesk-Herbin.app
lsof -p "$NEW_RDH_SERVER_PID" | rg '/Applications/RustDesk-Herbin.app/.*/liblibrustdesk.dylib'

test "$(launchctl print "gui/$UID_VALUE/com.carriez.RustDesk_server" | awk '/pid =/ {print $3; exit}')" = "$OFFICIAL_SERVER_PID"
test "$(pgrep -f '^/Applications/RustDesk.app/Contents/MacOS/service$' | head -1)" = "$OFFICIAL_SERVICE_PID"
```

Expected: new RDH server PID, boolean `KeepAlive=true`, dylib loaded from the RDH bundle, valid signature, and unchanged official server/service PIDs.

- [ ] **Step 6: Perform live same-app and cross-app acceptance**

Reconnect through RDH rdh.5, then:

1. Click the left ChatGPT window and confirm it moves above the larger ChatGPT window while the menu-bar application remains ChatGPT.
2. Click the larger ChatGPT window and confirm it moves back above the left window.
3. Repeat the round trip three times; each click must raise only the clicked window.
4. Click Chrome, WeChat, Finder, and ChatGPT in turn; each application must become frontmost as before.
5. Click repeatedly inside the already-focused WeChat window; confirm no window-order churn, termination, or crash report.
6. Exercise drag, text selection, right-click, scroll, and double-click once to confirm the pre-click hook did not alter other input paths.

Record the exact observed pass/fail result in `implementation-notes.md`.

- [ ] **Step 7: Recheck process health after acceptance**

```bash
ps -axo pid=,ppid=,user=,rss=,etime=,command= | \
  rg '/Applications/(RustDesk-Herbin|RustDesk)\.app/Contents/MacOS/(RustDesk-Herbin|RustDesk)( --server| --cm)?$|/Applications/(RustDesk-Herbin|RustDesk)\.app/Contents/MacOS/service$'
launchctl print "gui/$(id -u)/com.herbin.RustDesk-Herbin_server" | \
  rg 'state = running|pid =|last exit code|XPC_SERVICE_NAME'
find "$HOME/Library/Logs/DiagnosticReports" -maxdepth 1 -type f -mmin -10 \
  \( -iname '*RustDesk*' -o -iname '*WeChat*' \) -print
```

Expected: RDH and official server processes healthy, no duplicate no-argument RDH GUI processes, no fresh RustDesk/WeChat crash report, and RDH memory below the 1024 MiB restart threshold.

- [ ] **Step 8: Close the candidate only after acceptance evidence**

If all acceptance checks pass, keep the rollback copy until the next normal-use session has completed. Report the implementation commit, CI run URL, artifact checksum, installed server PID, official PID preservation, and manual two-ChatGPT-window result. Do not open a PR to upstream and do not promote or tag the fork unless the user asks.
