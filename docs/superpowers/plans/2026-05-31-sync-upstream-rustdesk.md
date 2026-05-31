# Sync Upstream rustdesk/master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge 41 upstream `rustdesk/master` commits (`5439ec38b..fa369365a`) into Tabby's `main` via one merge commit on `chore/sync-upstream-2026-05-31`, validate the build, and open a PR for user review.

**Architecture:** Branch-isolated merge. Create `chore/sync-upstream-2026-05-31` from current `main`, run `git merge upstream/master --no-ff`, resolve conflicts in one pass favouring Tabby behaviour where intentional, update the `libs/hbb_common` submodule, validate via `flutter analyze` + `flutter build ios --no-codesign` + `cargo check` + manual iOS-simulator smoke test, push, open PR. `main` is never modified by Claude; the user merges the PR.

**Tech Stack:** git (with submodule), Flutter 3.41.8 (system, not fvm), Rust + cargo, Xcode/iOS simulator, GitHub CLI (`gh`).

**Note on TDD:** This is an integration/infrastructure task. There is no new product code to test-drive. Each task's "verification" gate plays the role of the test: a concrete command with an expected pass criterion. Treat a failed gate the same as a failing test — stop and diagnose before proceeding.

---

## Task 1: Pre-flight — confirm clean state and record baseline

**Files:**
- Read-only: `git status`, `git rev-parse main`

- [ ] **Step 1: Verify working tree is clean**

Run from repo root:

```sh
git status
```

Expected: `On branch main`, `Your branch is up to date with 'origin/main'`, and the only untracked entry is `.claude/plans/`. **No staged or unstaged changes.**

If anything else is staged or modified, STOP. Either commit, stash, or discard before continuing — do not proceed with a dirty tree.

- [ ] **Step 2: Confirm we are on `main`**

```sh
git rev-parse --abbrev-ref HEAD
```

Expected output: `main`

If not on `main`: `git checkout main` first.

- [ ] **Step 3: Record baseline SHA for rollback**

```sh
git rev-parse main
```

Copy the output (a 40-char SHA) into a scratch note. This is the rollback target if anything goes wrong. Expected value at plan-writing time: `79aaca59d…` (the spec commit), but read whatever is current.

- [ ] **Step 4: Confirm `upstream` remote exists and is fetched**

```sh
git remote -v | grep upstream
git rev-parse upstream/master
```

Expected first command: shows `upstream  ssh://git@github.com/rustdesk/rustdesk.git (fetch)` and `(push)`.
Expected second command: `fa369365a…` (the upstream tip at plan-writing time; may be newer if upstream has moved — note the new SHA and use it as the target).

If `upstream/master` is stale, run `git fetch upstream` and re-check. **Do not** re-run if it errors on submodule fetch — that error is expected and is handled by Task 4.

---

## Task 2: Pre-flight — verify pre-merge build is green

**Files:**
- Read-only build of the Flutter iOS app and a Rust syntax check.

This task exists so that any post-merge failure can be attributed to the merge, not a pre-existing break.

- [ ] **Step 1: Flutter analyze on current `main`**

```sh
cd flutter && flutter pub get && flutter analyze
```

Expected: command completes with exit code 0. Note any pre-existing `info` or `warning` messages — those are the baseline; the post-merge analyze must not exceed them.

- [ ] **Step 2: Flutter iOS build on current `main`**

```sh
flutter build ios --no-codesign
```

Expected: `Built /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter/build/ios/iphoneos/Runner.app` (or similar), exit code 0.

If this fails, STOP. Fix the pre-merge break before attempting the merge.

- [ ] **Step 3: Cargo check on current `main`**

```sh
cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby && cargo check
```

Expected: `Finished \`dev\` profile` (or similar), exit code 0. Warnings are acceptable; errors are not.

If `cargo check` errors with missing C libraries (vcpkg, sciter), that may be a known environment gap. Note it as the baseline and continue — we will compare apples-to-apples against the post-merge `cargo check`.

---

## Task 3: Create the sync branch

**Files:**
- Modify: local git refs only.

- [ ] **Step 1: Create and switch to the sync branch**

```sh
git checkout -b chore/sync-upstream-2026-05-31
```

Expected: `Switched to a new branch 'chore/sync-upstream-2026-05-31'`

- [ ] **Step 2: Confirm branch identity**

```sh
git rev-parse --abbrev-ref HEAD
git log --oneline -1
```

Expected first command: `chore/sync-upstream-2026-05-31`
Expected second command: same SHA and message as `main`'s tip (we haven't merged yet).

---

## Task 4: Run the merge

**Files:**
- Modify: working tree (potentially many files), index, branch ref.

- [ ] **Step 1: Run the merge with explicit message and `--no-ff`**

```sh
git merge upstream/master --no-ff \
  -m "chore: merge upstream rustdesk/master into Tabby (sync 2026-05-31)"
```

Three possible outcomes:

**A. Clean auto-merge** — output ends with `Merge made by the 'ort' strategy.` followed by a diffstat. **Proceed to Task 5.**

**B. Conflicts reported** — output contains lines like `CONFLICT (content): Merge conflict in <path>`. Git pauses with the conflicts staged for resolution. **Proceed to Step 2 of this task.**

**C. Submodule fetch error** — output may complain about `libs/hbb_common` not being able to fetch a ref. This is non-fatal for the merge itself — the merge will still produce a tree with the upstream submodule pointer. **Note the warning and continue to Step 2.** Submodule resolution happens in Task 5.

- [ ] **Step 2: If conflicts, list them**

```sh
git status
```

Look for the `Unmerged paths:` section. Expected conflicting files (per spec prediction):

- `flutter/lib/mobile/widgets/floating_mouse.dart` (likely)
- Possibly one or more of: `flutter/lib/common.dart`, `flutter/lib/common/widgets/remote_input.dart`, `flutter/lib/common/widgets/toolbar.dart`, `flutter/lib/common/widgets/dialog.dart`, `flutter/lib/consts.dart`

Total expected: under 10 files. If the count is much higher (>20), STOP and reassess — something unexpected is happening. Run `git merge --abort` and re-examine before retrying.

- [ ] **Step 3: Resolve each conflict**

For each conflicting file:

1. Open the file. Find `<<<<<<< HEAD` / `=======` / `>>>>>>> upstream/master` markers.
2. Apply the resolution rule:
   - **Tabby-owned customisations** (anything under `flutter/lib/custom/`, our PowerStrip / FloatingMacroBar / session work, `floating_mouse.dart`): keep the Tabby version (above `=======`). Discard the upstream version unless it adds a brand-new field/method we don't have and that doesn't conflict semantically — in that case, additively merge.
   - **Shared common-layer files** (`flutter/lib/common.dart`, `common/widgets/*`, `consts.dart`): if upstream added a new constant / method, keep both. If both sides edited the same line of an existing function, prefer Tabby unless upstream's edit is a security/correctness fix — in that case, take upstream and verify Tabby's behaviour still works in the smoke test.
   - **Pure additions on either side**: keep both.
3. Remove all conflict markers.
4. Save the file.
5. `git add <path>` to mark resolved.

- [ ] **Step 4: Verify all conflicts resolved**

```sh
git status
```

Expected: no `Unmerged paths:` section. `Changes to be committed:` should list every file from Step 2 plus everything else the merge brought in.

- [ ] **Step 5: Complete the merge commit**

If Step 1 produced conflicts (outcome B), git is currently paused mid-merge. Complete it:

```sh
git commit --no-edit
```

`--no-edit` accepts the default merge message git already prepared (which includes our `-m` text from Step 1 plus the list of merged commits).

Expected: `[chore/sync-upstream-2026-05-31 <new-sha>] chore: merge upstream rustdesk/master into Tabby (sync 2026-05-31)` plus diffstat.

If Step 1 was outcome A (clean), the commit already exists — skip this step.

- [ ] **Step 6: Sanity-check the merge commit**

```sh
git log -1 --pretty=format:'%h %s%n  parents: %p%n'
```

Expected: shows two parent SHAs (one Tabby, one upstream — confirms this is a merge commit, not a fast-forward).

---

## Task 5: Update the submodule

**Files:**
- Modify: working-tree contents of `libs/hbb_common/` (the submodule's checked-out files, not the gitlink in the parent repo).

- [ ] **Step 1: Confirm what the merged tree expects**

```sh
git ls-tree HEAD libs/hbb_common
```

Expected: `160000 commit 2e9f641101c6bfbd1f4ca42a249bef7c14e52f5b\tlibs/hbb_common` (or a newer SHA if upstream advanced between plan-writing and execution).

Record this SHA — call it `EXPECTED_SUBMODULE_SHA`.

- [ ] **Step 2: Fetch the target ref into the submodule**

```sh
cd libs/hbb_common
git fetch origin 2e9f641101c6bfbd1f4ca42a249bef7c14e52f5b
```

(Substitute `EXPECTED_SUBMODULE_SHA` from Step 1 if different.)

Expected: `* branch <sha> -> FETCH_HEAD`, exit code 0.

If this fails with "not our ref": the submodule's remote may have changed. Run `git remote -v` and verify origin is `ssh://git@github.com/rustdesk/hbb_common`. If origin is correct and the ref still won't fetch, STOP — escalate to user before guessing.

- [ ] **Step 3: Run the recursive submodule update from repo root**

```sh
cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby
git submodule update --init --recursive
```

Expected: either silent success, or output like `Submodule path 'libs/hbb_common': checked out '2e9f641…'`. Exit code 0.

- [ ] **Step 4: Verify the submodule pointer matches**

```sh
git status
git diff --submodule
```

Expected `git status`: no changes to `libs/hbb_common` (the gitlink in the parent repo matches the working-tree checkout).

If `git status` shows `libs/hbb_common (new commits)` or similar, the working tree drifted from the merged pointer. Run `cd libs/hbb_common && git checkout EXPECTED_SUBMODULE_SHA && cd ..` and re-check.

---

## Task 6: Post-merge validation — Flutter

**Files:**
- Read-only build verification.

- [ ] **Step 1: Flutter pub get on the merged tree**

```sh
cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter
flutter pub get
```

Expected: `Got dependencies!` (or `Resolving dependencies...` followed by no errors). Exit code 0.

If a new transitive dep introduced a version conflict, surface the error — do not silently downgrade Tabby's pinned versions.

- [ ] **Step 2: Flutter analyze**

```sh
flutter analyze
```

Expected: exit code 0. Compare the issue list against the Task 2 Step 1 baseline. **Any new error must be investigated.** New `info` or `warning` items are acceptable but should be noted in the PR description.

If new errors appear:
1. Read each error.
2. If it points to a file we conflict-resolved in Task 4, re-examine that resolution.
3. If it points to a common-layer file upstream changed, our resolution may need adjustment.
4. Fix in place, then re-run `flutter analyze` until it passes.

- [ ] **Step 3: Flutter iOS build**

```sh
flutter build ios --no-codesign
```

Expected: `Built …/Runner.app`, exit code 0.

If the build fails:
- Read the first error (Xcode errors cascade; the first one is usually root cause).
- Common post-sync issues: missing iOS-side glue for a new common-layer feature, podfile mismatch, FFI signature drift.
- Fix in place. If the fix is large or touches new upstream subsystems, STOP and ask the user — do not autonomously redesign integration code.

---

## Task 7: Post-merge validation — Rust

**Files:**
- Read-only build verification.

- [ ] **Step 1: Cargo check on the merged tree**

```sh
cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby
cargo check
```

Expected: exit code 0, parity with Task 2 Step 3 baseline.

If `cargo check` errors with code-level errors (not missing-C-library environment errors): a Rust API drifted. Investigate. If the error is in a file Tabby has not edited (pure upstream territory like `src/server/`), the issue is likely a transitive Cargo.lock mismatch — try `cargo update -p <crate>` only for the failing crate, never blanket `cargo update`.

If `cargo check` errors with the same environment errors as in Task 2 Step 3 (vcpkg/sciter), that's the same baseline gap — note and proceed. Do not bring vcpkg into scope; it's separate from this sync.

---

## Task 8: Cross-check — confirm no upstream leakage into Tabby custom tree

**Files:**
- Read-only inspection.

- [ ] **Step 1: List any upstream commits that touched `flutter/lib/custom/`**

```sh
git log --oneline upstream/master..HEAD -- flutter/lib/custom/
```

Expected: only Tabby commits (the 256 pre-merge commits plus the merge commit itself if it touched anything under `custom/`). **No commits from the upstream side of the merge should appear here**, because upstream doesn't know about our `custom/` tree.

If an upstream-authored commit appears in the output: something weird happened (maybe a misnamed file). Inspect the commit, decide whether to revert just that change or accept it. Do not silently leave unexplained upstream edits in our custom tree.

- [ ] **Step 2: Spot-check that Tabby-owned files still contain Tabby code**

```sh
git diff main..HEAD -- flutter/lib/custom/strip/widgets/power_strip.dart | head -40
git diff main..HEAD -- flutter/lib/custom/overlay/floating_macro_bar.dart | head -40
```

Expected: both diffs are empty (the merge didn't touch either file). If non-empty, inspect to confirm the changes are intentional (e.g. a common-layer rename that propagated).

---

## Task 9: iOS simulator smoke test

**Files:**
- Manual interactive test on iOS simulator.

This task is the only one that requires human-in-the-loop interaction. If running this plan with subagent-driven-development, surface the simulator app and the test checklist to the user.

- [ ] **Step 1: Launch the app on simulator**

```sh
cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter
open -a Simulator
flutter run -d "iPhone 15 Pro"
```

(Substitute whichever simulator device name you have. `flutter devices` lists them.)

Expected: app launches without crash, lands on the session-list / connect screen.

If the app crashes on launch: capture the Xcode/Console crash log. Don't bury it — surface to the user.

- [ ] **Step 2: Verify core UI renders**

Manually check, in order:

1. **Session list / connect screen** — peer list (if any) renders, "Connect" input accepts text.
2. **PowerStrip** — open a session (or use the demo screen if available); confirm the strip renders, default layout shows expected keys, modifier keys (cmd/ctrl) cycle through states correctly.
3. **FloatingMacroBar** — confirm it renders, scrollable, taps register, recently-added shortcuts (`/exit`, `appr`, `c+p`, `c+p+d`) are present.
4. **Peer connection** — connect to a known test peer. Confirm session establishes, keyboard input works, two-finger scroll behaves correctly (reversed per recent fix), file-send button routes to FileManagerPage.

Pass = all four work without regression from pre-merge behaviour.

- [ ] **Step 3: Capture any regressions**

If anything is broken:
1. Identify whether the break is in a file we conflict-resolved (Task 4) or a file we did not touch.
2. If we did not touch it, the upstream change broke it — surface to user before patching; this may need a conscious decision about which upstream commit to revert or partially apply.
3. If we did touch it, our resolution was wrong — fix in place and re-run from this step.

- [ ] **Step 4: Kill the simulator session**

Press `q` in the `flutter run` terminal, or `Ctrl+C`. Confirm the app process exits cleanly.

---

## Task 10: Push and open PR

**Files:**
- Modify: remote `origin` ref for `chore/sync-upstream-2026-05-31`; create PR on GitHub.

- [ ] **Step 1: Push the branch**

```sh
cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby
git push -u origin chore/sync-upstream-2026-05-31
```

Expected: `* [new branch] chore/sync-upstream-2026-05-31 -> chore/sync-upstream-2026-05-31`, plus tracking confirmation.

- [ ] **Step 2: Draft the PR body**

Write the body to `/tmp/sync-pr-body.md` (don't commit this file — it's just for `gh pr create`):

```markdown
## Summary

Routine upstream sync: merges 41 commits from `rustdesk/rustdesk@master` (`5439ec38b..fa369365a`) into Tabby's `main`. Last sync was `c2ebed59f` (2026-04-29 timeframe).

Merge commit on a branch — Tabby's 256 commits are preserved as-is.

## Notable upstream changes in this batch

- **Password encryption refactor** — `1f26e452f refact(password): encrypt`
- **Toolbar drag + four-edge snap** — `6ad56075d Drag whole toolbar; snap to all four edges of the remote session window`
- **IPC scoping fixes** — `bc2c36215`, `78e8134ad`, `bb51c6aa4`
- **Wayland/COSMIC screencast fix** — `377547fa1 scrap/wayland: insert videoconvert to fix screencast on COSMIC / DMA-BUF portals`
- **Packaging — time64 support** — `e5fa40e90`
- **CI hardening** — `81e7d27ec`, `f3fc0b5ac`, `c19a0ceba` (action pinning, `cargo build --locked`)
- **Translation updates** — Portuguese, Dutch, German, Italian, Russian, Turkish, Korean, French, Latvian, Polish

## Submodule

- `libs/hbb_common` advanced from `c5f25e17…` to `2e9f6411…` (upstream merge of PR #544).

## Conflict resolution notes

<FILL IN POST-MERGE — list each conflicting file and which side was favoured, with one-line reason.>

## Validation performed

- [x] `flutter analyze` — no new errors vs pre-merge baseline
- [x] `flutter build ios --no-codesign` — green
- [x] `cargo check` — green (parity with baseline)
- [x] iOS simulator smoke test — launch / session list / PowerStrip / FloatingMacroBar / peer connection all pass
- [x] `git log upstream/master..HEAD -- flutter/lib/custom/` — confirmed no upstream leakage

## Out of scope (intentionally not in this PR)

- TestFlight build bump (will be a separate commit on `main` after this merges)
- Adopting upstream's new password encryption module in our session code
- Cherry-picking individual upstream commits (full catch-up was the explicit choice)

## Rollback

If something breaks post-merge on `main`: `git revert -m 1 <merge-sha>` produces a clean revert.
```

Replace the `<FILL IN POST-MERGE>` block with the actual conflict-resolution notes from Task 4 before opening the PR.

- [ ] **Step 3: Open the PR**

```sh
gh pr create \
  --base main \
  --head chore/sync-upstream-2026-05-31 \
  --title "chore: sync upstream rustdesk/master (2026-05-31)" \
  --body-file /tmp/sync-pr-body.md
```

Expected: `gh` prints the PR URL.

- [ ] **Step 4: Clean up the temp body file**

```sh
rm /tmp/sync-pr-body.md
```

- [ ] **Step 5: Hand off to user**

Report the PR URL to the user. **Do not merge the PR.** The user reviews and merges manually.

---

## Rollback procedures (reference, not steps)

**Before Task 10 (branch not pushed):**
```sh
git checkout main
git branch -D chore/sync-upstream-2026-05-31
```
Restores world to pre-Task 3 state. Submodule may need `git submodule update --init --recursive` to revert to `main`'s pointer.

**After Task 10 but before user merges:**
- Close the PR on GitHub without merging.
- `git push origin --delete chore/sync-upstream-2026-05-31` (deletes remote branch).
- Same local cleanup as above.

**After user merges to `main`:**
- `git revert -m 1 <merge-sha>` on `main`, push, open a follow-up PR.
- Do **not** force-push or rewrite `main`.

---

## Success criteria (from spec)

- [ ] `chore/sync-upstream-2026-05-31` branch exists, contains exactly one merge commit on top of pre-merge `main`.
- [ ] `flutter analyze` reports no new errors vs. pre-merge baseline.
- [ ] `flutter build ios --no-codesign` succeeds.
- [ ] `cargo check` succeeds (parity with pre-merge baseline).
- [ ] iOS simulator smoke test passes (launch, session list, PowerStrip, FloatingMacroBar, peer connection).
- [ ] `git log --oneline upstream/master..HEAD -- flutter/lib/custom/` shows no upstream commits leaked into our custom tree.
- [ ] PR opened against `RonenMars/Tabby:main`, awaiting user merge.
