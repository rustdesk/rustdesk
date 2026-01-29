# Patches for hbb_common

This directory contains patches for reference. The changes described have already been applied to the `hbb_common` submodule in this PR.

## hbb_common-log-rotation.patch

**Status**: ✅ Already applied in this PR (submodule commit 0c401fd)

**Purpose**: Add size-based log rotation to prevent excessive disk usage

**Apply to**: `libs/hbb_common` submodule  
**Target repository**: https://github.com/rustdesk/hbb_common

### Reference Information

This patch file is provided for:
- Documentation of the exact changes made
- Reference for maintainers
- Potential cherry-picking to other branches if needed

The changes have already been implemented in the submodule updated by this PR.

### What it does

- Changes log rotation from age-only to age-or-size based
- Rotates logs when they reach 100MB OR daily (whichever comes first)
- Limits total log storage to ~3.1GB (31 files × 100MB max each)
- Prevents runaway log files from consuming excessive disk space

### Testing

After applying the patch and rebuilding:

1. Verify logs are created in the standard location
2. Check that individual log files don't exceed 100MB
3. Confirm old files are still cleaned up after 31 days
4. Ensure log content is still complete and useful

See `../LOG_MANAGEMENT.md` for complete documentation.
