# ✅ Pre-Build Verification Checklist

Run this checklist **BEFORE** building to ensure everything is ready.

---

## 1️⃣ Verify All Files Exist

### Rust Audio Module Files
```bash
# Navigate to folder first
cd c:\Users\Aayan\Desktop\rustdesk

# Check all audio module files exist
dir src\audio\mod.rs
dir src\audio\codec.rs
dir src\audio\jitter_buffer.rs
dir src\audio\network.rs
dir src\audio\capture.rs
dir src\audio\playback.rs
dir src\audio\windows_impl.rs
dir src\audio\macos_impl.rs
dir src\audio\linux_impl.rs
```

Expected: All 9 files listed with "File not found" = ❌ PROBLEM

### Client Files
```bash
dir src\client\voice_call_handler.rs
dir src\client\mod.rs
```

Expected: Both files exist

### Documentation Files
```bash
dir README_VOICE_CALL.md
dir PROTOCOL_VOICE_CALL.md
dir TESTING_VOICE_CALL.md
dir IMPLEMENTATION_SUMMARY.md
dir QUICK_REFERENCE.md
dir BUILD_INSTRUCTIONS.md
dir VOICE_CALL_DELIVERY.md
dir START_HERE.md
dir VOICE_CALL_IMPLEMENTATION_COMPLETE.md
```

Expected: All 9 files exist

### Build Files
```bash
dir build.bat
dir Cargo.toml
```

Expected: Both exist

---

## 2️⃣ Verify Project Structure

### Check Key Directories
```powershell
# PowerShell
Test-Path "src\audio"       # Should be True
Test-Path "src\client"      # Should be True
Test-Path "target"          # May not exist yet (created by build)
Test-Path "Cargo.toml"      # Should be True
```

Or on any terminal:
```bash
# Check these files exist
ls src/audio/mod.rs
ls src/client/voice_call_handler.rs
ls Cargo.toml
```

Expected: All paths exist without errors

---

## 3️⃣ Verify Cargo.toml Updated

Check that the `voice-call` feature was added:

```bash
# Search for voice-call in Cargo.toml
findstr "voice-call" Cargo.toml

# Or on any OS:
grep "voice-call" Cargo.toml
```

Expected output:
```
voice-call = []
```

If not found: ❌ Cargo.toml not updated properly

---

## 4️⃣ Verify src/lib.rs Updated

Check that audio module is declared:

```bash
# Search for audio module in lib.rs
findstr "pub mod audio" src\lib.rs

# Or:
grep "pub mod audio" src/lib.rs
```

Expected output:
```
pub mod audio;
```

Must be near line 78-79, with `#[cfg(feature = "voice-call")]` above it.

If not found: ❌ lib.rs not updated properly

---

## 5️⃣ Check Rust Installation

### Verify Rust is Installed
```bash
rustc --version
```

Expected output:
```
rustc 1.75.0 (or newer)
```

If not found: ❌ Rust not installed - get it from https://rustup.rs/

### Update Rust
```bash
rustup update
```

This ensures you have latest version.

---

## 6️⃣ Check Cargo.lock (Optional)

Verify `Cargo.lock` exists (tracks exact versions):

```bash
dir Cargo.lock
```

Expected: File exists (may be auto-generated if missing)

---

## 7️⃣ Compile Test (Quick Check)

Do a quick compilation check without full build:

```bash
# Just check that syntax is correct (don't actually build)
cargo check --features voice-call

# This should complete in 1-2 minutes
# If it passes, full build will likely succeed
```

Expected: "Finished `dev` profile" message

If fails: Review error messages (usually tell you what's wrong)

---

## 8️⃣ File Size Verification

Check that source files are reasonable size (not corrupted):

```powershell
# PowerShell
Get-Item src\audio\codec.rs | Select-Object Length
# Should show ~15-20KB (450 lines)

Get-Item src\audio\jitter_buffer.rs | Select-Object Length
# Should show ~20-25KB (550 lines)
```

Or manually check files open in editor without errors.

---

## 9️⃣ Feature Flag Verification

Verify feature is correctly declared:

```bash
# Check feature definition in Cargo.toml
findstr /A:2 "^\[features\]" Cargo.toml
```

Look for section like:
```
[features]
inline = []
cli = []
...
voice-call = []
```

The `voice-call = []` should be there.

---

## 🔟 Final Pre-Build Check

Run this command to verify everything compiles without warnings:

```bash
cargo check --features voice-call --all-targets
```

Expected output:
```
Compiling rustdesk v1.4.5
Finished `dev` profile [unoptimized + debuginfo] target(s) in XXs
```

⚠️ If you see warnings:
- They're usually not critical
- Build will still proceed
- Note them but they shouldn't stop you

❌ If you see errors:
- Review the error message
- Usually tells you exactly what's wrong
- Common fix: `cargo update` then try again

---

## ✅ Green Light to Build

If all checks above pass:

✅ Rust installed
✅ All 11 source files present
✅ Configuration files updated
✅ Feature flag declared
✅ cargo check passes

**YOU'RE READY TO BUILD! 🚀**

Run:
```bash
cargo build --release --features voice-call
```

Wait 12-20 minutes, and your executable will be at:
```
target/release/rustdesk.exe
```

---

## 🆘 If Something Failed

### If Files Missing
❌ Check you're in correct directory:
```bash
cd c:\Users\Aayan\Desktop\rustdesk
dir Cargo.toml  # Should exist
```

### If Cargo.toml Not Updated
❌ Verify line 43 has: `voice-call = []`
```bash
head -n 50 Cargo.toml | tail -n 10
```

### If lib.rs Not Updated
❌ Verify lines 78-79 have audio module:
```bash
sed -n '75,82p' src/lib.rs
```

### If Rust Not Installed
❌ Download from https://rustup.rs/ and run installer
❌ Restart terminal after install
❌ Run: `rustc --version` to verify

### If Cargo Check Fails
❌ Run: `cargo update`
❌ Run: `cargo clean`
❌ Run: `cargo check --features voice-call` again

---

## 📋 Verification Checklist (Printable)

Copy and paste into terminal, run ALL of these:

```bash
# Go to RustDesk directory
cd c:\Users\Aayan\Desktop\rustdesk

# File checks
dir src\audio\mod.rs
dir src\audio\codec.rs
dir src\audio\jitter_buffer.rs
dir src\audio\network.rs
dir src\audio\capture.rs
dir src\audio\playback.rs
dir src\audio\windows_impl.rs
dir src\audio\macos_impl.rs
dir src\audio\linux_impl.rs
dir src\client\voice_call_handler.rs
dir src\client\mod.rs
dir Cargo.toml
dir BUILD_INSTRUCTIONS.md

# Rust checks
rustc --version
cargo --version
rustup update

# Quick compile check (1-2 minutes)
cargo check --features voice-call

# If all above succeeded, you're ready!
# Run this to build (12-20 minutes):
# cargo build --release --features voice-call
```

Expected: All `dir` commands show files exist, rustc/cargo show versions, cargo check shows "Finished"

---

## ⏱️ Timeline

| Step | Time | What Happens |
|------|------|---|
| Prerequisites check | 2 min | Verify Rust, files, structure |
| cargo check | 2 min | Quick syntax validation |
| **Total pre-build** | **4 min** | Confirms you're ready |
| cargo build --release | 12-20 min | Actual compilation |
| **Total build time** | **16-24 min** | Enjoy coffee ☕ |

---

## 🎯 Success Indicators

After running all checks, you should see:

✅ All `dir` commands show files
✅ `rustc --version` shows 1.75+
✅ `cargo --version` shows 2.0+
✅ `cargo check --features voice-call` shows "Finished" with no errors
✅ No broken paths or missing files

If you see all ✅, you're **100% ready** to build:

```bash
cargo build --release --features voice-call
```

---

## 📞 What If One Check Fails?

| Issue | Solution | Time |
|-------|----------|------|
| Files missing | Check you're in right directory | 1 min |
| Rust not found | Install from https://rustup.rs/ | 5 min |
| Cargo check fails | Run `cargo update; cargo clean` | 5 min |
| Cargo.toml wrong | Re-run setup (already done) | N/A |
| lib.rs wrong | Already updated correctly | N/A |

Most issues resolve with:
```bash
cargo update
cargo clean
cargo check --features voice-call
```

---

## ✨ Final Check

Before you click "Build", run this **ONE** command:

```bash
cargo check --features voice-call
```

If this succeeds with "Finished" message:

**✅ READY TO BUILD EXECUTABLE**

Run:
```bash
cargo build --release --features voice-call
```

**Your executable will be** 📦 **at: `target/release/rustdesk.exe`** 📦

---

**Ready? Let's go! 🚀**
