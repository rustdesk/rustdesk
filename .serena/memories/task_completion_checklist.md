# RustDesk Task Completion Checklist

## When Completing a Coding Task

### 1. Code Quality
- [ ] Follow Rust/Flutter code style conventions
- [ ] Use appropriate feature flags if needed
- [ ] Ensure platform-specific code is properly isolated
- [ ] Add documentation comments for public APIs

### 2. Testing
```bash
# Run Rust tests
cargo test

# Run Flutter tests (if UI changes)
cd flutter && flutter test
```

### 3. Build Verification
```bash
# For Rust changes
cargo build --release

# For Flutter changes
python3 build.py --flutter
```

### 4. Formatting and Linting
```bash
# Format Rust code
cargo fmt

# Check Rust code
cargo clippy

# Format Flutter code (if applicable)
cd flutter && flutter format lib/

# Analyze Flutter code
cd flutter && flutter analyze
```

### 5. Commit Guidelines
- Commits should be small and independently correct
- Use DCO sign-off: `git commit -s -m "message"`
- Branch from master
- Rebase to current master before PR

### 6. Before Submitting PR
- [ ] All tests pass
- [ ] Code is formatted
- [ ] No clippy/analyzer warnings (or justified)
- [ ] Changes are documented if needed
- [ ] Rebased to current master
- [ ] Commits are signed off (DCO)

## Configuration Changes
If modifying configurations, ensure changes are in:
- `libs/hbb_common/src/config.rs` - Main config file
- Feature flags in `Cargo.toml` if adding new features

## Platform-Specific Considerations
- Windows: Test with virtual display drivers if applicable
- macOS: Consider signing and notarization requirements
- Linux: Test with different package formats if distribution changes
- Mobile: Test on actual devices, not just emulators
