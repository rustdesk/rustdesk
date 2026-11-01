# iOS full-window remote-session layouts

## Scope and references

Stage 1 uses the complete iOS remote-session body on ordinary iPhones and on the Duo outer and unfolded displays, in portrait and landscape. Fit preserves the remote aspect ratio; intentional image letterboxing is separate from an app compatibility window or unused safe-area bands. The existing toolbar, floating action button, auxiliary keys, chat, mouse controls and popup menus retain their ordinary placement and behavior.

Native half-fold partitioning and controls that move between panes are deferred to Stage 2. Stage 1 has no hinge observer, native geometry/keyboard event channel, automatic half-fold keyboard opening, bounded control Overlay or separate iOS RemotePage implementation. The complete pre-split implementation remains recorded at `de2e67c8499a579f87f2f528c929c0c19da8ea2c`; extracting this branch does not discard that work.

- [Duo discussion](https://github.com/rustdesk/rustdesk/discussions/16684)
- [Apple full-window guidance](https://developer.apple.com/iphone-duo/prepare/)
- [Flutter scene migration](https://docs.flutter.dev/release/breaking-changes/uiscenedelegate)

## Design and regression surface

**Lifecycle and build.** iOS uses Flutter 3.44.8 and Xcode/SDK 27.1. AppDelegate registers plugins through the implicit-engine callback, and the existing SceneDelegate subclasses FlutterSceneDelegate without custom scene forwarding or geometry hooks. The inherited single scene manifest remains in place. Flutter deep-link routing is disabled so the existing uni_links path owns RustDesk links. The iOS 15 deployment target is inherited from upstream master ef827fd, rather than a new support-policy decision in this feature.

The iOS build and CI job select the matching toolchain and existing Flutter 3.44 bridge artifact. The source-patch script supports BSD sed on macOS while retaining GNU sed behavior for Windows arm64. Local builds save and restore the exact starting contents of common.dart, pubspec.yaml, tracked dependency locks and Runner.xcscheme on success and failure, including pre-existing dirty edits. Other platform SDK versions and package flows are unchanged. No remote protocol, generated bridge interface or audio behavior changes.

**Remote page and overlays.** Keep the original RemotePage build, Scaffold, toolbar/FAB callbacks, text controllers and body Overlay. Before connection, emptyOverlay retains its original BlockableOverlay key; after connection, its owner retains the original global-overlay fallback. Authentication, chat, password editing, virtual mouse controls and popup menus use their existing paths.

Only the iOS body inside the existing Overlay entry replaces the whole-body SafeArea and OrientationBuilder timer with WindowMetricsWatcher. The canvas reaches the body edges; QualityMonitor and auxiliary keys apply their own safe insets. Auxiliary-key background color reaches the window edge while its content stays outside the camera inset. Other platforms keep the original SafeArea/OrientationBuilder branch. The existing keyboard SystemChrome behavior remains unchanged.

**Session links.** Existing uni_links routing still owns connection URLs. On iOS, reopening the peer already displayed by a ready session reuses that page instead of creating a second page that resets the shared FFI during the first page's build. The guard reads existing session and dialog state only. It does not apply new URL options to an already-ready session; home, closed, connecting, other-peer and other connection-type routing retains the existing behavior.

**Canvas and pointer origin.** WindowMetricsWatcher reports the real body viewport to CanvasModel. Registered iOS viewports use zero canvas padding for drawing and pointer conversion. Pages without a registered viewport retain their original padding and size computation. Controls continue using the original canvas adjustment and blocked-rectangle rules. Clearing a session resets its registered viewport and iOS user-adjustment flag.

**Keyboard and user position.** Window/DPR changes refit the image. Ordinary keyboard resizes update the viewport without refitting, retaining the existing scale/offset save, cursor avoidance and restoration. Keyboard detection reads the FlutterView raw inset because Scaffold consumes the inherited MediaQuery inset. Preserve isMobileCanvasChanged for registered iOS viewports so a toolbar toggle after a keyboard round trip does not discard the restored user position.

A newly mounted body can temporarily have zero height while keyboard insets change. Ignore empty viewports and retain the existing pending refit until the first usable body size; fitting a zero-height viewport would select the 1.0 fallback and ordinary keyboard suppression could preserve that wrong scale. This generic viewport guard is retained from the pre-split half-fold exit fix without carrying the half-fold lifecycle or UI.

Rejected alternatives remain stretching/cropping the remote framebuffer, changing the remote host resolution to hide a local layout problem, additional keyboard scale/offset caches, and replacing the existing overlay/control architecture.

## Validation evidence and extraction boundary

The completed pre-split evidence is preserved by the source history and the validation record at `de2e67c8499a579f87f2f528c929c0c19da8ea2c`:

| Source | Completed scope | Limit when applying it to Stage 1 |
| --- | --- | --- |
| Duo and iPhone 18 Pro simulators | The complete five-posture Duo matrix and iPhone 18 Pro portrait/landscape matrix used twelve distinct remote aspect ratios, with real-peer image, pointer and keyboard checks. | These runs used the pre-split iOS page. They remain completed evidence for that implementation, rather than acceptance of the extracted original-overlay path. |
| Physical iPhone 13 Pro | Eight checks covered remote proportions, pointer delivery, keyboard/toolbar behavior, auxiliary keys, virtual mouse, chat and password cancellation; a later 4f0262e smoke test was also completed. | Device checks remain tied to their original binaries. |
| User's physical Duo | The user confirmed the principal physical experience and reported a half-fold-to-unfold sizing defect. Its corrected development-signed IPA was delivered. | The correction still awaited the user's physical retest in the pre-split record; Stage 1 does not ship native half-fold partitioning. |
| iOS builds | The refreshed upstream iOS 15 configuration produced an unsigned archive without a deployment-target override. Development signing was verified separately. | Neither an unsigned archive nor development signing establishes App Store/release distribution. |

Stage 1 changes the page composition relative to the pre-split validation. Its extracted revision therefore requires its own full-window checks on Duo outer/unfolded portrait and landscape, and iPhone 18 Pro portrait and landscape, across the same twelve ratios: 16:9, 16:10, 3:2, 4:3, 5:4, 43:18, 32:9, 9:16, 9:20, 20:9, 3:4 and 1:1. Relevant regressions include manual scale/pan followed by keyboard and toolbar round trips, reconnect, pre-connect authentication, chat/password overlays and cold/warm RustDesk URLs. Stage 2 owns the half-fold partition cases. Bulk screenshots, logs and build identities stay outside version control.

The extraction's focused source tests and new runtime/build results must be recorded with their own revision and scope; the historical matrix is not reset to missing or presented as newly rerun. Runtime on older supported iOS versions, physical mouse accuracy, floating/split/hardware keyboards, two-finger gestures, reverse landscape, iPad multitasking and release distribution remain separate acceptance boundaries. A portrait desktop framebuffer verifies a phone aspect ratio rather than a mobile remote OS.

## Deferred half-fold work

Stage 2 restores native division/keyboard geometry, upper-image rendering, and the existing controls that move below the division or above native input. It carries the half-fold trackpad, distinct keyboard toggle, tagged menu dismissal and feature-local mouse/chat placement. Higher keyboards and resized control areas need their own focused checks. The accepted chat-icon position reset, manually dragged legacy windows and incoming-message validation limit remain scoped to that later work.

Chinese Pinyin remains a separate input limitation: the same nihao/Return sequence reproduced in half-fold and ordinary layouts and in a historical pre-migration binary control. Its exact historical source revision was unavailable, and it did not establish Unicode delivery or editor line breaks. Preserve the existing shared input path in this layout change.
