on run {daemon_file, agent_file, user}
  set daemon_plist to "/Library/LaunchDaemons/com.carriez.RustDesk_service.plist"
  set agent_plist to "/Library/LaunchAgents/com.carriez.RustDesk_server.plist"

  set sh1 to "echo " & quoted form of daemon_file & " > " & daemon_plist & " && chown root:wheel " & daemon_plist & ";"

  set sh2 to "echo " & quoted form of agent_file & " > " & agent_plist & " && chown root:wheel " & agent_plist & ";"

  set sh3 to "user_preferences_dir=/Users/" & quoted form of user & "/Library/Preferences/com.carriez.RustDesk; root_preferences_dir=/var/root/Library/Preferences/com.carriez.RustDesk; mkdir -p \"$root_preferences_dir\";"
  set sh4 to "test ! -f \"$user_preferences_dir/RustDesk.toml\" || cp -rf \"$user_preferences_dir/RustDesk.toml\" \"$root_preferences_dir\";"
  set sh5 to "test ! -f \"$user_preferences_dir/RustDesk2.toml\" || cp -rf \"$user_preferences_dir/RustDesk2.toml\" \"$root_preferences_dir\";"

  set sh6 to "uid=$(id -u " & quoted form of user & " 2>/dev/null || true);"
  set sh7 to "launchctl load -w " & daemon_plist & ";"
  set sh8 to "agent_label=$(basename " & quoted form of agent_plist & " .plist);"
  set sh9 to "if [ -n \"$uid\" ]; then launchctl bootstrap gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootstrap user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl load -w " & quoted form of agent_plist & " || true; else launchctl load -w " & quoted form of agent_plist & " || true; fi;"
  set sh10 to "if [ -n \"$uid\" ]; then launchctl kickstart -k gui/$uid/$agent_label 2>/dev/null || launchctl kickstart -k user/$uid/$agent_label 2>/dev/null || true; fi;"

  set sh to "set -e;" & sh1 & sh2 & sh3 & sh4 & sh5 & sh6 & sh7 & sh8 & sh9 & sh10

  do shell script sh with prompt "RustDesk wants to install daemon and agent" with administrator privileges
end run
