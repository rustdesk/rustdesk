# 🔧 RustDesk Voice Calling - Build Setup Fix

## Problem
Build failed because `link.exe` (Windows linker) is not found or Visual Studio build tools are missing.

## Solution

### Step 1: Install Visual Studio Build Tools

1. Download from: https://visualstudio.microsoft.com/downloads/

2. **Download "Build Tools for Visual Studio 2022"** (not Visual Studio Community)

3. Run the installer (VSBuildTools.exe)

4. When prompted, select **"Desktop development with C++"** workload

5. During installation, make sure these are selected:
   - [x] MSVC v143 - VS 2022 C++ x64/x86 build tools
   - [x] Windows 11 SDK 
   - [x] CMake tools for Windows

6. Complete the installation (may take 10-20 minutes)

### Step 2: Set Environment Variables

After installation, open **PowerShell as Administrator** and run:

```powershell
# Add Visual Studio to PATH
$vcvars = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
& $vcvars

# Verify Visual C++ compiler is available
cl.exe
# Should show: "Microsoft (R) C/C++ Optimizing Compiler"

# Verify linker is available
link.exe /?
# Should output linker help information
```

### Step 3: Start Fresh Build

```bash
cd c:\Users\Aayan\Desktop\rustdesk

# Clean all previous build artifacts
cargo clean

# Update dependencies
cargo update

# Build with voice-call feature
cargo build --release --features voice-call
```

**Expected time:** 20-40 minutes (longer on first build)

## Alternative: Install Pre-Built Build Tools

If you're having trouble with Visual Studio, you can install just the needed tools:

```bash
# Option A: LLVM (faster alternative linker)
cargo install llvm-tools-preview
rustup component add llvm-tools-preview

# Then try building with LLVM:
RUSTFLAGS="-C linker=lld-link.exe" cargo build --release --features voice-call
```

## Troubleshooting

### Error: "link.exe not found"
→ Visual Studio C++ tools not installed correctly
→ Run installer again, select "Desktop development with C++"

### Error: "cl.exe not found" 
→ Same as above - need Visual Studio build tools

### Error: "Cannot find MSVC"
→ Install Visual Studio Community (full version) instead of Build Tools

### Error: Long build takes >60 minutes
→ Normal for first build, incremental builds are faster
→ Computer may be slow - close other programs

---

## Full Setup Steps (From Scratch)

If everything is broken, follow this from the beginning:

### 1. Install Rust
```powershell
# Download from https://rustup.rs/
# Run installer
# Restart your terminal (close and reopen PowerShell)
```

### 2. Install Visual Studio Build Tools
- Go to: https://visualstudio.microsoft.com/downloads/
- Download: **Build Tools for Visual Studio 2022**
- Install with **"Desktop development with C++"** workload
- Restart computer

### 3. Verify Installation
```bash
rustc --version        # Should show 1.75+
cargo --version        # Should show 2.0+
cl.exe                 # Should show MSVC compiler
link.exe /?            # Should show linker help
```

If all show output (no "not found" errors), proceed to step 4.

### 4. Build RustDesk with Voice Calling
```bash
cd c:\Users\Aayan\Desktop\rustdesk
cargo clean
cargo update
cargo build --release --features voice-call
```

---

## What Happens During Build

The build downloads and compiles:
- Rust dependencies (crates)
- Native C/C++ libraries (libvpx, libyuv, opus, etc.)
- RustDesk core modules
- Voice calling features

Each step takes time:
- `Downloading crates` - 2-5 minutes
- `Compiling Rust` - 5-15 minutes  
- `Compiling C/C++` - 10-20 minutes
- **Total: 20-40 minutes** (varies by computer)

---

## Success Indicators

When build succeeds, you'll see:

```
   Compiling rustdesk v1.4.5
    Finished `release` profile [optimized] target(s) in XXm XXs
```

Your executable will be at:
```
target/release/rustdesk.exe
```

Size should be: **50-80MB**

---

## If Build Still Fails

### Check System Requirements

Verify your system meets minimum:
- Windows 10 or 11 (64-bit)
- Intel i5 or equivalent or better
- 8GB RAM minimum (16GB recommended)
- 20GB free disk space for build artifacts

### Try Minimal Build

```bash
# Test just the voice-call module without full app
cd libs/hbb_common
cargo test
```

If this works, the problem is with RustDesk dependencies, not voice calling.

### Get Help

If build still fails:
1. Copy the full error message
2. Run: `rustc --version` and `cargo --version`
3. Check: `cl.exe` and `link.exe` are available
4. Ensure: No antivirus blocking `link.exe`

---

## BUILD CHECKLIST

Before building, verify all of these:

- [ ] Windows 10/11 64-bit
- [ ] Visual Studio 2022 Build Tools installed
- [ ] `cl.exe` command works (C++ compiler)
- [ ] `link.exe` command works (linker)
- [ ] `rustc --version` shows 1.75+
- [ ] `cargo --version` shows 2.0+
- [ ] 20GB free disk space
- [ ] In correct directory: `dir Cargo.toml` shows file

Once all checked, run:
```bash
cargo build --release --features voice-call
```

---

## Expected Output

```
Compiling windows_x86_64_msvc v0.52.6
Compiling magnum v0.1.0
Compiling rustdesk v1.4.5 (<project dir>)
Finished `release` profile [optimized] target(s) in 24m 30s
```

Then your file is ready at:
```
target\release\rustdesk.exe (size: ~50-80MB)
```

---

**After these steps, your build should succeed. Good luck! 🚀**
