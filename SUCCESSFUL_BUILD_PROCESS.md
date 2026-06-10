# ✅ Successful RustDesk Installer Build Process

This document records the **confirmed working process** for compiling the RustDesk installer with the Cislink icon.

---

## 📋 Prerequisites

- ✅ Inno Setup 6 installed at: `C:\Program Files (x86)\Inno Setup 6\`
- ✅ RustDesk executable: `D:\Rustdesk\rustdesk.exe`
- ✅ Cislink icon: `D:\Rustdesk\res\cislink.ico`
- ✅ Installer script: `D:\Rustdesk\RustDesk-Installer.iss`
- ✅ Administrator access to PowerShell

---

## ✅ Working Compilation Process

### Step 1: Open PowerShell as Administrator

**Method 1:**
- Press `Windows + X`
- Click **"Terminal (Admin)"** or **"PowerShell (Admin)"**

**Method 2:**
- Search for "PowerShell"
- Right-click → **"Run as administrator"**

### Step 2: Temporarily Disable Windows Defender Real-Time Protection

This is **required** to avoid Error 110 (Resource update error).

1. Open **Windows Security** (Windows + I → Privacy & Security → Windows Security)
2. Click **"Virus & threat protection"**
3. Click **"Manage settings"** under "Virus & threat protection settings"
4. Toggle **"Real-time protection"** to **OFF**
5. Click **Yes** on UAC prompt if asked

**Note:** Protection will automatically re-enable after ~15 minutes.

### Step 3: Navigate to RustDesk Directory

```powershell
cd D:\Rustdesk
```

### Step 4: Run Compilation Command

**Important:** Use the `&` operator and quotes around paths with spaces.

```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "RustDesk-Installer.iss"
```

**Breakdown:**
- `&` = PowerShell operator to execute commands
- First quotes = Path to Inno Setup compiler
- Second quotes = Path to installer script (relative or absolute)

**Alternative with full path:**
```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "D:\Rustdesk\RustDesk-Installer.iss"
```

### Step 5: Wait for Compilation

The process takes approximately **4-5 seconds**.

**Expected output:**
```
Inno Setup 6 Command-Line Compiler
Copyright (C) 1997-2025 Jordan Russell...
...
Successful compile (X.XXX sec). Resulting Setup program filename is:
D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe
```

### Step 6: Re-enable Windows Defender

1. Go back to **Windows Security**
2. Turn **"Real-time protection"** back **ON**

### Step 7: Verify the Installer

**Check file exists:**
```powershell
Get-ChildItem D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe
```

**Verify icon:**
- Right-click the installer file
- Select **Properties**
- You should see the Cislink icon in the file properties

**Check file hash:**
```powershell
Get-FileHash D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe -Algorithm SHA256
```

---

## 🎯 Expected Results

**Output Location:**
```
D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe
```

**File Size:** ~24 MB

**Features:**
- ✅ Custom Cislink icon on installer executable
- ✅ Latest RustDesk executable (v2.0)
- ✅ Pre-configured with Cislink server settings
  - ID Server: hbbs.cislink.nl
  - Relay Server: hbbr.cislink.nl
  - Public Key: VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=
- ✅ Automatic configuration file creation
- ✅ Optional desktop icon and autostart
- ✅ Clean uninstall with config removal option

---

## 🚫 Common Errors and Solutions

### Error: "Unexpected token 'RustDesk-Installer.iss'"

**Cause:** Missing `&` operator in PowerShell

**Wrong:**
```powershell
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" RustDesk-Installer.iss
```

**Correct:**
```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "RustDesk-Installer.iss"
```

### Error: "Resource update error: EndUpdateResource failed (110)"

**Cause:** Windows Defender is locking the icon/executable files

**Solution:** Disable Windows Defender Real-time protection (Step 2)

### Error: "另一个程序正在使用此文件" (File in use)

**Cause:** RustDesk is running or Windows Defender is scanning

**Solutions:**
1. Stop RustDesk: `taskkill /F /IM rustdesk.exe /T`
2. Disable Windows Defender Real-time protection
3. Wait 5 seconds and try again

---

## 📝 Installer Configuration Details

**File:** `D:\Rustdesk\RustDesk-Installer.iss`

**Key Settings:**
```ini
#define MyAppVersion "2.0"
SetupIconFile=res\cislink.ico
OutputBaseFilename=RustDesk_Cislink_Installer_v{#MyAppVersion}
```

**Server Configuration (Auto-deployed):**
```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
```

---

## 🔄 Quick Reference Commands

**Full build process (copy-paste ready):**
```powershell
# Navigate to directory
cd D:\Rustdesk

# Compile installer (after disabling Windows Defender)
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "RustDesk-Installer.iss"

# Verify output
Get-ChildItem Output\*.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1
```

**One-liner from any directory:**
```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "D:\Rustdesk\RustDesk-Installer.iss"
```

---

## 📅 Build History

**v2.0 - October 26, 2025**
- Updated to latest RustDesk executable
- Added Cislink custom icon
- Confirmed working build process documented

**v1.0 - October 12, 2025**
- Initial installer version
- Basic configuration without custom icon

---

## ✅ Success Criteria

The build is successful when:
1. ✅ No compilation errors
2. ✅ Output file exists at `D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe`
3. ✅ File size is approximately 24 MB
4. ✅ Cislink icon appears in file properties
5. ✅ Installer runs without errors
6. ✅ RustDesk automatically connects to Cislink servers after installation

---

**Last Updated:** October 26, 2025
**Tested On:** Windows 11 (Build 22631.4890)
**Inno Setup Version:** 6.5.4
