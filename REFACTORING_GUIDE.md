# RustDesk Refactoring Guide

## Overview

This document provides analysis and recommendations for improving code quality in RustDesk.

---

## 1. server/connection.rs (5059 lines)

### Statistics:
- **79 `unwrap()` calls** 
- **106 `clone()` calls**

### Analysis:

#### Safe `unwrap()` patterns (no action needed):
| Pattern | Count | Risk | Reason |
|---------|-------|------|--------|
| `.lock().unwrap()` | 26 | ✅ Low | Panic on poisoned mutex is acceptable |
| `.read().unwrap()` | 12 | ✅ Low | Same as above |
| `.write().unwrap()` | 12 | ✅ Low | Same as above |

#### Potentially unsafe `unwrap()` (review needed):
| Line | Code | Recommendation |
|------|------|----------------|
| 3049 | `NonZeroI64::new(get_time()).unwrap()` | Safe - time is always > 0 |
| 1351, 2277, 2288 | Various Option unwraps | Consider `unwrap_or_default()` |
| 2988, 2992 | Parser unwraps | Add error handling |

#### Clone optimization opportunities:
| Line | Current | Suggestion |
|------|---------|------------|
| 357 | `tx_from_cm_holder.clone()` | Required - used in async |
| 822 | `pos.clone()` | Could use `&pos` |
| 586-634 | `conn.inner.clone()` | Required for subscription |

### Recommendation:
**Priority: LOW** - Most unwraps are on mutex locks which is idiomatic Rust. Refactoring would add complexity without significant benefit.

---

## 2. client.rs (88 unwrap, 72 clone)

### Key patterns:
- Channel send/receive operations
- Configuration parsing
- Network operations

### Recommendation:
**Priority: MEDIUM** - Network-related unwraps could benefit from proper error handling.

---

## 3. ui_session_interface.rs (97 unwrap, 3 TODO)

### TODOs in file:
1. Line 875: `// flutter only TODO new input`
2. Line 1090: `// flutter only TODO new input`  
3. Line 1422: `// TODO: can add a confirm dialog`

### Recommendation:
**Priority: LOW** - TODOs are feature requests, not bugs.

---

## 4. platform/windows.rs (84 unsafe blocks)

### Categories:
| Category | Count | Risk |
|----------|-------|------|
| Windows API calls | ~60 | Normal for Windows interop |
| Memory operations | ~15 | Review needed |
| FFI callbacks | ~9 | Normal |

### Recommendation:
**Priority: LOW** - Unsafe blocks are necessary for Windows API interop. Adding `// SAFETY:` comments would improve maintainability.

---

## 5. Dead Code (#[allow(dead_code)])

Found 53 instances. Categories:
- Platform-specific code (conditionally compiled)
- Future features
- Test utilities

### Recommendation:
**Priority: LOW** - Most are intentional for cross-platform support.

---

## Summary

| File | Priority | Action |
|------|----------|--------|
| server/connection.rs | LOW | Document, don't change |
| client.rs | MEDIUM | Add error handling for network ops |
| ui_session_interface.rs | LOW | Complete TODOs as features |
| platform/windows.rs | LOW | Add SAFETY comments |
| dead_code | LOW | Review during cleanup |

### Safe Refactoring Approach:
1. **Don't touch** mutex/rwlock unwraps - they're idiomatic
2. **Add error handling** for network/file operations
3. **Add tests first** before any major refactoring
4. **Document** unsafe blocks with `// SAFETY:` comments

---

## Code Quality Metrics

```
Total .rs files: 172
Total lines of Rust: ~150,000
unwrap() total: ~500
clone() total: ~800
unsafe blocks: ~200
```

The codebase is reasonably well-written. Most patterns follow Rust idioms.






