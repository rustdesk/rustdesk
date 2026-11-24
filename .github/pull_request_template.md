<!--
Read these before opening a PR. They apply whether the code is AI-generated or written by hand:
- Feature PRs need maintainer approval first: https://github.com/rustdesk/rustdesk/discussions/17223
- End-to-end testing on real devices is mandatory: https://github.com/rustdesk/rustdesk/discussions/17225

PRs that skip these requirements, leave the sections below empty, or have an
unjustifiably large regression surface may be closed without review.
Translation-only, docs-only, and CI-only PRs may skip the test, AI cross-review, and regression sections.
-->

## Type

- [ ] Bug fix
- [ ] Feature / enhancement
- [ ] Translation / documentation / CI

## Linked issue or approval

<!--
Bug fix: link the issue (Fixes #...).
Feature: link the discussion where a maintainer approved it. Feature PRs without
prior approval may be closed without review. Approval of the idea does not
guarantee that the implementation will be accepted.
-->

Fixes #

Feature approval ([required for features](https://github.com/rustdesk/rustdesk/discussions/17223)):

## What and why

<!-- The problem, the root cause (for bug fixes), and how this change addresses it. Keep it short. -->

## End-to-end test on real devices

<!--
Compiling, unit tests, CI, code reading, and AI analysis are not substitutes for this.
If you could not reproduce the issue or lack the environment, say so below.
Do not claim a fix based on reasoning alone.
-->

**Environment** (each device: OS and version, controlling or controlled side; direct or relay; public or self-hosted server):

-

**Reproduced on master** (bug fixes: how you triggered the original issue):

**Steps and actual results with this PR:**

1.

**Not tested / could not reproduce:**

## Screenshots / video

<!--
Required if this PR adds, changes, or affects any UI, including side effects on
existing UI. Upload a video or screenshots from the real devices above that prove
the change; for changes to existing UI, show before and after.
Write "No UI impact" if there is none.
-->

## AI cross-review

<!--
Have this diff reviewed by AI models other than the one that wrote it.
Reviewers can be wrong or hallucinate, so do not apply findings blindly: judge
each one yourself. Fix the real issues; a finding you leave unfixed is fine if
you can explain why.
As proof, paste each model's review output (or a share link to the conversation)
and, for each finding, what you decided and why. Put long output in a <details> block.
-->

**Models used:**

**Review output, and your decision on each finding:**

## Regression surface

<!--
List the existing files and runtime paths whose behavior changes, why each change
is unavoidable, and which existing functionality around them you re-tested.
Cover backward compatibility (older clients/servers) and other platforms.
New code that only runs when the new feature is used does not need to be listed.
-->

## Known limits

<!-- Optional: corner cases deliberately left unhandled. -->

## Checklist

- [ ] Features: a maintainer approved this in a discussion before I started, as required by [#17223](https://github.com/rustdesk/rustdesk/discussions/17223). Unapproved feature PRs may be closed without review.
- [ ] I tested this end to end on real devices on every platform it touches, as required by [#17225](https://github.com/rustdesk/rustdesk/discussions/17225), and the results above come from those runs.
- [ ] Bug fixes: I reproduced the original issue on master and confirmed this PR fixes it.
- [ ] UI added, changed, or affected: I uploaded a video or screenshots above that prove the change.
- [ ] I re-tested the existing functionality listed under "Regression surface".
- [ ] The diff contains only what this change needs: no unrelated refactoring, formatting, renames, or drive-by fixes.
- [ ] I had this diff cross-reviewed by different AI models, not only the one that wrote it, and included their review output above. I judged each finding myself instead of applying it blindly: I fixed the real issues and can explain why I left the others.
