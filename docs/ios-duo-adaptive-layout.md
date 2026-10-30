# Adaptive iOS remote-session layouts

## Requirements and references

Remote sessions should use the complete iPhone Duo window in outer portrait, outer landscape, unfolded portrait and unfolded landscape. A horizontal half-fold keeps native text input below the division. The upper area holds the remote image with auxiliary keys at the top and an explicitly expandable session toolbar at the bottom; pinned auxiliary keys or selected touch modifiers remain visible when the toolbar is collapsed. Fit must preserve the remote aspect ratio; its letterboxing is separate from an app compatibility window or unused safe-area bands.

Ordinary layouts retain the existing toolbar placement, user scaling/panning and keyboard offset restoration. Entering a ready half-fold remote session requests the native keyboard once. Done dismisses input; a dedicated lower keyboard entry restores it. The upper toolbar keyboard icon toggles auxiliary keys only, so opening native input and showing auxiliary keys have distinct controls. Chat Done preserves its draft; remote Enter remains a separate action.

- [Duo discussion](https://github.com/rustdesk/rustdesk/discussions/16684)
- [Apple full-window guidance](https://developer.apple.com/iphone-duo/prepare/)
- [UIHingeInteraction](https://developer.apple.com/documentation/uikit/uihingeinteraction)
- [Flutter scene migration](https://docs.flutter.dev/release/breaking-changes/uiscenedelegate)

## Design choices

Use the official Flutter scene/implicit-engine entry point and current-view native geometry. Keep the existing RemotePage route, session, input protocol, text controllers and controls. iOS uses a separate layout entry point so the established non-iOS body remains intact.

The render surface extends to the window edges. Foreground controls apply their own safe-area handling; the physical outer camera remains an occlusion. A division's interaction margins are not a permanently undisplayable band. Read the division frame/margins and keyboard intersection rather than assuming a fixed hinge or keyboard size.

Rejected alternatives:

- An input accessory stays directly above the system keyboard and does not reduce its reserved frame; use independent top-aligned auxiliary keys instead.
- Default stretching, cropping or changing the remote host's physical resolution changes visible content rather than fixing the local window.
- A new fixed keyboard scale floor or additional scale/offset caches duplicates existing canvas state and changes ordinary input behavior.

## Architecture and regression surface

**Native lifecycle.** AppDelegate registers plugins through the implicit-engine callback; RustDeskSceneDelegate attaches a transparent, non-interactive geometry observer to the active root view. One event channel reports view bounds, hinge status, active division frames/margins and the keyboard intersection in view coordinates. New hinge/reserved-region APIs are guarded by iOS 27.1 availability. Flutter deep-link routing is disabled so the existing uni_links path continues handling RustDesk links. Status-bar appearance is controlled by the view controller.

**Layout.** RemotePage enters half-fold mode only for a partially open, horizontal, active division matching the current window dimensions. This rejects closed-device divisions and stale geometry during rotation. The upper image reaches the division's core boundary; controls start at its lower interaction boundary. If the keyboard overlaps the upper boundary, the image uses the keyboard's actual top. A persistent bounded Overlay and its own Scaffold keep session controls, menus and chat in the lower area without remounting their owners or applying the keyboard inset twice. Authentication uses the global overlay because it can precede the lower layout.

**Canvas and input.** WindowMetricsWatcher reports the real body viewport to CanvasModel. Registered iOS viewports use the same zero-padding origin for drawing and pointer conversion; pages without a registered viewport retain their original padding. Lower-pane controls do not subtract space from the upper canvas. Android action-overlay initialization uses the same ready/first-image condition as the original mobile layout, with the overlay attached to the session's bounded control area.

Half-fold virtual mouse controls use a 250-point container with matching inherited dimensions. This accommodates the wheel and joystick; left/right buttons use deterministic positions without reading or saving ordinary fullscreen position preferences. Lower-control blocked areas are ignored by upper-canvas checks; active drag/scroll blocking remains in effect. Touch-mode mouse movement uses relative deltas rather than treating a lower-pane global position as an upper-canvas pointer position.

**Keyboard and user position.** Window/DPR changes refit the image. Ordinary keyboard resizes update viewport dimensions without refitting, retaining the existing scale/offset save, cursor avoidance and restoration. Keyboard detection reads the FlutterView's raw inset because Scaffold consumes the inherited MediaQuery inset. Preserve isMobileCanvasChanged for registered iOS viewports during keyboard input so a later toolbar toggle does not discard the restored user position. Half-fold keyboard changes may refit the upper image. Session clear resets the viewport, division flag and iOS adjustment flag.

**Chat.** The existing chat-window entry point suppresses automatic keyboard requests whenever controls are outside the canvas. This covers the toolbar, floating chat icon and incoming-message paths. Ordinary chat keeps its multiline configuration. Half-fold Done only dismisses the keyboard; sending remains explicit.

**Half-fold keyboard controls.** Keep one bounded control widget mounted inside the upper image Stack and its existing Scaffold. Its floating button expands the original blue toolbar; the toolbar keyboard icon requests the existing KeyHelpTools without changing native keyboard visibility. Collapsing the toolbar hides unpinned auxiliary keys, while KeyHelpTools retains its own pin, compact/Fn selection and visible touch-modifier state. The same instance survives Done and keyboard reopening. The auxiliary area follows the original top placement, applies its own safe area and can scroll within the upper bounds; its overlay does not resize or refit the image. Remove safe-area padding only in this upper control group; ordinary screen-bottom toolbars retain it.

Native input opens on first image or a transition into half-fold only when keyboard input is already permitted. The lower manual entry retains the original permission gate; automatic entry does not reopen a warning the user declined. Keyboard-frame updates and Done do not reopen it automatically. After Done, the lower area shows the existing mouse controls and a dedicated Keyboard button. This entry opens native input; it never toggles auxiliary keys. Ordinary toolbar keyboard/FAB behavior remains unchanged.

The floating chat widget reads full-view dimensions internally. In half-fold mode, configure its existing non-fullscreen offset with the lower overlay's origin, and recreate only that icon when its bounded size changes. This keeps its initial position and global drag conversion inside the lower controls without changing ordinary chat or its text controller.

**Builds.** iOS uses Flutter 3.44.8, an SDK providing iOS 27.1 APIs, and the existing Flutter 3.44 bridge-artifact path. The shared source-patch script supports BSD sed on macOS while retaining GNU sed behavior for Windows arm64. Other platform Flutter versions remain unchanged. No remote protocol, generated bridge interface, audio or file-transfer behavior is changed.

Confirmed regressions and fix rationale:

- Whole-page SafeArea plus canvas padding left unused edge bands; using a real body viewport also prevents counting the bottom toolbar as displayable image space.
- Treating every body resize as Fit discarded normal keyboard/user positioning. Distinguish window changes, keyboard changes and ordinary viewport changes; preserve the existing adjustment flag through keyboard restoration.
- The separate iOS entry point omitted Android action-overlay initialization. Restore the original ready/first-image condition rather than adding a second overlay lifecycle.
- Replacing a keyed Overlay lost entries after authentication; retaining one entry and rebuilding its child preserves session controls. An added opaque middle layer swallowed first clicks and drags, so it is removed.
- Restricting chat keyboard suppression to the toolbar left floating-icon and incoming-message paths inconsistent. Apply the policy once at the shared chat-window entry point.
- The floating chat widget's full-view origin placed its icon outside the lower overlay. Use its existing non-fullscreen offset for half-fold coordinates rather than adding position state.
- The OS Password edit icon assumed its menu was a Navigator route and popped the remote page from the half-fold overlay. Give only that menu an explicit tagged dismissal callback; ordinary popup menus retain their existing dismissal path.
- Virtual mouse widgets inherited lower-pane dimensions inside a shorter fixed box, clipping the wheel/buttons and confusing lower-local blocking rectangles with upper-canvas coordinates. Match the control box dimensions and isolate half-fold positions from ordinary saved positions.
- Hiding half-fold controls and auxiliary keys behind the native keyboard made combination-key input inaccessible. Keep the auxiliary instance above the division, expose the toolbar on request and give native input a distinct lower entry. Reuse the original modifier and pin behavior rather than duplicating key state. The native keyboard owns its size; a custom inputView would replace that keyboard.

## Review disposition and proposed follow-up

Design review against runtime `7bd877f` / PR head `49abf5b`, checked on 2026-10-30. The [maintainer's seven-item review](https://github.com/rustdesk/rustdesk/pull/16759#issuecomment-6515116614) is reconciled below. Resolved bot threads and an incremental review without new findings do not establish that every merge requirement is met.

| Review item | Current disposition | Next action |
| --- | --- | --- |
| Chat input and Send hidden by the docked keyboard | Still an open defect. Top auxiliary keys do not expose the lower chat editor. | Prototype keyboard avoidance for the existing chat window; keep its controller, draft and Done behavior. |
| OS Password editing pops the session route | Fixed by tagged half-fold menu dismissal; ordinary menus retain their existing path. | Retain the fix and verify edit/cancel with a live session. The editor's keyboard visibility is a separate remaining limitation. |
| Virtual mouse controls clipped by their container | Source fix uses matching 250-point control/MediaQuery bounds. | Replay wheel, left/right buttons and joystick on the final binary. |
| Lower control rectangles block upper-canvas taps | Source fix separates lower rectangles from upper hit testing while retaining active drag/scroll blocking. | Replay upper clicks with lower controls enabled on the final binary. |
| Virtual mouse maps lower global positions into the upper canvas | Source fix uses relative deltas in half-fold. | Compare final-binary cursor motion and clicks with the real peer. The default-controls-off ratio matrix does not cover this path. |
| CI SDK and minimum iOS support | Runner/SDK mismatch remains; minimum support is a separate compatibility decision. | Select a compatible iOS runner/toolchain, execute an iOS build, and obtain the maintainer's supported-version policy before changing deployment targets. |
| Local builds leave Flutter 3.44 edits in a reused checkout | Still valid. The script modifies source/dependencies without restoring their original contents. | Restore the files changed by the build on success/failure while preserving the user's pre-existing modifications. |

**Candidate chat fix, not implemented.** Prefer retaining the same bounded Overlay and chat entry. While half-fold chat explicitly requests native typing, has input focus and intersects the keyboard, temporarily give that Overlay the full-window region and actual keyboard inset. Keep the lower control subtree wrapped in its original lower-size MediaQuery and a stable padding wrapper so its background and mouse box remain below the division. The existing IOSDraggable can then place its existing chat window above the keyboard. Listen to the existing FocusNode as well as geometry; do not add a second keyboard owner, saved-position cache, timer or input state machine. Top auxiliary controls remain in their current position. Merely restoring viewInsets inside an Overlay still clipped to the lower pane cannot solve the defect.

This candidate temporarily overlays the upper image with the existing, explicitly opened chat window; it does not add a default chat composer. Confirm this interaction before implementation. Its first-frame avoidance, dragged positions, focus transfer from the remote keyboard, Done restoration and draft preservation require a local prototype. Keep ordinary chat unchanged. The OS Password editor's analogous visibility problem must remain explicit rather than being described as fixed by the menu callback.

**Build follow-up.** Limit runner selection to the iOS job. The [official runner mapping](https://github.com/actions/runner-images/blob/main/README.md) maps macos-latest to macOS 26, whose [installed SDK list](https://github.com/actions/runner-images/blob/main/images/macos/macos-26-arm64-Readme.md) stops at iOS 26.5. The [Xcode 27 image](https://github.com/actions/runner-images/blob/main/images/macos/xcode-27-arm64-Readme.md) provides Xcode 27.1. Select that toolchain before Rust, vcpkg and Flutter so their SDK choice agrees. Ordinary PR CI passes upload-artifact=false and skips the iOS job; obtain a build-only iOS result without running nightly/tag publication flows just to validate this PR.

The iOS 13 Runner/Podfile and forced iOS 11 pod settings predate this PR. [Apple's supported deployment table](https://developer.apple.com/xcode/system-requirements) lists iOS 15 and later for both Xcode 26 and 27. The [master baseline iOS job](https://github.com/rustdesk/rustdesk/actions/runs/40978778897/job/123816093932), at this PR's base, nevertheless completed successfully. Neither the old settings nor that successful build prove iOS 13 device support or that the new toolchain must fail. Do not automatically drop iOS 13/14, claim their support, or introduce dual release builds; the maintainer must settle the intended support range, then align and validate the relevant targets.

For local cleanup, save the exact starting contents of common.dart, pubspec.yaml and tracked pubspec.lock / ios/Podfile.lock before patching. Restore affected files through exit cleanup on successful and failed builds; preserve pre-existing dirty edits rather than checking files out from HEAD. Ignored generated caches can follow Flutter's normal regeneration path. Validate a reused checkout with pre-existing edits, not just a clean CI checkout.

The agreed implementation boundary is correctness plus small, feature-local changes. Already fixed items need evidence rather than another rewrite. Generic docstring-coverage suggestions do not override the repository's rule against unnecessary comments. Explicitly accepted legacy floating-chat limitations do not require a general overlay replacement. Runtime edits retain the user's complete simulator-matrix requirement in addition to targeted input regressions; the maintainer's suggestion of lighter verification does not override that user constraint.

## Delivery and validation contract

Implementation and review are scoped to this feature's final diff. Runtime changes require the complete simulator matrix: five Duo postures by twelve distinct remote ratios, followed by iPhone 18 Pro portrait/landscape by the same twelve ratios. Ratios are 16:9, 16:10, 3:2, 4:3, 5:4, 43:18, 32:9, 9:16, 9:20, 20:9, 3:4 and 1:1; identical ratios at different pixel counts are not separate cases.

Each case uses a real RustDesk peer and checks the remote framebuffer/decoded size, complete fitted image, five delivered pointer events, keyboard opening/dismissal and canvas restoration. Ordinary cases preserve scale; half-fold cases also check upper-image geometry with the keyboard visible, recording intentional floating-button coverage separately from image clipping. Explicitly expanded toolbars and auxiliary keys intentionally cover part of the image. Supplementary regressions cover user adjustment followed by keyboard round-trip and toolbar toggling, session reconnect, and half-fold chat entry paths. Keep bulk screenshots, logs, build identities, automation and successful-run references outside version control.

Simulator verification is the current acceptance boundary. Physical Duo/15 Pro testing, floating/split/hardware keyboards, two-finger gestures, reverse landscape, iPad multitasking, other pages' status bars, remote Chinese delivery and signed release/CI builds are separate checks. A portrait desktop framebuffer verifies a phone aspect ratio rather than a mobile remote OS. The simulator's environment-only native-library compatibility and iOS 15 deployment override do not change the committed minimum of iOS 13 or establish older-device support. No release schedule or physical-device sign-off is assigned here.

Known limits:

- Open maintainer requirement: the docked system keyboard can cover the entire half-fold lower area, hiding the chat editor and Send button. The OS Password editor has the same visibility limitation; fixing its menu dismissal preserves the session but does not make that editor visible behind the keyboard. Done dismisses the keyboard, and chat retains its draft. The upper floating-button/toolbar and auxiliary keys restore remote controls; they do not relocate chat or password editors. A default chat composer in the upper area is not part of the selected interaction.
- After switching from half-fold to the closed outer display, the ordinary floating chat icon can be outside the visible window. Its legacy widget reads implicit inner-view dimensions; the half-fold offset fix deliberately leaves the ordinary configuration intact. Toolbar chat remains accessible. Returning to half-fold restores the icon without losing the draft. A general floating-widget geometry replacement is outside this change.
- Half-fold size changes recreate only the floating icon, resetting its dragged position. Its lower drag boundary can overlap the toolbar, as in the existing fullscreen behavior.
- The incoming-message chat path is source-reviewed but not runtime-accepted: the dedicated Linux peer's connection-manager process exited before registering a connection. Toolbar and floating-icon entry, native Chinese composition and three Done cycles were exercised separately; those checks do not establish incoming-message delivery.
