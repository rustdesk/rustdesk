# Patches for hbb_common

This directory contains patches that should be applied to the `hbb_common` library submodule.

## hbb_common-log-rotation.patch

**Purpose**: Add size-based log rotation to prevent excessive disk usage

**Apply to**: `libs/hbb_common` submodule  
**Target repository**: https://github.com/rustdesk/hbb_common

### How to apply

#### Option 1: Apply in hbb_common repository

```bash
cd /path/to/hbb_common
git apply /path/to/rustdesk/docs/patches/hbb_common-log-rotation.patch
git commit -m "Add size-based log rotation to prevent excessive disk usage"
```

#### Option 2: Apply in rustdesk repository submodule

```bash
cd libs/hbb_common
git apply ../../docs/patches/hbb_common-log-rotation.patch
git commit -m "Add size-based log rotation to prevent excessive disk usage"
# Note: This creates a local commit in the submodule
```

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
