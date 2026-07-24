# macOS Window Targeting Rules Design

## Status and Relationship to the Previous Design

This design supersedes the target-selection and Dock-exclusion portions of
`2026-07-22-macos-window-level-activation-design.md`.

The previous design remains authoritative for its use of public Accessibility
APIs, PID validation, concrete-window `AXRaise`, and unconditional delivery of
the original remote mouse event. Its assumption that all Dock and non-zero-layer
windows can be skipped is no longer valid.

## Context

RDH currently runs a macOS application/window activation helper immediately
before every remote left-button-down event. The helper enumerates on-screen
CoreGraphics windows, discards every window whose `kCGWindowLayer` is not zero,
discards Dock-owned windows, activates the selected regular application, and
raises the concrete Accessibility window when necessary.

The layer-zero filter fixed a full-screen Notification Center overlay that
otherwise hid the intended application from hit testing. It also introduced a
new class of click-through regressions:

- A Finder path-bar context menu can open normally on right-button-down, but
  selecting an item with the left button activates a window below the menu,
  dismisses the menu, and loses the command.
- A Finder preview/display-options popover can similarly dismiss and activate a
  lower window.
- Selecting **Quit** from a Dock application menu activates the window below the
  menu and does not quit the selected application.

A live read-only probe of the Dock reproduction established the complete
failure chain:

- the pointer was over the enabled `AXMenuItem` titled `退出`;
- the top CoreGraphics owner was `DockHelper`
  (`com.apple.dock.helper`), PID `64334`;
- the menu window was layer `101`, alpha `1.0`, and contained the pointer;
- the first layer-zero window at the same coordinate was ChatGPT, PID `80988`.

The production filter therefore skipped a real interactive menu, chose
ChatGPT, activated ChatGPT before the original mouse-down, and caused the menu
item to disappear before it could receive the click.

Window layer is useful evidence, but it is not sufficient to decide whether a
window should be penetrated.

## Goals

- Preserve application activation and concrete-window raising for normal remote
  clicks.
- Preserve interaction with menus, popovers, Dock UI, and other transient
  non-zero-layer UI without activating a lower window first.
- Continue through explicitly identified passive overlays such as the observed
  Notification Center full-display overlay.
- Make target-selection exceptions configurable without rebuilding RDH.
- Provide a deterministic raw-input baseline for debugging and A/B comparison.
- Validate and reload configuration without restarting RDH or interrupting the
  active remote connection.
- Keep configuration parsing and disk I/O out of the per-click hot path.
- Keep the feature macOS-only and independent from the official RustDesk rescue
  installation.

## Non-Goals

- Automatically switching from RDH to the official RustDesk application or
  handing an active connection between their services.
- Watching the configuration file for changes.
- Adding a graphical configuration editor in the first version.
- Adding Lua, JavaScript, plugins, automatic rule learning, random A/B
  assignment, or per-click experiment alternation.
- Replacing RustDesk's mouse transport or Enigo event injection.
- Consuming, synthesizing, retrying, or delaying the original mouse event.
- Using private CoreGraphics, Accessibility, CGS, or SkyLight APIs.
- Matching rules by PID, volatile window ID, localized window title, or regular
  expressions in the first version.

## Modes and Actions

### Runtime modes

`rules`
: Evaluate the effective rule set and optionally activate/raise the selected
  application window before delivering the original mouse event.

`passthrough`
: Skip window collection, rule matching, activation, and raising. Deliver the
  original mouse event through the existing upstream input path. This is the A/B
  baseline and an immediate behavioral rollback, not a switch to the installed
  official RustDesk application.

The mode is deterministic for an entire configuration generation. RDH must not
randomize or alternate modes because window focus and ordering are stateful.

### Rule actions

`skip`
: The candidate is a confirmed passive overlay. Continue evaluating the next
  CoreGraphics candidate under the pointer.

`forward_only`
: Stop candidate traversal. Do not activate or raise any application. Continue
  immediately to the original remote mouse event.

`activate`
: Select this candidate. Activate its owning regular application when needed
  and raise the concrete Accessibility window when available and different from
  the focused window.

## Architecture

### 1. macOS window collector

The Objective-C++ platform layer remains responsible for CoreGraphics,
`NSRunningApplication`, and Accessibility calls. It collects candidates in real
CoreGraphics Z order without pre-filtering by layer or Dock ownership.

For each bounded candidate record it exposes only the facts needed by the rule
engine:

- owner PID, bundle identifier, and process name;
- CoreGraphics window ID, layer, alpha, and bounds;
- application activation policy;
- whether the candidate covers the pointer's entire display;
- Accessibility hit PID, role, and subrole when the system-wide AX hit belongs
  to that candidate.

`covers_display` means that the candidate bounds cover the display containing
the pointer within one logical point on every edge. It is not the same as a
normal application's macOS full-screen state.

The Rust/Objective-C++ boundary uses bounded C-compatible records. TOML, JSON,
Foundation collections, and owned Objective-C objects do not cross the
per-click FFI boundary.

### 2. Rust rule engine

A macOS-only Rust module owns:

- configuration parsing and validation;
- compilation into ordered immutable rules;
- candidate matching;
- runtime mode, generation, and effective-rule metadata;
- atomic replacement after a successful reload;
- diagnostic event formatting.

All specified fields in one rule are combined with AND. Multiple values inside
one field are combined with OR. Matching does no disk I/O and does not hold a
lock across asynchronous work.

### 3. macOS activation executor

The platform executor receives the selected candidate and pointer coordinate
only for `activate`. It retains the existing public-API behavior:

1. resolve the owning `NSRunningApplication`;
2. require a regular, non-terminated application;
3. capture and PID-validate the Accessibility window at the pointer;
4. activate the application when it is not frontmost;
5. perform `AXRaise` only when the target differs from the focused AX window;
6. release all Core Foundation objects before returning.

It does not guess by title or geometry and does not retry.

### 4. original input path

`src/server/input_service.rs` remains the owner of the original mouse event.
After optional targeting preprocessing, it always proceeds to the existing
`en.mouse_down(MouseButton::Left)` call.

## Per-Click Data Flow

In `passthrough` mode:

1. receive remote left-button-down;
2. skip all targeting preprocessing;
3. deliver the original mouse-down.

In `rules` mode:

1. receive remote left-button-down;
2. read the current immutable configuration generation;
3. collect bounded candidates at the current cursor position;
4. evaluate candidates from top to bottom;
5. on `skip`, continue to the next candidate;
6. on `forward_only`, stop preprocessing;
7. on `activate`, invoke the macOS activation executor and stop traversal;
8. deliver the original mouse-down regardless of preprocessing outcome.

## Configuration File

The user-editable file is:

```text
~/Library/Application Support/RustDesk-Herbin/window-targeting.toml
```

The runtime never rewrites an existing file. Candidate deployment may create a
minimal template only when the file is absent.

```toml
version = 1
mode = "rules"
diagnostics = false

[[rules]]
id = "dock-transient-ui"
priority = 1000
action = "forward_only"
bundle_id_prefixes = ["com.apple.dock"]
layer_min = 1

[[rules]]
id = "custom-passive-overlay"
priority = 900
action = "skip"
bundle_ids = ["com.example.overlay"]
layer_min = 1
covers_display = true
```

### Supported top-level fields

- `version`: required integer; the first format version is `1`.
- `mode`: required; `rules` or `passthrough`.
- `diagnostics`: optional boolean, default `false`.
- `rules`: optional ordered array of user rules.

### Supported rule fields

- `id`: required unique non-empty string.
- `priority`: optional integer, default `0`.
- `action`: required; `skip`, `forward_only`, or `activate`.
- `bundle_ids`: exact bundle identifier values.
- `bundle_id_prefixes`: bundle identifier prefixes.
- `process_names`: exact process names for diagnosis-oriented matching.
- `layers`: exact integer layer values.
- `layer_min` and `layer_max`: inclusive integer bounds.
- `ax_roles`: exact Accessibility role values.
- `ax_subroles`: exact Accessibility subrole values.
- `activation_policies`: `regular`, `accessory`, or `prohibited`.
- `covers_display`: boolean structural condition.

Different matcher fields are AND conditions. Values within an array are OR
conditions. `layers` cannot be combined with `layer_min` or `layer_max`.

Every rule must have at least one matcher. A `skip` rule must include both:

- a stable bundle matcher (`bundle_ids` or `bundle_id_prefixes`); and
- a structural matcher (`layers`, `layer_min`, `layer_max`, or
  `covers_display`).

This prevents an accidental global pass-through rule.

Unknown fields, unsupported versions or values, duplicate IDs, contradictory
layer matchers, empty matchers, and unsafe `skip` rules invalidate the entire
user configuration. Partial loading is not allowed.

## Precedence and Built-In Rules

The first matching rule wins. Evaluation order is:

1. user rules by descending `priority`;
2. user rules with equal priority in file order;
3. ordered built-in rules;
4. conservative hard defaults.

Initial built-in behavior:

1. `AXMenu`, `AXMenuItem`, and `AXPopover` hits use `forward_only`.
2. bundle identifiers beginning with `com.apple.dock` use `forward_only`.
3. the identified Notification Center non-zero-layer, display-covering passive
   overlay uses `skip`.
4. layer-zero windows owned by regular applications use `activate`.
5. every other candidate uses `forward_only`.

User rules precede built-ins and can therefore override a built-in match without
a separate disable list.

If the file is absent, RDH starts in `rules` mode with built-ins only. If an
existing file is invalid at process start, RDH logs the validation failure and
starts with built-ins only. A runtime reload failure never replaces the
currently active generation.

## CLI and IPC

RDH adds a dedicated management command rather than overloading the existing
single-value `--option` interface:

```bash
RustDesk-Herbin --window-targeting status
RustDesk-Herbin --window-targeting validate
RustDesk-Herbin --window-targeting reload
```

`status`
: Query the active user `--server` process through the existing main IPC and
  print mode, generation, effective rule count, source, deterministic effective
  configuration hash, and diagnostics state.

`validate`
: Resolve the active user's configuration path and run the same parser and
  validator without changing runtime state.

`reload`
: Send a dedicated IPC request to the active user `--server` process. Parse,
  validate, and compile into a temporary immutable rule set, then atomically
  replace the current set only after complete success.

The CLI never edits the TOML file. `status` and `reload` fail explicitly when no
unique active user server can be reached. When invoked with administrative
privileges on an installed macOS build, the command follows RustDesk's existing
active-user main-IPC routing rather than addressing root's IPC namespace.

Successful output is machine-readable single-line key/value text:

```text
OK mode=rules rules=6 generation=4 hash=8b03 diagnostics=false
```

Validation or reload failure returns a non-zero exit status and identifies the
rule and field when applicable:

```text
ERROR rule=custom-overlay field=action reason="skip requires bundle and structural matchers"
ACTIVE mode=passthrough generation=3 unchanged=true
```

The IPC request and response are dedicated typed variants rather than magic
option keys.

## Runtime State and Reload Safety

The active state is an immutable compiled rule set behind an atomic shared
reference. A click observes exactly one generation. It cannot observe a mixture
of old and new rules.

Generation starts at `1` for each server process and increments only after a
successful reload. The effective configuration hash is deterministic for the
validated mode and ordered effective rules, so A/B runs can record the exact
behavioral input. A process restart resets the generation but not the hash.

Reload does not restart the server, recreate the input service, alter the
connection list, or pause mouse delivery. Configuration parsing happens before
the atomic swap and outside the click path.

## Failure Handling

- Collector or FFI failure resolves to `forward_only`.
- Missing or unowned candidates resolve to `forward_only`.
- Missing Accessibility permission or AX hit-test failure prevents
  window-level raising but never prevents original mouse delivery.
- A non-regular, unavailable, or terminated selected application is not
  activated; the original mouse event still proceeds.
- `activateWithOptions` or `AXRaise` failure is reported but not retried.
- An unknown non-zero-layer candidate resolves to `forward_only`.
- Only a validated `skip` rule can continue through a candidate.
- Runtime reload failure preserves mode, generation, rules, connection, and
  server PID.
- Repeated identical runtime diagnostic failures are rate-limited.

No fallback may activate a different application from the selected candidate.

## Diagnostics and Privacy

Per-click diagnostics are disabled by default. When enabled, one decision event
contains:

- current mode, generation, and configuration hash;
- candidate bundle identifier, process name, layer, activation policy, and
  non-sensitive structural facts;
- AX role and subrole when available;
- matched rule ID and action;
- whether activation and raising were attempted and their result;
- preprocessing duration.

Window titles and remote peer content are not logged. Repeated errors are
rate-limited. Startup and explicit reload always log a concise configuration
summary even when per-click diagnostics are disabled.

## Performance Contract

- No configuration file reads, parsing, hashing, allocation-heavy
  serialization, retries, sleeps, or background coordination occur per click.
- Diagnostics-off preprocessing on the target Mac must have p95 latency no
  greater than 10 ms under the focused benchmark.
- `passthrough` mode does not call CoreGraphics or Accessibility targeting APIs.

## Testing Strategy

### Pure Rust unit tests

- parse valid `rules` and `passthrough` configurations;
- reject every invalid schema and unsafe `skip` case;
- verify priority, equal-priority file order, built-in order, and first-match
  semantics;
- verify AND across fields and OR within arrays;
- verify conservative defaults;
- verify `passthrough` bypasses collector and executor;
- verify failed reload preserves the active generation and hash;
- verify successful reload increments generation exactly once;
- verify diagnostic redaction and deterministic hashing.

### Recorded candidate fixtures

Use platform-neutral candidate snapshots to test:

- DockHelper layer `101` plus enabled `AXMenuItem` over a layer-zero ChatGPT
  window resolves to `forward_only`;
- Finder non-zero-layer menu/popover over another application resolves to
  `forward_only`;
- a validated Notification Center display-covering overlay resolves to `skip`
  and permits the intended regular application candidate below;
- two layer-zero windows belonging to the same application select the concrete
  clicked window for raising;
- an unknown non-zero-layer owner resolves to `forward_only`;
- a normal cross-application layer-zero candidate resolves to `activate`.

### Source and FFI contract tests

- preserve preprocessing immediately before the original left mouse-down;
- ensure the original mouse-down remains unconditional;
- ensure candidate collection no longer blanket-filters non-zero layers or Dock;
- ensure only `activate` invokes application/window ordering APIs;
- ensure the FFI record layout is bounded and validated;
- reject private API markers.

### CLI and IPC tests

- parse `status`, `validate`, and `reload`;
- route status/reload to the unique active user server;
- validate without mutating runtime state;
- preserve state and return non-zero on invalid reload;
- report new generation/hash after successful reload;
- reject absent or ambiguous server targets explicitly.

### macOS focused benchmark

Benchmark collection, matching, and optional activation with diagnostics off.
Report median, p95, and maximum, and require p95 at or below 10 ms on the target
Mac before deployment.

## Live Remote Acceptance

Keep the official RustDesk rescue channel available during candidate
installation. Record `status` before each A/B pass.

1. In `passthrough`, confirm the upstream focus bug remains reproducible and
   Dock/Finder transient UI still receives its raw click.
2. Reload `rules` without changing the server PID or disconnecting the session.
3. Verify cross-application activation between ChatGPT and X.
4. Verify alternating two windows belonging to ChatGPT.
5. Verify Finder path-bar **Copy as Pathname** completes without activating the
   lower window.
6. Verify Finder preview/display-options popovers remain interactive.
7. Use a disposable test application to verify Dock menu **Quit** executes and
   does not activate the lower window first.
8. Reproduce the Notification Center overlay and verify clicks still target the
   intended regular application.
9. Submit an invalid reload and verify the previous mode, generation, hash, PID,
   connection, and behavior remain unchanged.
10. Verify local mouse behavior is unchanged.
11. Disable diagnostics and complete the focused latency benchmark.

## Rollout and Rollback

The candidate is built through the existing macOS CI workflow. Verify commit,
architecture, bundle identity, ad-hoc signing expectations, LaunchAgent
persistence, and memory watchdog boundaries before installation.

Retain the currently installed RDH artifact and official RustDesk rescue channel
until live acceptance passes.

Behavioral rollback does not require a restart:

1. set `mode = "passthrough"`;
2. run `validate`;
3. run `reload`;
4. confirm `status` reports the new mode and generation.

If the candidate itself affects service health, IPC, input delivery, or startup,
use the established bounded application/LaunchAgent rollback while connected
through the official rescue channel.

## Rejected Alternatives

### Bundle/layer allow and deny lists only

They cannot express the difference between passive overlays and interactive
transient UI owned by the same system process.

### File watching

Editors commonly save through temporary files, partial writes, and renames.
Watching requires debounce and introduces silent mid-session behavior changes.
Explicit validation and reload provide a deliberate, auditable A/B boundary.

### Scriptable rules

Lua or JavaScript would add runtime, security, latency, and failure surfaces to
the mouse hot path without a current requirement.

### Automatic official-application fallback

Closing RDH, launching official RustDesk, reconciling two services, and handing
off an active connection is materially broader and less reliable than the
required deterministic `passthrough` baseline.
