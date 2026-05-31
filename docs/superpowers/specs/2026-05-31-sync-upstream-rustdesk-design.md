# Sync Tabby fork with upstream rustdesk/master — 2026-05-31

## Goal

Bring all upstream `rustdesk/rustdesk` commits between our last sync
(`c2ebed59f`, "Merge upstream rustdesk/master into Tabby") and current
`upstream/master` (`fa369365a`) into Tabby's `main` via a single merge
commit on a feature branch.

- **Commits to absorb:** 41 (`5439ec38b..fa369365a`)
- **Our divergence:** 256 commits of Tabby work ahead
- **Strategy:** merge commit on a branch → reviewable PR → user merges to `main`
- **Tabby history:** preserved untouched (no rebase, no squash)

## Why merge commit (not rebase, not squash-rebase)

| | Merge commit (chosen) | Rebase | Squash + rebase |
|---|---|---|---|
| Conflict resolution passes | 1 | up to 256 | 1 |
| Tabby commit history | intact | replayed (new SHAs) | collapsed into 1 commit |
| Rewrites `main` | no | yes (force-push) | yes (force-push) |
| `git blame` / `git bisect` on Tabby work | works | works | broken |
| Revertable | `git revert -m 1 <merge>` | hard | hard |
| Consistent with last sync (`c2ebed59f`) | yes | no | no |

Decision: merge commit. Same conflict economy as squash-rebase without
destroying Tabby authorship history; matches the project's existing
sync pattern.

## Pre-flight checks

1. **Clean working tree** — `git status` must show no staged/unstaged
   changes. The untracked `.claude/plans/` directory is fine.
2. **Upstream fetched** — `upstream/master` resolves to `fa369365a`.
   Already verified.
3. **Submodule ref reachable** — `libs/hbb_common` target is
   `2e9f641101c6bfbd1f4ca42a249bef7c14e52f5b` (merge of upstream PR
   #544). The earlier `c7f5567…` "not our ref" error was a transient
   complaint about an *intermediate* commit's submodule pointer during
   recursive fetch; the final target ref is fully fetchable from
   `rustdesk/hbb_common origin` and has already been pulled into the
   submodule's local object store.
4. **Baseline SHA recorded** — capture `git rev-parse main` before any
   operation so we can `git reset --hard <sha>` if the merge goes
   sideways.
5. **Pre-merge build is green** — run `cd flutter && flutter build ios
   --no-codesign` *before* merging so any post-merge failure can be
   attributed to the merge, not a pre-existing break.

## Conflict surface (predicted)

Cross-referencing the 195 files upstream touched against files our 256
Tabby commits modified:

- **Likely conflict:** `flutter/lib/mobile/widgets/floating_mouse.dart`
  — only file in our active Tabby UI work that overlaps upstream's
  change set.
- **Possible conflicts:** `flutter/lib/common.dart`,
  `flutter/lib/common/widgets/remote_input.dart`,
  `flutter/lib/common/widgets/toolbar.dart`,
  `flutter/lib/common/widgets/dialog.dart`,
  `flutter/lib/consts.dart` — common-layer files we have touched and
  upstream may have touched too. Confirm during the actual merge.
- **No expected conflict:** `src/server/`, `src/platform/windows/`,
  `src/platform/linux*.rs`, `src/lang/*.rs`, `.github/workflows/*`,
  Cargo files. These are upstream-only territory for us.

Total expected conflicting files: **under 10**, resolvable in one pass.

## Procedure

### Branch creation and merge

```sh
# from clean main
git checkout -b chore/sync-upstream-2026-05-31

# the merge — --no-ff guarantees a merge commit even if a fast-forward
# were theoretically possible (it isn't here)
git merge upstream/master --no-ff \
  -m "chore: merge upstream rustdesk/master into Tabby (sync 2026-05-31)"
```

If conflicts:
1. Resolve them file-by-file, favouring Tabby behaviour where the two
   diverge intentionally (e.g. our `floating_mouse.dart` customisations
   over upstream's mouse code).
2. For files that are pure additions by either side, keep both
   additions.
3. `git add <resolved files>` then `git commit` (no `-m`, let it use
   the default merge message which lists all merged commits).

### Submodule update

```sh
git submodule update --init --recursive
git status  # should show no changes; submodule pointer matches the merged tree
```

### Validation

```sh
cd flutter
flutter pub get
flutter analyze
flutter build ios --no-codesign
```

Then in a separate shell from repo root:

```sh
cargo check
```

Smoke test on iOS simulator:
1. App launches without crash.
2. Session list renders.
3. PowerStrip and FloatingMacroBar render and respond.
4. Can establish a connection to a known peer.

### Cross-checks

```sh
# Verify we didn't accidentally land any change in our custom/ tree
git log --oneline upstream/master..HEAD -- flutter/lib/custom/

# Should be empty (or only show our pre-merge Tabby commits, none from
# the merge itself).
```

### Land it

```sh
git push origin chore/sync-upstream-2026-05-31
```

Open PR via `gh pr create` with title:
`chore: sync upstream rustdesk/master (2026-05-31)`

PR body should call out notable upstream changes the reviewer should
know about:
- Password encryption refactor (`1f26e452f refact(password): encrypt`)
- Toolbar drag + four-edge snap (`6ad56075d`)
- IPC scoping fixes (`bc2c36215`, `78e8134ad`, `bb51c6aa4`)
- Wayland/COSMIC screencast fix (`377547fa1`)
- ScreencaptureKit / hwcodec build pinning changes
- Multiple translation updates (Portuguese, Dutch, German, Italian,
  Russian, Turkish, Korean, French)

User reviews and merges the PR. **Claude does not merge.**

## Rollback

**Before merge to `main`:** the branch is disposable. Either:
- `git checkout main && git branch -D chore/sync-upstream-2026-05-31`,
  or
- `git reset --hard <baseline-sha>` on the branch.

**After merge to `main`:**
- `git revert -m 1 <merge-sha>` produces a clean revert commit that
  restores pre-merge state without rewriting history.

## Out of scope (explicit)

- **TestFlight build bump** — separate, deliberate step after sync is
  verified. Do not bump in this PR.
- **Cherry-picking specific upstream commits** — full catch-up was
  chosen; we take all 41.
- **Docs/branding alignment** — only conflict-driven edits to our docs.
  No proactive rewrites to match upstream wording.
- **Independent `libs/hbb_common` changes** — submodule follows
  upstream's pointer; we do not override it.
- **Refactoring our Tabby code** to match upstream's new patterns
  (e.g. adopting the new password encryption module) — that is a
  separate decision, separate PR.

## Success criteria

- [ ] `chore/sync-upstream-2026-05-31` branch exists, contains exactly
      one merge commit on top of pre-merge `main`.
- [ ] `flutter analyze` reports no new errors vs. pre-merge baseline.
- [ ] `flutter build ios --no-codesign` succeeds.
- [ ] `cargo check` succeeds.
- [ ] iOS simulator smoke test passes (launch, session list,
      PowerStrip, FloatingMacroBar, peer connection).
- [ ] `git log --oneline upstream/master..HEAD -- flutter/lib/custom/`
      shows no upstream commits leaked into our custom tree.
- [ ] PR opened against `RonenMars/Tabby:main`, awaiting user merge.
