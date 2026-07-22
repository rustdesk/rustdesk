on run {daemon_file, agent_file, user}

  set prefs_dir to "/Users/" & user & "/Library/Preferences/com.carriez.RustDesk/"
  set prefs_toml to quoted form of (prefs_dir & "RustDesk.toml")
  set prefs2_toml to quoted form of (prefs_dir & "RustDesk2.toml")

  set sh1 to "echo " & quoted form of daemon_file & " > /Library/LaunchDaemons/com.carriez.RustDesk_service.plist && chown root:wheel /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"

  set sh2 to "echo " & quoted form of agent_file & " > /Library/LaunchAgents/com.carriez.RustDesk_server.plist && chown root:wheel /Library/LaunchAgents/com.carriez.RustDesk_server.plist;"

  set sh3 to "cp -rf " & prefs_toml & " /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh4 to "cp -rf " & prefs2_toml & " /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh5 to "launchctl bootout system/com.carriez.RustDesk_service 2>/dev/null || launchctl unload -w /Library/LaunchDaemons/com.carriez.RustDesk_service.plist 2>/dev/null || true; launchctl bootstrap system /Library/LaunchDaemons/com.carriez.RustDesk_service.plist 2>/dev/null || launchctl load -w /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"

  set sh to sh1 & sh2 & sh3 & sh4 & sh5

  do shell script sh with prompt "RustDesk wants to install daemon and agent" with administrator privileges
end run
