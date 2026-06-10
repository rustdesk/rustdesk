# How to Add Cislink Icon to the Installer

Your RustDesk installer v2.0 has been successfully compiled, but **without the custom Cislink icon** due to Windows Defender resource locking (error 110).

The icon can be easily added by temporarily disabling Windows Defender and recompiling. Here's how:

---

## ✅ Current Status

- **Installer File:** `D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe`
- **Size:** 24 MB
- **SHA256:** `3123AD3A561BA6CB6236DC03E58233CDDE8B8D7CFAA28A58F50C81EE9FBD54FE`
- **Icon Status:** ❌ Using default Inno Setup icon (not Cislink icon)
- **Cislink Icon:** ✅ Ready at `D:\Rustdesk\res\cislink.ico` (3.6 KB)

---

## 🔧 Method 1: Quick Recompile with Icon (Recommended)

### Step 1: Temporarily Disable Windows Defender

1. **Open Windows Security:**
   - Press `Windows + I` (Settings)
   - Go to: **Privacy & Security** → **Windows Security**
   - Click: **Virus & threat protection**

2. **Turn OFF Real-time protection:**
   - Click **Manage settings** under "Virus & threat protection settings"
   - Toggle **Real-time protection** to **OFF**
   - Click **Yes** on the UAC prompt

### Step 2: Compile with Icon

Open PowerShell in `D:\Rustdesk` and run:

```powershell
cd D:\Rustdesk\BuildTemp
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" RustDesk-Installer-Build.iss
```

**Before compiling,** edit `D:\Rustdesk\BuildTemp\RustDesk-Installer-Build.iss` line 40:

**Change from:**
```ini
; SetupIconFile=cislink.ico
```

**To:**
```ini
SetupIconFile=cislink.ico
```

### Step 3: Re-enable Windows Defender

After compilation completes:
1. Go back to Windows Security
2. Turn **Real-time protection** back **ON**

### Step 4: Verify

The new installer will be at:
```
D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.0.exe
```

Right-click the file → Properties → you should see the Cislink icon!

---

## 🔧 Method 2: Using Windows Defender Exclusion

This method is safer as it doesn't fully disable protection:

### Step 1: Add Folder Exclusion

1. **Windows Security** → **Virus & threat protection** → **Manage settings**
2. Scroll to **Exclusions** → Click **Add or remove exclusions**
3. Click **Add an exclusion** → **Folder**
4. Select: `D:\Rustdesk`
5. Click **Select Folder**

### Step 2: Enable Icon in Script

Edit `D:\Rustdesk\BuildTemp\RustDesk-Installer-Build.iss` line 40:

```ini
SetupIconFile=cislink.ico  # Uncomment this line
```

### Step 3: Compile

```powershell
cd D:\Rustdesk\BuildTemp
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" RustDesk-Installer-Build.iss
```

### Step 4: Remove Exclusion

1. Go back to **Windows Security** → **Exclusions**
2. Find `D:\Rustdesk` in the list
3. Click the three dots **⋯** → **Remove**

---

## 🔧 Method 3: Use Resource Hacker (Add Icon After Compilation)

If you don't want to disable Windows Defender, you can add the icon **after** compilation:

### Step 1: Download Resource Hacker

Download from: http://www.angusj.com/resourcehacker/

### Step 2: Open Installer in Resource Hacker

1. Open Resource Hacker
2. **File** → **Open** → Select `RustDesk_Cislink_Installer_v2.0.exe`

### Step 3: Replace Icon

1. In the left panel, expand **Icon** → **MAINICON**
2. **Action** → **Replace Icon**
3. Click **Open file with new icon** → Browse to `D:\Rustdesk\res\cislink.ico`
4. Click **Replace**
5. **File** → **Save**

---

## 📋 Quick Command Reference

### Check if icon is in correct location:
```powershell
Test-Path "D:\Rustdesk\res\cislink.ico"
Test-Path "D:\Rustdesk\BuildTemp\cislink.ico"
```

### Compile from BuildTemp (with icon enabled):
```powershell
cd D:\Rustdesk\BuildTemp
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" RustDesk-Installer-Build.iss
```

### Verify installer was created:
```powershell
Get-ChildItem D:\Rustdesk\Output\*.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1
```

---

## ❓ Troubleshooting

### Error 110 - Resource update failed
- **Cause:** Windows Defender is scanning/locking the icon file
- **Solution:** Use Method 1 or Method 2 above

### Icon doesn't appear after compilation
- **Check:** Make sure line 40 in the `.iss` file is uncommented
- **Check:** Icon file exists at `D:\Rustdesk\BuildTemp\cislink.ico`
- **Try:** Method 3 (Resource Hacker) as a workaround

### Can't disable Windows Defender
- **Try:** Method 2 (Exclusion) instead
- **Or:** Use Method 3 (Resource Hacker) to add icon post-compilation

---

## ✅ Verification

After adding the icon, verify it worked:

1. **Right-click** the installer file
2. **Properties** → You should see the Cislink icon
3. **Run the installer** - the setup wizard should show the Cislink icon

---

## 📝 Notes

- The **installed RustDesk application** will have its proper icon regardless
- This only affects the **installer executable** appearance
- Windows Defender Real-time protection automatically re-enables after ~15 minutes
- The current installer (without custom icon) is fully functional - the icon is purely cosmetic

---

**Need help?** The installer works perfectly without the icon - this is only for branding/appearance!
