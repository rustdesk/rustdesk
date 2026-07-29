# macOS Physical-Footprint Watchdog Design

## Problem

RDH's macOS `--server` memory watchdog currently compares the configured
threshold against `sysinfo::Process::memory()`, which reports resident memory.
The observed long-running leak can move most allocated pages into compressed
memory or swap, leaving RSS below the threshold while macOS `phys_footprint`
continues to grow. A daily RSS check therefore misses the condition it is meant
to contain.

## Scope

Replace only the watchdog's memory measurement and related log terminology.
Preserve all existing recovery policy:

- the exact launchd-supervision gate;
- the `rdh-memory-restart-threshold-mib` option and 1024 MiB default;
- one daily check at 06:00 local time;
- the 00:00 through 06:59 unattended window;
- intentional disregard of active remote connections in that window;
- exit code 75 so the existing user LaunchAgent relaunches only `--server`;
- no installation, service restart, or change to window targeting.

## Measurement

Call macOS's public `proc_pid_rusage` API for the current process using
`RUSAGE_INFO_V0`, then compare `ri_phys_footprint` with the existing byte
threshold.

`proc_pid_rusage` has been available since macOS 10.9, below RDH's macOS 10.14
deployment target. `RUSAGE_INFO_V0` already contains `ri_phys_footprint`, so the
implementation does not need a newer flavor, a new Rust dependency, an
Objective-C++ bridge, or a `vmmap` subprocess.

The Rust representation of `rusage_info_v0` will use `#[repr(C)]` and only the
fields defined by the public SDK structure. The unsafe FFI call will be isolated
inside one focused helper.

## Failure behavior

If `proc_pid_rusage` fails, the watchdog logs the operating-system error and
skips that day's restart decision. It does not fall back to RSS because RSS is
known to produce a false negative for this leak. The next scheduled daily check
tries the public API again.

## Tests and verification

Implementation follows a red-green sequence:

1. Tighten the RDH source-contract test so the current RSS implementation fails
   and the physical-footprint API and terminology are required.
2. Add focused Rust tests for the C layout assumptions and a live current-process
   query returning a nonzero footprint on macOS.
3. Replace the RSS helper and update watchdog logs with the minimum production
   change needed to pass.
4. Run the focused Rust tests, the RDH source-contract test, formatting checks,
   and the existing macOS CI build.

A local public-API probe already confirmed that
`proc_pid_rusage(...).ri_phys_footprint` matched `vmmap` exactly for the running
RDH server.

## Acceptance criteria

- No watchdog decision or log refers to RSS.
- The threshold is compared against current `phys_footprint`.
- Measurement failure cannot trigger a restart and cannot silently fall back to
  RSS.
- Scheduling, launchd gating, threshold configuration, exit behavior, and
  window-targeting code remain unchanged.
- Focused tests and the macOS build pass.
- Building and verification do not install or restart the candidate.
