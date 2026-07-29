# macOS Physical-Footprint Watchdog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make RDH's daily macOS `--server` watchdog compare its 1 GiB threshold against `phys_footprint` instead of RSS.

**Architecture:** Keep the existing scheduling, launchd gate, threshold option, and exit-based recovery policy intact. Isolate one public macOS `proc_pid_rusage(RUSAGE_INFO_V0)` call inside `memory_watchdog.rs`, represent the SDK structure with `#[repr(C)]`, and return an `io::Result<u64>` containing `ri_phys_footprint`.

**Tech Stack:** Rust 2021, macOS `libproc`/`sys/resource.h` public API, Python source-contract test, GitHub Actions macOS aarch64 candidate build.

## Global Constraints

- Support the repository's macOS 10.14 deployment target; `proc_pid_rusage` is available from macOS 10.9.
- Do not add a Rust dependency, Objective-C++ bridge, subprocess, fallback metric, retry loop, or new configuration option.
- Preserve the exact launchd-supervision gate, `rdh-memory-restart-threshold-mib` option, 1024 MiB default, daily 06:00 check, 00:00 through 06:59 unattended window, active-connection policy, and exit code 75.
- On measurement error, log the OS error and skip that day's decision; never fall back to RSS.
- Do not change window targeting, install an app, or restart RDH/RDO.
- Preserve the existing unstaged `implementation-notes.md` work. Update only its watchdog wording and never stage it with the implementation commit.

---

### Task 1: Replace RSS with the current process physical footprint

**Files:**
- Modify: `tests/test_herbin_branding.py:541-550`
- Modify: `src/server/memory_watchdog.rs:1-145`
- Modify: `docs/rdh-upgrade-runbook.md:34-52`
- Modify without staging: `implementation-notes.md:35-51`

**Interfaces:**
- Consumes: `rdh-memory-restart-threshold-mib`, the existing `run(threshold_bytes: u64)` loop, and launchd exit recovery.
- Produces: `fn current_phys_footprint_bytes() -> io::Result<u64>` and the private `#[repr(C)] struct RusageInfoV0`.

- [ ] **Step 1: Add source-contract assertions that reject the current RSS implementation**

In `tests/test_herbin_branding.py`, extend the existing watchdog assertions with:

```python
    assert "proc_pid_rusage" in memory_watchdog_rs
    assert "RUSAGE_INFO_V0" in memory_watchdog_rs
    assert "phys_footprint" in memory_watchdog_rs
    assert "current_rss_bytes" not in memory_watchdog_rs
    assert "sysinfo::" not in memory_watchdog_rs
    assert "rss=" not in memory_watchdog_rs
    assert "RSS" not in memory_watchdog_rs
```

- [ ] **Step 2: Add focused Rust tests before adding production definitions**

Append these tests to the existing `#[cfg(test)] mod tests` in
`src/server/memory_watchdog.rs`:

```rust
    #[test]
    fn rusage_info_v0_layout_matches_macos_abi() {
        let info = RusageInfoV0::default();
        let base = (&info as *const RusageInfoV0) as usize;
        let footprint = (&info.phys_footprint as *const u64) as usize;

        assert_eq!(std::mem::size_of::<RusageInfoV0>(), 96);
        assert_eq!(footprint - base, 72);
    }

    #[test]
    fn reads_nonzero_current_process_physical_footprint() {
        let footprint = current_phys_footprint_bytes()
            .expect("current process physical footprint should be readable");

        assert!(footprint > 0);
    }
```

- [ ] **Step 3: Run the red tests and record the expected failures**

Run:

```bash
python3 tests/test_herbin_branding.py
cargo test --lib server::memory_watchdog::tests
```

Expected:

- the Python contract fails because `proc_pid_rusage` is absent;
- the Rust test target fails to compile because `RusageInfoV0` and
  `current_phys_footprint_bytes` do not exist yet.

Do not change production code until both failures are observed for those exact
reasons.

- [ ] **Step 4: Add the minimal public-API representation and helper**

Replace the `sysinfo` imports with `std::io`, and add the following private
definitions near the watchdog constants:

```rust
const RUSAGE_INFO_V0: i32 = 0;

#[repr(C)]
#[derive(Default)]
struct RusageInfoV0 {
    _uuid: [u8; 16],
    _user_time: u64,
    _system_time: u64,
    _package_idle_wakeups: u64,
    _interrupt_wakeups: u64,
    _pageins: u64,
    _wired_size: u64,
    _resident_size: u64,
    phys_footprint: u64,
    _process_start_abstime: u64,
    _process_exit_abstime: u64,
}

extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut RusageInfoV0) -> i32;
}
```

Replace `current_rss_bytes` with:

```rust
fn current_phys_footprint_bytes() -> io::Result<u64> {
    let mut info = RusageInfoV0::default();
    let result = unsafe {
        proc_pid_rusage(
            std::process::id() as i32,
            RUSAGE_INFO_V0,
            &mut info,
        )
    };

    if result == 0 {
        Ok(info.phys_footprint)
    } else {
        Err(io::Error::last_os_error())
    }
}
```

The only unsafe operation is the FFI call; the C-compatible structure owns the
entire output buffer.

- [ ] **Step 5: Change only the watchdog measurement and terminology**

In `run`, remove the `Pid`, `System`, and process-refresh state. Replace the
current RSS read with:

```rust
        let footprint_bytes = match current_phys_footprint_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                log::error!(
                    "RDH memory watchdog could not read the --server physical footprint: {err}"
                );
                continue;
            }
        };
```

Compare `footprint_bytes` to `threshold_bytes`. Use
`phys_footprint={} MiB` in both the restart and passed-check logs. Do not alter
the surrounding schedule, unattended-window check, or `std::process::exit`.

- [ ] **Step 6: Run focused green tests**

Run:

```bash
python3 tests/test_herbin_branding.py
cargo test --lib server::memory_watchdog::tests
```

Expected: the Python source contract exits 0 and all focused watchdog Rust tests
pass, including the existing scheduling tests.

- [ ] **Step 7: Update the two watchdog documents**

In `docs/rdh-upgrade-runbook.md`:

- replace "resident memory" with "physical footprint";
- state that the value comes from public
  `proc_pid_rusage(RUSAGE_INFO_V0).ri_phys_footprint`;
- state that a read error skips the day's decision and never falls back to RSS.

In the already-dirty `implementation-notes.md`, change only the watchdog
paragraphs from RSS to physical footprint and record the same fail-closed
behavior. Leave that file unstaged.

- [ ] **Step 8: Run the complete local verification gate**

Run:

```bash
python3 tests/test_herbin_branding.py
cargo test --lib server::memory_watchdog::tests
cargo fmt -- --check
git diff --check
git diff --cached --check
if rg -n "RSS|rss=|current_rss_bytes|sysinfo::" \
  src/server/memory_watchdog.rs; then
  exit 1
fi
git status --short
```

Expected:

- tests and formatting exit 0;
- the `rg` command returns no matches;
- only the four planned files plus the plan/spec commits are in scope;
- `implementation-notes.md` remains unstaged.

- [ ] **Step 9: Commit the implementation without the dirty notes file**

Run:

```bash
git add \
  src/server/memory_watchdog.rs \
  tests/test_herbin_branding.py \
  docs/rdh-upgrade-runbook.md
git diff --cached --check
git diff --cached --name-status
git commit -m "fix: monitor macOS physical footprint"
```

Expected staged files: exactly the three listed paths. Do not stage
`implementation-notes.md`.

---

### Task 2: Verify the committed candidate in macOS CI without installing it

**Files:**
- Verify: `.github/workflows/codex-macos-herbin.yml`
- Verify: the GitHub Actions run metadata for RDH revision 11

**Interfaces:**
- Consumes: the committed Task 1 source on branch `rdh/1.4.9`.
- Produces: a successful `RDH macOS Candidate Build` run whose source SHA equals the local implementation commit.

- [ ] **Step 1: Recheck the exact push scope**

Run:

```bash
git status --short
git log --oneline fork/rdh/1.4.9..HEAD
git diff --stat fork/rdh/1.4.9..HEAD
```

Expected: the design, plan, existing RDH.10 source commit, and physical-footprint
implementation are the only commits ahead; the unstaged notes file is not part
of the push.

- [ ] **Step 2: Push only the current RDH branch**

This external Git operation must be performed by the root/controller agent, not
an implementation or review subagent:

```bash
git push fork rdh/1.4.9
```

- [ ] **Step 3: Dispatch the existing macOS candidate workflow**

The root/controller agent records the prior run, dispatches the workflow, and
waits for a new run ID:

```bash
previous_run_id="$(
  gh run list \
    --repo Herbin-s/rustdesk \
    --workflow codex-macos-herbin.yml \
    --branch rdh/1.4.9 \
    --event workflow_dispatch \
    --limit 1 \
    --json databaseId \
    --jq '.[0].databaseId // empty'
)"

gh workflow run codex-macos-herbin.yml \
  --repo Herbin-s/rustdesk \
  --ref rdh/1.4.9 \
  -f source_ref=rdh/1.4.9 \
  -f rdh_revision=11

run_id=""
for attempt in $(seq 1 30); do
  run_id="$(
    gh run list \
      --repo Herbin-s/rustdesk \
      --workflow codex-macos-herbin.yml \
      --branch rdh/1.4.9 \
      --event workflow_dispatch \
      --limit 1 \
      --json databaseId \
      --jq '.[0].databaseId // empty'
  )"
  if [[ -n "$run_id" && "$run_id" != "$previous_run_id" ]]; then
    break
  fi
  sleep 2
done

test -n "$run_id"
test "$run_id" != "$previous_run_id"
gh run watch "$run_id" --repo Herbin-s/rustdesk --exit-status
gh run view "$run_id" --repo Herbin-s/rustdesk \
  --json conclusion,headSha,url,workflowName
```

Expected: workflow `RDH macOS Candidate Build`, conclusion `success`, and
`headSha` equal to the pushed implementation commit.

- [ ] **Step 4: Report the verified boundary**

Report the local red-green evidence, implementation commit, CI run URL, source
SHA, and that no artifact was installed and no RDH/RDO process was restarted.
Leave RDH.10 running until the user separately authorizes installation.
