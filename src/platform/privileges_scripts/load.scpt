on run {user}

  set sh1 to "cp -rf /Users/" & user & "/Library/Preferences/com.carriez.RustDesk/RustDesk.toml /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh2 to "cp -rf /Users/" & user & "/Library/Preferences/com.carriez.RustDesk/RustDesk2.toml /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh3 to "launchctl load -w /Library/LaunchDaemons/com.carriez.rustdesk_service.plist;"

  set sh4 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk_server.plist;"

  set sh to sh1 & sh2 & sh3

  do shell script sh with prompt "RustDesk want to launch daemon" with administrator privileges
  do shell script sh4

end run
