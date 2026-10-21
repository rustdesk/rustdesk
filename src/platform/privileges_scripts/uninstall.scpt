-- Other service paths use load/unload; here a separate process revokes root grants.
-- unload can return zero on failure (see man launchctl), masking a failed stop.
-- Use bootout without an unload fallback so set -e aborts revocation on stop failure.
set sh1 to "if launchctl print system/com.carriez.RustDesk_service >/dev/null 2>&1; then launchctl bootout system/com.carriez.RustDesk_service; fi;"
set revoke_grants to "HOME=/var/root " & quoted form of "/Applications/RustDesk.app/Contents/MacOS/service" & " --clear-trusted-devices;"
set sh2 to "/bin/rm /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
set sh3 to "/bin/rm /Library/LaunchAgents/com.carriez.RustDesk_server.plist;"

set sh to "set -e;" & sh1 & revoke_grants & sh2 & sh3
do shell script sh with prompt "RustDesk wants to unload daemon" with administrator privileges