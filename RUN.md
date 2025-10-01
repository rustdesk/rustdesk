# RustDesk Stealth Mode - Setup and Usage Guide

This is a modified version of RustDesk with stealth mode features:
- No visible windows on startup
- Global hotkey (Ctrl+Shift+M) to toggle Connection Manager visibility
- Auto-accept connections from whitelisted peer IDs

---

## 📋 Prerequisites

### Linux Dependencies

Before building, install the required system dependencies:

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y libkeybinder-3.0-dev

# Fedora/RHEL
sudo dnf install keybinder3-devel

# Arch Linux
sudo pacman -S keybinder3
```

### Build Dependencies (if not already installed)

```bash
# Standard RustDesk dependencies
sudo apt-get install -y \
  zip g++ gcc git curl wget nasm yasm libgtk-3-dev clang \
  libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake make \
  libclang-dev ninja-build libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev libpam0g-dev
```

---

## 🔧 Building the Project

### 1. Install Flutter Dependencies

```bash
cd flutter
flutter pub get
cd ..
```

### 2. Build the Application

```bash
# Debug build (faster, for testing)
python3 build.py --flutter

# Release build (optimized, for production)
python3 build.py --flutter --release
```

**Build output location:**
- Debug: `flutter/build/linux/x64/debug/bundle/rustdesk`
- Release: `flutter/build/linux/x64/release/bundle/rustdesk`

---

## ⚙️ Configuration

### Set Up Whitelist (Optional)

To enable auto-accept for specific peer IDs, create or edit the config file:

```bash
# Create config directory if it doesn't exist
mkdir -p ~/.config/rustdesk

# Add whitelist to config file
echo 'whitelist_peer_ids = "YOUR_PEER_ID1,YOUR_PEER_ID2,YOUR_PEER_ID3"' >> ~/.config/rustdesk/RustDesk2.toml
```

**To find your peer ID:**
1. Run RustDesk normally (without --server)
2. Your ID is shown on the main screen
3. Share this ID with devices you want to whitelist

**Example config:**
```toml
# ~/.config/rustdesk/RustDesk2.toml
[options]
whitelist_peer_ids = "123456789,987654321,111222333"
```

---

## 🚀 Running the Application

### Method 1: Direct Server Mode (Recommended)

Run RustDesk in stealth server mode:

```bash
# From project root
./flutter/build/linux/x64/release/bundle/rustdesk --server

# Or if using debug build
./flutter/build/linux/x64/debug/bundle/rustdesk --server
```

**What happens:**
- No window appears on startup
- Service runs in background listening for connections
- When connection request arrives, Connection Manager (CM) starts hidden
- CM remains hidden unless you press the hotkey

---

## ⌨️ Keyboard Shortcuts

### Global Hotkey: Show/Hide Connection Manager

**Press:** `Ctrl + Shift + M`

- If CM is hidden → Shows CM window
- If CM is visible → Hides CM window

**Note:** This works system-wide, even when RustDesk is not focused.

---

## 📊 Features & Behavior

### 1. Stealth Startup ✅
- Running with `--server` flag keeps everything hidden
- No zombie windows or visible UI elements
- Perfect for unattended remote support scenarios

### 2. Connection Manager Toggle ✅
- Completely hidden by default
- Press `Ctrl+Shift+M` to show when you need to accept/reject connections
- Press again to hide
- Window state persists between connections

### 3. Auto-Accept Whitelist ✅
- Whitelisted peer IDs connect automatically
- No CM window shown for whitelisted connections
- Instant connection for trusted devices
- Logs show: `"Auto-accepting whitelisted peer: <ID>"`

### 4. Security Considerations ⚠️

**Important Security Notes:**
- Whitelisted IDs bypass connection approval
- Only add trusted devices to whitelist
- Regularly review and update whitelist
- Monitor logs for unexpected connections

---

## 🔍 Troubleshooting

### Issue: "hotkey_manager requires keybinder-3.0"

**Solution:**
```bash
sudo apt-get install -y libkeybinder-3.0-dev
```

Then rebuild:
```bash
python3 build.py --flutter --release
```

### Issue: Global hotkey not working

**Check:**
1. Verify hotkey registration in logs:
   ```
   flutter: Global hotkey Ctrl+Shift+M registered successfully
   ```

2. Ensure no other application uses `Ctrl+Shift+M`

3. Check permissions (some desktop environments may restrict global hotkeys)

### Issue: Whitelist not working

**Verify:**
1. Config file exists: `~/.config/rustdesk/RustDesk2.toml`
2. Peer IDs are correct (check logs for actual incoming IDs)
3. Format is correct: `whitelist_peer_ids = "id1,id2,id3"`
4. Restart RustDesk after config changes

### Issue: GTK warnings in logs

The GTK warnings are expected and harmless. They're suppressed by the fix, but may still appear in debug mode.

---

## 📝 Viewing Logs

### Real-time Logs

```bash
# Run with visible output
./flutter/build/linux/x64/release/bundle/rustdesk --server 2>&1 | tee rustdesk.log
```

### What to Look For

**Successful startup:**
```
flutter: launch args: [--cm]
flutter: --cm started
flutter: Global hotkey Ctrl+Shift+M registered successfully
```

**Whitelisted connection:**
```
flutter: Auto-accepting whitelisted peer: 123456789
```

**Hotkey pressed:**
```
flutter: Hotkey pressed: Showing CM window
flutter: Hotkey pressed: Hiding CM window
```

---

## 🔄 Running as a Service (Optional)

To run RustDesk as a system service:

### Create systemd service file:

```bash
sudo nano /etc/systemd/system/rustdesk-stealth.service
```

**Content:**
```ini
[Unit]
Description=RustDesk Stealth Server
After=network.target

[Service]
Type=simple
User=YOUR_USERNAME
Environment="DISPLAY=:0"
Environment="XAUTHORITY=/home/YOUR_USERNAME/.Xauthority"
ExecStart=/home/YOUR_USERNAME/rustdesk/flutter/build/linux/x64/release/bundle/rustdesk --server
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

**Replace:**
- `YOUR_USERNAME` with your actual username

**Enable and start:**
```bash
sudo systemctl daemon-reload
sudo systemctl enable rustdesk-stealth
sudo systemctl start rustdesk-stealth
```

**Check status:**
```bash
sudo systemctl status rustdesk-stealth
```

**View logs:**
```bash
sudo journalctl -u rustdesk-stealth -f
```

---

## 🎯 Usage Scenarios

### Scenario 1: Personal Remote Support
1. Run RustDesk with `--server` on family member's PC
2. Add your peer ID to their whitelist
3. Connect anytime without them needing to accept

### Scenario 2: Lab/Testing Environment
1. Deploy on test machines with `--server`
2. Whitelist admin peer IDs
3. Access machines remotely without visible UI

### Scenario 3: Monitoring Station
1. Run on monitoring PC in hidden mode
2. Press `Ctrl+Shift+M` when you need to check connections
3. Hide again for clean display

---

## 📦 File Modifications Summary

For reference, here are the files modified from original RustDesk:

1. **flutter/pubspec.yaml** - Added `hotkey_manager: ^0.2.3`
2. **flutter/lib/main.dart** - Added hotkey registration and stealth startup
3. **flutter/lib/models/server_model.dart** - Added whitelist auto-accept logic
4. **libs/hbb_common/src/config.rs** - Added whitelist config option

---

## 🆘 Getting Help

If you encounter issues:

1. Check this documentation thoroughly
2. Review logs for error messages
3. Verify all dependencies are installed
4. Ensure config file syntax is correct
5. Try rebuilding from scratch: `flutter clean && python3 build.py --flutter --release`

---

## 🎉 Success Indicators

You'll know everything is working when:

1. ✅ Build completes without errors
2. ✅ Running `--server` shows no windows
3. ✅ Logs show: "Global hotkey Ctrl+Shift+M registered successfully"
4. ✅ Pressing `Ctrl+Shift+M` toggles CM window
5. ✅ Whitelisted connections auto-accept
6. ✅ Non-whitelisted connections show CM for approval

---

**Enjoy your stealth RustDesk! 🚀**
