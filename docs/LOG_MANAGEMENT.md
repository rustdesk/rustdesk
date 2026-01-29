# Log File Management

## Overview

RustDesk uses `flexi_logger` for logging with automatic log rotation. Logs are written to platform-specific directories and rotated to prevent excessive disk usage.

## Current Implementation

### Log Locations

- **macOS**: `~/Library/Logs/RustDesk/`
- **Linux**: `~/.local/share/logs/RustDesk/`
- **Android**: `{APP_HOME}/RustDesk/Logs/`
- **Windows**: Config directory → `log/` subdirectory

### Current Rotation Policy

The logging system (implemented in `libs/hbb_common/src/lib.rs`) uses size-based rotation:

```rust
.rotate(
    // Rotate logs daily OR when they reach 100MB (whichever comes first)
    Criterion::AgeOrSize(Age::Day, 100_000_000),
    Naming::Timestamps,
    Cleanup::KeepLogFiles(31),
)
```

### Benefits

1. **Bounded Disk Usage**: Maximum log storage becomes ~3.1GB (31 files × 100MB)
2. **Maintained Organization**: Daily rotation still occurs for time-based organization
3. **Prevents Runaway Logs**: Heavy activity days can't create multi-gigabyte single files
4. **Automatic Cleanup**: Old files still automatically deleted after 31 days

### Implementation

The implementation is in the `hbb_common` library:

**File**: `libs/hbb_common/src/lib.rs`  
**Function**: `init_log()`  
**Line**: ~407

```diff
- Criterion::Age(Age::Day),
+ // Rotate logs daily OR when they reach 100MB to prevent excessive disk usage
+ // With 31 files max, this limits total log storage to ~3.1GB
+ Criterion::AgeOrSize(Age::Day, 100_000_000),
```

## Tuning Parameters

The 100MB size limit can be adjusted based on deployment needs:

- **50MB** (`50_000_000`): More conservative, max ~1.5GB total
- **100MB** (`100_000_000`): Balanced approach, max ~3.1GB total  ✅ Recommended
- **200MB** (`200_000_000`): Permissive, max ~6.2GB total

## Monitoring

Users can monitor log disk usage at the locations listed above. The rotation ensures:

1. Logs older than 31 days are automatically deleted
2. Individual files never exceed the configured size limit
3. Total disk usage is bounded and predictable

## Testing

To verify the rotation works correctly:

1. Check log directory before and after rotation
2. Verify old files are cleaned up after 31 days
3. Monitor file sizes don't exceed the configured limit
4. Ensure logs still contain all necessary debugging information

## References

- flexi_logger documentation: https://docs.rs/flexi_logger/
- RustDesk logging implementation: `libs/hbb_common/src/lib.rs`
