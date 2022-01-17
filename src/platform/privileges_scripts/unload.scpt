set sh1 to "launchctl unload -w /Library/LaunchDaemons/com.carriez.rustdesk_service.plist;"
set sh2 to "launchctl unload -w /Library/LaunchAgents/com.carriez.rustdesk_server.plist;"

do shell script sh1 with prompt "RustDesk want to unload daemon" with administrator privileges
do shell script sh2

