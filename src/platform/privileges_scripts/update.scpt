on run {user, cur_pid, source_path, expected_sha256}

  set agent_plist to "/Library/LaunchAgents/com.carriez.RustDesk_server.plist"
  set daemon_plist to "/Library/LaunchDaemons/com.carriez.RustDesk_service.plist"
  set app_bundle to "/Applications/RustDesk.app"
  set installed_info_q to quoted form of (app_bundle & "/Contents/Info.plist")
  set service_executable to app_bundle & "/Contents/MacOS/service"
  set daemon_label to "com.carriez.RustDesk_service"
  set daemon_socket to "/tmp/RustDesk-service/ipc_service"
  set readiness_attempts to "30"
  set write_plist_attempts to "60"

  set source_path_q to quoted form of source_path
  set expected_sha256_q to quoted form of expected_sha256
  set daemon_plist_q to quoted form of daemon_plist
  set daemon_target_q to quoted form of ("system/" & daemon_label)
  set check_source to "if [ -n " & expected_sha256_q & " ]; then test -f " & source_path_q & "; else test -d " & source_path_q & "; fi;"
  -- Rehash the root-owned copy in a clean environment before staging bytes.
  set prepare_verified to "verified_dir=$(/usr/bin/mktemp -d /tmp/.rustdeskupdate-verified.XXXXXX); /bin/chmod 0700 \"$verified_dir\"; verified_app=\"$verified_dir/RustDesk.app\"; dmg_attached=0; if [ -n " & expected_sha256_q & " ]; then verified_dmg=\"$verified_dir/update.dmg\"; /bin/cp " & source_path_q & " \"$verified_dmg\"; /usr/sbin/chown root:wheel \"$verified_dmg\"; /bin/chmod 0400 \"$verified_dmg\"; actual_sha256=$(/usr/bin/env -i /usr/bin/shasum -a 256 \"$verified_dmg\"); actual_sha256=${actual_sha256%% *}; if [ \"$actual_sha256\" != " & expected_sha256_q & " ]; then echo 'Update DMG SHA256 mismatch' >&2; exit 1; fi; dmg_mount=\"$verified_dir/mount\"; /bin/mkdir \"$dmg_mount\"; dmg_attached=1; /usr/bin/hdiutil attach -readonly -nobrowse -mountpoint \"$dmg_mount\" \"$verified_dmg\" >/dev/null; /usr/bin/ditto \"$dmg_mount/RustDesk.app\" \"$verified_app\"; /usr/bin/hdiutil detach \"$dmg_mount\" -force >/dev/null; dmg_attached=0; /bin/rm -f \"$verified_dmg\"; else /usr/bin/ditto " & source_path_q & " \"$verified_app\"; fi; /usr/sbin/chown -R root:wheel \"$verified_app\"; /bin/chmod -R go-w \"$verified_app\";"
  set validate_verified_app to "installed_bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' " & installed_info_q & "); candidate_bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' \"$verified_app/Contents/Info.plist\"); if [ -z \"$installed_bundle_id\" ] || [ -z \"$candidate_bundle_id\" ] || [ \"$installed_bundle_id\" != \"$candidate_bundle_id\" ]; then echo 'Update app bundle identifier mismatch' >&2; exit 1; fi;"
  set resolve_uid to "uid=$(id -u " & quoted form of user & ");"
  set unload_agent to "if [ -n \"$uid\" ]; then launchctl bootout gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootout user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl unload -w " & quoted form of agent_plist & " || true; else launchctl unload -w " & quoted form of agent_plist & " || true; fi;"
  set unload_service to "launchctl unload -w " & daemon_plist_q & " || true;"
  set kill_others to "pids=$(pgrep -x 'RustDesk' | grep -vx " & cur_pid & " || true); if [ -n \"$pids\" ]; then echo \"$pids\" | xargs kill -9 || true; fi;"

  set prepare_swap_paths to "temp_bundle=" & quoted form of app_bundle & ".new.$$; old_bundle=" & quoted form of app_bundle & ".old.$$;"
  set cleanup_swap_paths to "rm -rf \"$temp_bundle\" \"$old_bundle\";"
  set backup_plists to "daemon_plist_backup=\"$verified_dir/daemon.plist\"; agent_plist_backup=\"$verified_dir/agent.plist\"; daemon_plist_existed=0; agent_plist_existed=0; if [ -e " & daemon_plist_q & " ]; then cp -p " & daemon_plist_q & " \"$daemon_plist_backup\"; daemon_plist_existed=1; fi; if [ -e " & quoted form of agent_plist & " ]; then cp -p " & quoted form of agent_plist & " \"$agent_plist_backup\"; agent_plist_existed=1; fi;"
  set stage_bundle to "ditto \"$verified_app\" \"$temp_bundle\";"
  set protect_staged_bundle to "chown -R root:wheel \"$temp_bundle\"; chmod -R go-w \"$temp_bundle\"; (xattr -r -d com.apple.quarantine \"$temp_bundle\" || true);"
  set move_current_bundle to "if [ -e " & quoted form of app_bundle & " ]; then mv " & quoted form of app_bundle & " \"$old_bundle\"; bundle_backed_up=1; fi;"
  set install_staged_bundle to "mv \"$temp_bundle\" " & quoted form of app_bundle & "; bundle_swapped=1;"
  set rollback_bundle to "if [ \"${bundle_backed_up:-0}\" -eq 1 ]; then if [ ! -e \"$old_bundle\" ]; then rollback_status=1; elif ! rm -rf " & quoted form of app_bundle & "; then rollback_status=1; elif ! mv \"$old_bundle\" " & quoted form of app_bundle & "; then rollback_status=1; fi; elif [ \"${bundle_swapped:-0}\" -eq 1 ]; then rm -rf " & quoted form of app_bundle & " || rollback_status=1; fi;"
  set rollback_plists to "if [ \"${daemon_plist_existed:-0}\" -eq 1 ]; then cp -p \"$daemon_plist_backup\" " & daemon_plist_q & " || rollback_status=1; else rm -f " & daemon_plist_q & " || rollback_status=1; fi; if [ \"${agent_plist_existed:-0}\" -eq 1 ]; then cp -p \"$agent_plist_backup\" " & quoted form of agent_plist & " || rollback_status=1; else rm -f " & quoted form of agent_plist & " || rollback_status=1; fi;"
  set cleanup_verified to "if [ \"${dmg_attached:-0}\" -eq 1 ]; then /usr/bin/hdiutil detach \"$dmg_mount\" -force >/dev/null 2>&1 || cleanup_status=1; fi; if [ -n \"${temp_bundle:-}\" ]; then rm -rf \"$temp_bundle\" || cleanup_status=1; fi; if [ -n \"${verified_dir:-}\" ]; then rm -rf \"$verified_dir\" || cleanup_status=1; fi;"

  -- Generate plist definitions from the installed release that will run them.
  set write_new_plists to "write_new_plists() { " & quoted form of service_executable & " --write-plists & write_pid=$!; for _ in $(/usr/bin/seq 1 " & write_plist_attempts & "); do if ! kill -0 \"$write_pid\" 2>/dev/null; then wait \"$write_pid\"; return $?; fi; sleep 1; done; kill -TERM \"$write_pid\" 2>/dev/null || true; sleep 1; kill -KILL \"$write_pid\" 2>/dev/null || true; wait \"$write_pid\" 2>/dev/null || true; return 124; }; write_new_plists;"
  set load_service to "launchctl load -w " & daemon_plist_q & ";"
  set agent_label_cmd to "agent_label=$(basename " & quoted form of agent_plist & " .plist);"
  -- Agent load failure must trigger rollback; restoration remains best effort.
  set bootstrap_agent to "if [ -n \"$uid\" ]; then launchctl bootstrap gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootstrap user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl load -w " & quoted form of agent_plist & "; else launchctl load -w " & quoted form of agent_plist & "; fi;"
  set kickstart_agent to "if [ -n \"$uid\" ]; then launchctl kickstart -k \"gui/$uid/$agent_label\" 2>/dev/null || launchctl kickstart -k \"user/$uid/$agent_label\" 2>/dev/null || true; fi;"
  set load_agent to agent_label_cmd & bootstrap_agent & kickstart_agent
  -- Registration can succeed before a job is ready; keep rollback until both IPC sockets are live.
  set check_service to "service_info=$(launchctl print " & daemon_target_q & " 2>/dev/null || true); printf '%s\n' \"$service_info\" | grep -E '^[[:space:]]*state = running[[:space:]]*$' >/dev/null && [ -S " & quoted form of daemon_socket & " ]"
  set check_agent to "agent_info=$(launchctl print \"gui/$uid/$agent_label\" 2>/dev/null || launchctl print \"user/$uid/$agent_label\" 2>/dev/null || launchctl print \"system/$agent_label\" 2>/dev/null || true); printf '%s\n' \"$agent_info\" | grep -E '^[[:space:]]*state = running[[:space:]]*$' >/dev/null && [ -S \"/tmp/RustDesk-$uid/ipc\" ]"
  set wait_for_service to "service_ready=0; for _ in $(/usr/bin/seq 1 " & readiness_attempts & "); do if " & check_service & "; then service_ready=1; break; fi; sleep 1; done; [ \"$service_ready\" -eq 1 ];"
  set wait_for_agent to "agent_ready=0; for _ in $(/usr/bin/seq 1 " & readiness_attempts & "); do if " & check_agent & "; then agent_ready=1; break; fi; sleep 1; done; [ \"$agent_ready\" -eq 1 ];"
  set verify_readiness to check_service & ";" & check_agent & ";"
  set restore_service to "launchctl load -w " & daemon_plist_q & " || rollback_status=1;"
  set restore_agent to "if [ -n \"$uid\" ]; then launchctl bootstrap gui/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl bootstrap user/$uid " & quoted form of agent_plist & " 2>/dev/null || launchctl load -w " & quoted form of agent_plist & " || rollback_status=1; else launchctl load -w " & quoted form of agent_plist & " || rollback_status=1; fi;"
  set rollback_update to "status=$?; trap - EXIT; set +e; cleanup_status=0; if [ \"${transaction_started:-0}\" -eq 1 ] && [ \"${transaction_committed:-0}\" -ne 1 ]; then rollback_status=0;" & unload_agent & unload_service & rollback_bundle & rollback_plists & restore_service & restore_agent & "if [ \"$rollback_status\" -ne 0 ]; then status=1; fi; fi; if [ \"${rollback_status:-0}\" -eq 0 ]; then " & cleanup_verified & "fi; if [ \"$cleanup_status\" -ne 0 ] && [ \"${transaction_committed:-0}\" -ne 1 ]; then status=1; elif [ \"$cleanup_status\" -ne 0 ]; then echo 'UPDATE_CLEANUP_FAILED_AFTER_COMMIT'; fi; exit \"$status\";"
  set commit_update to "transaction_committed=1; if ! rm -rf \"$old_bundle\"; then echo 'UPDATE_CLEANUP_FAILED_AFTER_COMMIT'; fi;"

  set sh to "set -e; transaction_started=0; transaction_committed=0; bundle_backed_up=0; bundle_swapped=0; trap " & quoted form of rollback_update & " EXIT;" & check_source & prepare_verified & validate_verified_app & resolve_uid & prepare_swap_paths & cleanup_swap_paths & backup_plists & stage_bundle & protect_staged_bundle & "transaction_started=1;" & unload_agent & unload_service & kill_others & move_current_bundle & install_staged_bundle & write_new_plists & load_service & wait_for_service & load_agent & wait_for_agent & verify_readiness & commit_update

  do shell script sh with prompt "RustDesk wants to update itself" with administrator privileges
end run
