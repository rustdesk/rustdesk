# Adaptive iOS remote-session layouts

## Requirements and references

Remote sessions should use the complete iPhone Duo window in outer portrait, outer landscape, unfolded portrait and unfolded landscape. A horizontal half-fold should reserve the upper area for the remote image and place every session control and input surface below the division. Fit must preserve the remote aspect ratio; its letterboxing is separate from an app compatibility window or unused safe-area bands.

Ordinary layouts retain the existing toolbar placement, user scaling/panning and keyboard offset restoration. Opening a half-fold chat window should leave the lower controls visible until the user requests text input. Done dismisses input and preserves the draft; remote Enter remains a separate action.

- [Duo discussion](https://github.com/rustdesk/rustdesk/discussions/16684)
- [Apple full-window guidance](https://developer.apple.com/iphone-duo/prepare/)
- [UIHingeInteraction](https://developer.apple.com/documentation/uikit/uihingeinteraction)
- [Flutter scene migration](https://docs.flutter.dev/release/breaking-changes/uiscenedelegate)

## Design choices

Use the official Flutter scene/implicit-engine entry point and current-view native geometry. Keep the existing RemotePage route, session, input protocol, text controllers and controls. iOS uses a separate layout entry point so the established non-iOS body remains intact.

The render surface extends to the window edges. Foreground controls apply their own safe-area handling; the physical outer camera remains an occlusion. A division's interaction margins are not a permanently undisplayable band. Read the division frame/margins and keyboard intersection rather than assuming a fixed hinge or keyboard size.

Rejected alternatives:

- An input accessory above the system keyboard occupies the upper area and conflicts with the display/input split.
- Default stretching, cropping or changing the remote host's physical resolution changes visible content rather than fixing the local window.
- A new fixed keyboard scale floor or additional scale/offset caches duplicates existing canvas state and changes ordinary input behavior.

## Architecture and regression surface

**Native lifecycle.** AppDelegate registers plugins through the implicit-engine callback; RustDeskSceneDelegate attaches a transparent, non-interactive geometry observer to the active root view. One event channel reports view bounds, hinge status, active division frames/margins and the keyboard intersection in view coordinates. New hinge/reserved-region APIs are guarded by iOS 27.1 availability. Flutter deep-link routing is disabled so the existing uni_links path continues handling RustDesk links. Status-bar appearance is controlled by the view controller.

**Layout.** RemotePage enters half-fold mode only for a partially open, horizontal, active division matching the current window dimensions. This rejects closed-device divisions and stale geometry during rotation. The upper image reaches the division's core boundary; controls start at its lower interaction boundary. If the keyboard overlaps the upper boundary, the image uses the keyboard's actual top. A persistent bounded Overlay and its own Scaffold keep session controls, menus and chat in the lower area without remounting their owners or applying the keyboard inset twice. Authentication uses the global overlay because it can precede the lower layout.

**Canvas and input.** WindowMetricsWatcher reports the real body viewport to CanvasModel. Registered iOS viewports use the same zero-padding origin for drawing and pointer conversion; pages without a registered viewport retain their original padding. Lower-pane controls do not subtract space from the upper canvas. Android action-overlay initialization uses the same ready/first-image condition as the original mobile layout, with the overlay attached to the session's bounded control area.

Half-fold virtual mouse controls use a 250-point container with matching inherited dimensions. This accommodates the wheel and joystick; left/right buttons use deterministic positions without reading or saving ordinary fullscreen position preferences. Lower-control blocked areas are ignored by upper-canvas checks; active drag/scroll blocking remains in effect. Touch-mode mouse movement uses relative deltas rather than treating a lower-pane global position as an upper-canvas pointer position.

**Keyboard and user position.** Window/DPR changes refit the image. Ordinary keyboard resizes update viewport dimensions without refitting, retaining the existing scale/offset save, cursor avoidance and restoration. Keyboard detection reads the FlutterView's raw inset because Scaffold consumes the inherited MediaQuery inset. Preserve isMobileCanvasChanged for registered iOS viewports during keyboard input so a later toolbar toggle does not discard the restored user position. Half-fold keyboard changes may refit the upper image. Session clear resets the viewport, division flag and iOS adjustment flag.

**Chat.** The existing chat-window entry point suppresses automatic keyboard requests whenever controls are outside the canvas. This covers the toolbar, floating chat icon and incoming-message paths. Ordinary chat keeps its multiline configuration. Half-fold Done only dismisses the keyboard; sending remains explicit.

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

## Delivery and validation contract

Implementation and review are scoped to this feature's final diff. Runtime changes require the complete simulator matrix: five Duo postures by twelve distinct remote ratios, followed by iPhone 18 Pro portrait/landscape by the same twelve ratios. Ratios are 16:9, 16:10, 3:2, 4:3, 5:4, 43:18, 32:9, 9:16, 9:20, 20:9, 3:4 and 1:1; identical ratios at different pixel counts are not separate cases.

Each case uses a real RustDesk peer and checks the remote framebuffer/decoded size, complete fitted image, five delivered pointer events, keyboard opening/dismissal and canvas restoration. Ordinary cases preserve scale; half-fold cases also check the complete upper image with the keyboard visible. Supplementary regressions cover user adjustment followed by keyboard round-trip and toolbar toggling, session reconnect, and half-fold chat entry paths. Keep bulk screenshots, logs, build identities, automation and successful-run references outside version control.

Simulator verification is the current acceptance boundary. Physical Duo/15 Pro testing, floating/split/hardware keyboards, two-finger gestures, reverse landscape, iPad multitasking, other pages' status bars, remote Chinese delivery and signed release/CI builds are separate checks. A portrait desktop framebuffer verifies a phone aspect ratio rather than a mobile remote OS. The simulator's environment-only native-library compatibility and iOS 15 deployment override do not change the committed minimum of iOS 13 or establish older-device support. No release schedule or physical-device sign-off is assigned here.

Known limits:

- The docked system keyboard can cover the entire half-fold lower area, hiding the chat editor and Send button. The OS Password editor has the same visibility limitation; fixing its menu dismissal preserves the session but does not make that editor visible behind the keyboard. Done dismisses the keyboard, and chat retains its draft. Keeping the upper area display-only, retaining the native IME and showing the editor while typing are not jointly achievable in that geometry: an input accessory also occupies the upper area, and iPhone ignores custom input-assistant items. A chat-only compact editor above the keyboard requires an explicit exception to the display-only requirement; this interaction decision remains open.
- After switching from half-fold to the closed outer display, the ordinary floating chat icon can be outside the visible window. Its legacy widget reads implicit inner-view dimensions; the half-fold offset fix deliberately leaves the ordinary configuration intact. Toolbar chat remains accessible. Returning to half-fold restores the icon without losing the draft. A general floating-widget geometry replacement is outside this change.
- Half-fold size changes recreate only the floating icon, resetting its dragged position. Its lower drag boundary can overlap the toolbar, as in the existing fullscreen behavior.
- The incoming-message chat path is source-reviewed but not runtime-accepted: the dedicated Linux peer's connection-manager process exited before registering a connection. Toolbar and floating-icon entry, native Chinese composition and three Done cycles were exercised separately; those checks do not establish incoming-message delivery.
