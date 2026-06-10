# RustDesk Installer Build Instructions

Due to Windows Defender/Antivirus file locking issues during automated compilation, please follow these manual steps to build the installer.

## Updated Files
- ✅ **rustdesk.exe** (New version dated Oct 26, 23MB)
- ✅ **RustDesk-Installer.iss** (Version updated to 2.0)
- ✅ **res/icon.ico** (Present and ready)

## Method 1: Using PowerShell Script with Admin Rights (Recommended)

1. **Right-click on Windows Start Menu** → Select **PowerShell (Admin)** or **Terminal (Admin)**

2. **Navigate to the RustDesk directory:**
   ```powershell
   cd D:\Rustdesk
   ```

3. **Run the build script:**
   ```powershell
   .\build-installer.ps1
   ```

4. **The script will:**
   - Add a temporary Windows Defender exclusion
   - Compile the installer
   - Remove the exclusion
   - Show the output location

5. **Find your installer in:**
   ```
   D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe
   ```

## Method 2: Manual Compilation via Inno Setup GUI

1. **Open Inno Setup Compiler:**
   - Press `Windows Key` → Type "Inno Setup Compiler" → Press Enter
   - OR navigate to: `C:\Program Files (x86)\Inno Setup 6\Compil32.exe`

2. **Open the script file:**
   - In Inno Setup, click **File** → **Open**
   - Browse to: `D:\Rustdesk\RustDesk-Installer.iss`
   - Click **Open**

3. **Compile the installer:**
   - Click **Build** → **Compile** (or press `Ctrl+F9`)
   - Wait for compilation to complete

4. **Find the output:**
   - The installer will be created at:
   ```
   D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe
   ```

## Method 3: Temporary Antivirus Exclusion (If above methods fail)

1. **Open Windows Security:**
   - Press `Windows Key` → Type "Windows Security" → Press Enter

2. **Add an exclusion:**
   - Go to: **Virus & threat protection** → **Manage settings**
   - Scroll down to **Exclusions** → Click **Add or remove exclusions**
   - Click **Add an exclusion** → **Folder**
   - Select: `D:\Rustdesk`

3. **Compile using command line:**
   - Open PowerShell (regular, not admin needed now)
   ```powershell
   cd D:\Rustdesk
   & "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" RustDesk-Installer.iss
   ```

4. **Remove the exclusion after compilation:**
   - Go back to Windows Security exclusions
   - Remove `D:\Rustdesk` from the exclusion list

## Troubleshooting

### Error: "Another program is using this file"
- **Cause:** Windows Defender or antivirus is scanning the executable
- **Solution:** Use Method 1 or Method 3 above

### Error: "Resource update error: EndUpdateResource failed (110)"
- **Cause:** Icon file or executable is locked
- **Solution:**
  1. Stop all RustDesk processes: `taskkill /F /IM rustdesk.exe /T`
  2. Wait 5 seconds
  3. Try compilation again

### Error: Inno Setup not found
- **Check Installation:**
  ```powershell
  Test-Path "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
  ```
- **Download Inno Setup 6:** https://jrsoftware.org/isdl.php

## Installer Configuration

The installer is configured with:
- **Version:** 2.0
- **Server Settings:**
  - ID Server: `hbbs.cislink.nl`
  - Relay Server: `hbbr.cislink.nl`
  - Public Key: `VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`
- **Features:**
  - Automatic configuration deployment
  - Optional desktop icon
  - Optional autostart
  - Clean uninstallation with config removal option

## Output

Expected output file:
- **Name:** `RustDesk_Cislink_Installer_v2.0.exe`
- **Location:** `D:\Rustdesk\Output\`
- **Size:** ~11-13 MB (compressed from 23MB exe)

---

**Need Help?** Check the compilation log or run with verbose output to see detailed error messages.
