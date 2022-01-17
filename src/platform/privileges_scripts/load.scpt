set sh1 to "launchctl load -w /Library/LaunchDaemons/com.carriez.rustdesk_service.plist;"
set sh2 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk_server.plist;"

do shell script sh1 with prompt "RustDesk want to launch daemon" with administrator privileges
do shell script sh2

