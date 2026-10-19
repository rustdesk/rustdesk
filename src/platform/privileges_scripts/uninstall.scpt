set sh1 to "if launchctl print system/com.carriez.RustDesk_service >/dev/null 2>&1; then launchctl bootout system/com.carriez.RustDesk_service || launchctl unload -w /Library/LaunchDaemons/com.carriez.RustDesk_service.plist; fi;"
set revoke_grants to "HOME=/var/root " & quoted form of "/Applications/RustDesk.app/Contents/MacOS/service" & " --clear-trusted-devices;"
set sh2 to "/bin/rm /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
set sh3 to "/bin/rm /Library/LaunchAgents/com.carriez.RustDesk_server.plist;"

set sh to "set -e;" & sh1 & revoke_grants & sh2 & sh3
do shell script sh with prompt "RustDesk wants to unload daemon" with administrator privileges