on run {daemon_file, agent_file, user}
  set daemon_plist to "/Library/LaunchDaemons/com.carriez.RustDesk_service.plist"
  set agent_plist to "/Library/LaunchAgents/com.carriez.RustDesk_server.plist"

  set sh1 to "echo " & quoted form of daemon_file & " > " & daemon_plist & " && chown root:wheel " & daemon_plist & ";"

  set sh2 to "echo " & quoted form of agent_file & " > " & agent_plist & " && chown root:wheel " & agent_plist & ";"

  set sh3 to "cp -rf /Users/" & user & "/Library/Preferences/com.carriez.RustDesk/RustDesk.toml /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh4 to "cp -rf /Users/" & user & "/Library/Preferences/com.carriez.RustDesk/RustDesk2.toml /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh5 to "uid=$(id -u " & quoted form of user & " 2>/dev/null || true);"
  set sh6 to "launchctl load -w " & daemon_plist & ";"
  set sh7 to "agent_label=$(basename " & quoted form of agent_plist & " .plist);"
  set sh8 to "if [ -n \"$uid\" ]; then launchctl bootstrap gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootstrap user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl load -w " & quoted form of agent_plist & " || true; else launchctl load -w " & quoted form of agent_plist & " || true; fi;"
  set sh9 to "if [ -n \"$uid\" ]; then launchctl kickstart -k gui/$uid/$agent_label 2>/dev/null || launchctl kickstart -k user/$uid/$agent_label 2>/dev/null || true; fi;"

  set sh to "set -e;" & sh1 & sh2 & sh3 & sh4 & sh5 & sh6 & sh7 & sh8 & sh9

  do shell script sh with prompt "RustDesk wants to install daemon and agent" with administrator privileges
end run
