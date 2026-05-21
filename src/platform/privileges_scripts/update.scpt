on run {daemon_file, agent_file, user, cur_pid, source_dir}

  set agent_plist to "/Library/LaunchAgents/com.carriez.RustDesk_server.plist"
  set daemon_plist to "/Library/LaunchDaemons/com.carriez.RustDesk_service.plist"
  set app_bundle to "/Applications/RustDesk.app"

  set check_source to "test -d " & quoted form of source_dir & " || exit 1;"
  set trusted_signer to "trusted_signer() { codesign --verify --deep --strict \"$1\"; };"
  set verify_source to "trusted_signer " & quoted form of source_dir & ";"
  set prepare_verified to "verified_dir=$(mktemp -d /tmp/.rustdeskupdate-verified.XXXXXX); verified_app=\"$verified_dir/RustDesk.app\"; ditto " & quoted form of source_dir & " \"$verified_app\" && chown -R root:wheel \"$verified_app\" && chmod -R go-w \"$verified_app\" && trusted_signer \"$verified_app\";"
  set resolve_uid to "uid=$(id -u " & quoted form of user & " 2>/dev/null || true);"
  set unload_agent to "if [ -n \"$uid\" ]; then launchctl bootout gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootout user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl unload -w " & quoted form of agent_plist & " || true; else launchctl unload -w " & quoted form of agent_plist & " || true; fi;"
  set unload_service to "launchctl unload -w " & daemon_plist & " || true;"
  set kill_others to "pids=$(pgrep -x 'RustDesk' | grep -vx " & cur_pid & " || true); if [ -n \"$pids\" ]; then echo \"$pids\" | xargs kill -9 || true; fi;"

  set prepare_swap_paths to "temp_bundle=" & quoted form of app_bundle & ".new.$$; old_bundle=" & quoted form of app_bundle & ".old.$$;"
  set cleanup_swap_paths to "rm -rf \"$temp_bundle\" \"$old_bundle\";"
  set stage_bundle to "ditto \"$verified_app\" \"$temp_bundle\";"
  set protect_staged_bundle to "chown -R root:wheel \"$temp_bundle\"; chmod -R go-w \"$temp_bundle\"; (xattr -r -d com.apple.quarantine \"$temp_bundle\" || true); trusted_signer \"$temp_bundle\";"
  set move_current_bundle to "if [ -e " & quoted form of app_bundle & " ]; then mv " & quoted form of app_bundle & " \"$old_bundle\"; fi;"
  set install_staged_bundle to "if mv \"$temp_bundle\" " & quoted form of app_bundle & "; then rm -rf \"$old_bundle\"; else if [ -e \"$old_bundle\" ]; then mv \"$old_bundle\" " & quoted form of app_bundle & "; fi; exit 1; fi;"
  set copy_files to prepare_swap_paths & cleanup_swap_paths & stage_bundle & protect_staged_bundle & move_current_bundle & install_staged_bundle
  set cleanup_verified to "if [ -n \"${temp_bundle:-}\" ]; then rm -rf \"$temp_bundle\"; fi; if [ -n \"${verified_dir:-}\" ]; then rm -rf \"$verified_dir\"; fi;"

  set write_daemon_plist to "echo " & quoted form of daemon_file & " > " & daemon_plist & " && chown root:wheel " & daemon_plist & ";"
  set write_agent_plist to "echo " & quoted form of agent_file & " > " & agent_plist & " && chown root:wheel " & agent_plist & ";"
  set load_service to "launchctl load -w " & daemon_plist & ";"
  set agent_label_cmd to "agent_label=$(basename " & quoted form of agent_plist & " .plist);"
  set bootstrap_agent to "if [ -n \"$uid\" ]; then launchctl bootstrap gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootstrap user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl load -w " & quoted form of agent_plist & " || true; else launchctl load -w " & quoted form of agent_plist & " || true; fi;"
  set kickstart_agent to "if [ -n \"$uid\" ]; then launchctl kickstart -k gui/$uid/$agent_label 2>/dev/null || launchctl kickstart -k user/$uid/$agent_label 2>/dev/null || true; fi;"
  set load_agent to agent_label_cmd & bootstrap_agent & kickstart_agent

  set sh to "set -e; trap " & quoted form of cleanup_verified & " EXIT;" & trusted_signer & check_source & verify_source & prepare_verified & resolve_uid & unload_agent & unload_service & kill_others & copy_files & write_daemon_plist & write_agent_plist & load_service & load_agent

  do shell script sh with prompt "RustDesk wants to update itself" with administrator privileges
end run
