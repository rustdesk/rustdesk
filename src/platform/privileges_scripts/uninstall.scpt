set sh1 to "launchctl unload -w /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
set sh2 to "/bin/rm /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
set sh3 to "/bin/rm /Library/LaunchAgents/com.carriez.RustDesk_server.plist;"
set legacy_daemon_plist to "/Library/LaunchDaemons/com.carriez." & "Rust" & "Desk-Herbin_service.plist"
set legacy_agent_plist to "/Library/LaunchAgents/com.carriez." & "Rust" & "Desk-Herbin_server.plist"
set bad_daemon_plist to "/Library/LaunchDaemons/com.herbin." & "Rust" & "Desk-Herbin-Herbin_service.plist"
set bad_agent_plist to "/Library/LaunchAgents/com.herbin." & "Rust" & "Desk-Herbin-Herbin_server.plist"
set sh4 to "launchctl bootout system " & quoted form of legacy_daemon_plist & " 2>/dev/null || launchctl unload -w " & quoted form of legacy_daemon_plist & " || true;"
set sh5 to "/bin/rm -f " & quoted form of legacy_daemon_plist & " " & quoted form of legacy_agent_plist & ";"
set sh6 to "launchctl bootout system " & quoted form of bad_daemon_plist & " 2>/dev/null || launchctl unload -w " & quoted form of bad_daemon_plist & " || true;"
set sh7 to "/bin/rm -f " & quoted form of bad_daemon_plist & " " & quoted form of bad_agent_plist & ";"

set sh to sh1 & sh2 & sh3 & sh4 & sh5 & sh6 & sh7
do shell script sh with prompt "RustDesk wants to unload daemon" with administrator privileges
