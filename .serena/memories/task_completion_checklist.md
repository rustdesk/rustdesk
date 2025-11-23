# Task Completion Checklist

When completing a task in RustDesk, follow these steps:

## Code Changes
1. Make minimal, focused changes to accomplish the task
2. Follow existing code style and patterns
3. Add appropriate comments if needed (match existing comment style)

## Testing
1. Run relevant tests based on the area changed:
   - **Rust changes**: `cargo test`
   - **Flutter changes**: `cd flutter && flutter test`
   - **Android changes**: Build Android APK to verify compilation

2. For Android/mobile changes:
   - Ensure the app compiles: `cd flutter && flutter build android --debug`
   - Test on a device or emulator if possible

3. For desktop changes:
   - Build the desktop app: `python3 build.py --flutter`

## Code Quality
1. **Flutter code**: Run `cd flutter && flutter analyze`
2. **Rust code**: Run `cargo clippy` (if available)
3. Ensure no new warnings are introduced

## Documentation
1. Update documentation if the changes affect user-facing features
2. Update README.md if setup/build instructions change
3. Add code comments for complex logic

## Final Verification
1. Verify all tests pass
2. Verify the build completes successfully
3. Check git status to ensure only intended files are changed
4. Review changes one final time before committing
