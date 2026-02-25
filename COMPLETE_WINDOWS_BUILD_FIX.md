# 🔧 Complete Visual Studio Setup & Build Fix for Windows

## Problem Summary

The error `link.exe failed` means the Windows C++ linker cannot be found. This is because Visual Studio Build Tools is either:
1. Not installed
2. Not properly configured
3. Not in the system PATH

## IMMEDIATE FIX (5 minutes)

### Option A: Launch from Visual Studio Command Prompt

Instead of using PowerShell, use **Visual Studio's Developer Command Prompt**:

1. Open Windows Start menu
2. Type: `Developer Command Prompt`
3. Click: **"Developer Command Prompt for VS 2022"**
4. Type: `cd C:\Users\Aayan\Desktop\rustdesk`
5. Type: `cargo build --release --features voice-call`

The Developer Command Prompt automatically sets up all paths to `link.exe` and other tools.

**This should work immediately!**

### Option B: Set Environment Variables (Advanced)

If Option A doesn't work, manually set the path:

```powershell
# Run as Administrator in PowerShell
$vsversion = "2022"
$vspath = "C:\Program Files\Microsoft Visual Studio\$vsversion\Community"
$env:PATH = "$vspath\VC\Tools\MSVC\14.39.33519\bin\Hostx64\x64;$env:PATH"
$env:lib = "$vspath\VC\Tools\MSVC\14.39.33519\lib\x64;$vspath\VC\Tools\MSVC\14.39.33519\lib\x86;$env:lib"

# Test that linker is available
link.exe /?

# Now build
cd C:\Users\Aayan\Desktop\rustdesk
cargo build --release --features voice-call
```

---

## COMPLETE SETUP (From Scratch - 30 minutes)

### Step 1: Uninstall Old Visual Studio (if present)

1. Open **Settings** → **Apps** → **Apps & features**
2. Search: `Visual Studio` or `Build Tools`
3. Click each one and select **Uninstall**
4. Restart computer

### Step 2: Install Visual Studio Build Tools

**Download:**
- Go to: https://visualstudio.microsoft.com/downloads/
- Scroll down to **"Build Tools for Visual Studio 2022"**
- Click **"Download"**
- Run the exe when download completes

**Install:**
1. When installer opens, click **"Continue"**
2. Wait for it to load (1-2 minutes)
3. Check ONLY these boxes:
   - ✅ **Desktop development with C++**
4. Do NOT check other workloads
5. Click **"Install"** (will take 5-10 minutes)
6. Check: **Visual C++ build tools**
7. Click **"Install"** if prompted
8. Wait for completion - **RESTART when prompted**

### Step 3: Verify Installation

After restart, open **PowerShell (Administrator)** and run:

```powershell
# Check Visual C++ compiler
cl.exe
# Output should be: "Microsoft (R) C/C++ Optimizing Compiler"

# Check linker
link.exe /?
# Output should show linker help

# Check Rust
rustc --version
# Output should be: "rustc 1.X.X"

cargo --version
# Output should be: "cargo 1.X.X"
```

**All four commands must show output (not "not found" error)**

### Step 4: Build RustDesk

```bash
cd C:\Users\Aayan\Desktop\rustdesk
cargo clean
cargo update
cargo build --release --features voice-call
```

Expected output when successful:
```
   Compiling rustdesk v1.4.5
    Finished `release` profile [optimized] target(s) in 25m 30s
```

Your executable: `target\release\rustdesk.exe` (50-80MB)

---

## CHECKLIST: Verify Installation

Run this to confirm everything is set up:

```powershell
# Run each line separately in PowerShell (Administrator)

# 1. Check Rust
rustc --version
cargo --version

# 2. Check Visual Studio Build Tools
cl.exe
link.exe /?

# 3. Check Cargo configuration
cargo config list

# 4. Quick build test
cd "C:\Users\Aayan\Desktop\rustdesk\voice-calling-standalone"
cargo build --release
```

If all commands work without errors, you're ready to build:

```bash
cd "C:\Users\Aayan\Desktop\rustdesk"
cargo build --release --features voice-call
```

---

## BUILD COMMAND REFERENCE

### Standard Build
```bash
cargo build --release --features voice-call
```
**Time:** 20-40 minutes (first build)  
**Output:** target/release/rustdesk.exe

### Build Without Features (Debug)
```bash
cargo build --features voice-call
```
**Time:** 5-15 minutes (faster, but slower exe)

### Clean Rebuild
```bash
cargo clean
cargo build --release --features voice-call
```
**Time:** 20-40 minutes (full rebuild)

### Just Update Dependencies
```bash
cargo update
cargo build --release --features voice-call
```

### Build with Verbose Output
```bash
cargo build --release --features voice-call --verbose 2>&1 | tee build.log
```

---

## TROUBLESHOOTING

### Issue: "link.exe not found"
**Cause:** Visual Studio Build Tools not in PATH  
**Fix:** Use Developer Command Prompt (see Option A above)

### Issue: "cl.exe not found"
**Cause:** MSVC compiler not installed  
**Fix:** Reinstall Build Tools, check "Desktop development with C++"

### Issue: "Build tools not found"
**Cause:** VS 2022 Build Tools not installed  
**Fix:** Download from https://visualstudio.microsoft.com/downloads/

### Issue: Build takes > 1 hour
**Cause:** Computer is slow or running other apps  
**Fix:** Close other programs, use release build (`--release`)

### Issue: "Linking failed with code 1127"
**Cause:** Corrupt installation or missing SDK  
**Fix:** Uninstall and reinstall Build Tools completely

### Issue: "No space on disk"
**Cause:** Not enough disk space for build  
**Fix:** Free up 20+ GB and clear `cargo clean`

---

## IF ALL ELSE FAILS

### Nuclear Option: Complete Reinstall

```bash
# 1. Uninstall everything
# - Control Panel → Programs → Uninstall Program
# - Remove: Visual Studio Build Tools
# - Remove: Rust (from Settings → Apps)
# Restart computer

# 2. Clear Rust cache
Remove-Item -Recurse -Force $env:USERPROFILE\.cargo
Remove-Item -Recurse -Force $env:USERPROFILE\.rustup

# 3. Install Rust fresh from https://rustup.rs/
# - Download installer
# - Run and accept defaults
# - Restart terminal

# 4. Install Build Tools fresh
# From https://visualstudio.microsoft.com/downloads/
# Download Build Tools for VS 2022
# Select "Desktop development with C++"
# Restart when done

# 5. Build
cd C:\Users\Aayan\Desktop\rustdesk
cargo clean
cargo build --release --features voice-call
```

---

## DEVELOPER COMMAND PROMPT QUICK START

This is the fastest way to build:

1. Open Start Menu
2. Type: `Developer Command Prompt for VS 2022`
3. Press Enter
4. Paste:
   ```
   cd /d C:\Users\Aayan\Desktop\rustdesk
   cargo build --release --features voice-call
   ```
5. Press Enter
6. Wait 20-40 minutes
7. Done! Your file: `target\release\rustdesk.exe`

---

## BUILD COMPLETE - What's Next?

When you see:
```
    Finished `release` profile [optimized] target(s) in XXm XXs
```

Your executable is ready:
```
C:\Users\Aayan\Desktop\rustdesk\target\release\rustdesk.exe
```

Size: 50-80MB  
Ready to use or distribute to others

---

## LAST RESORT SUPPORT

If nothing works, try the minimal test build:

```bash
cd "C:\Users\Aayan\Desktop\rustdesk\voice-calling-standalone"
cargo build --release
```

If this builds:
- ✅ Rust and voice-calling code is fine
- ⚠️ Problem is with full RustDesk dependencies

If this fails:
- ⚠️ Your system setup needs:
  1. Fresh Rust install from https://rustup.rs/
  2. Fresh Build Tools from https://visualstudio.microsoft.com/downloads/
  3. System restart
  4. Use Developer Command Prompt

---

**Try Option A (Developer Command Prompt) first - it works 95% of the time!**
