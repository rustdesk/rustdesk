on run {daemon_file, agent_file, user}

  set daemon_plist to "/Library/LaunchDaemons/com.carriez.RustDesk_service.plist"
  set agent_plist to "/Library/LaunchAgents/com.carriez.RustDesk_server.plist"
  set legacy_daemon_plist to "/Library/LaunchDaemons/com.carriez." & "Rust" & "Desk-Herbin_service.plist"
  set legacy_agent_plist to "/Library/LaunchAgents/com.carriez." & "Rust" & "Desk-Herbin_server.plist"

  set resolve_uid to "uid=$(id -u " & quoted form of user & " 2>/dev/null || true);"
  set unload_legacy_agent to "if [ -n \"$uid\" ]; then launchctl bootout gui/$uid " & quoted form of legacy_agent_plist & " 2>/dev/null || launchctl bootout user/$uid " & quoted form of legacy_agent_plist & " 2>/dev/null || launchctl unload -w " & quoted form of legacy_agent_plist & " || true; else launchctl unload -w " & quoted form of legacy_agent_plist & " || true; fi;"
  set unload_legacy_daemon to "launchctl bootout system " & quoted form of legacy_daemon_plist & " 2>/dev/null || launchctl unload -w " & quoted form of legacy_daemon_plist & " || true;"
  set remove_legacy_plists to "rm -f " & quoted form of legacy_daemon_plist & " " & quoted form of legacy_agent_plist & ";"

  set sh1 to "echo " & quoted form of daemon_file & " > " & daemon_plist & " && chown root:wheel " & daemon_plist & ";"

  set sh2 to "echo " & quoted form of agent_file & " > " & agent_plist & " && chown root:wheel " & agent_plist & ";"

  set sh3 to "cp -rf /Users/" & user & "/Library/Preferences/com.carriez.RustDesk/RustDesk.toml /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh4 to "cp -rf /Users/" & user & "/Library/Preferences/com.carriez.RustDesk/RustDesk2.toml /var/root/Library/Preferences/com.carriez.RustDesk/;"

  set sh5 to "launchctl load -w " & daemon_plist & ";"

  set agent_label_cmd to "agent_label=$(basename " & quoted form of agent_plist & " .plist);"
  set bootstrap_agent to "if [ -n \"$uid\" ]; then launchctl bootstrap gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootstrap user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl load -w " & quoted form of agent_plist & " || true; else launchctl load -w " & quoted form of agent_plist & " || true; fi;"
  set kickstart_agent to "if [ -n \"$uid\" ]; then launchctl kickstart -k gui/$uid/$agent_label 2>/dev/null || launchctl kickstart -k user/$uid/$agent_label 2>/dev/null || true; fi;"
  set load_agent to agent_label_cmd & bootstrap_agent & kickstart_agent

  set sh to resolve_uid & unload_legacy_agent & unload_legacy_daemon & remove_legacy_plists & sh1 & sh2 & sh3 & sh4 & sh5 & load_agent

  do shell script sh with prompt "RustDesk wants to install daemon and agent" with administrator privileges
end run
