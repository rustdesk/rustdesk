on run {user}
  set service_exe to quoted form of "/Applications/RustDesk.app/Contents/MacOS/service"
  set sh1 to "launchctl unload -w /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
  -- The helper verifies the daemon has stopped before transferring its current grants.
  set handoff to "HOME=/var/root " & service_exe & " --trusted-device-handoff user " & quoted form of user & " || exit 1;"
  set sh2 to "/bin/rm /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
  set sh3 to "/bin/rm /Library/LaunchAgents/com.carriez.RustDesk_server.plist;"

  set sh to sh1 & handoff & sh2 & sh3
  do shell script sh with prompt "RustDesk wants to unload daemon" with administrator privileges
end run
