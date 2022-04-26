set sh1 to "launchctl unload -w /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
set sh2 to "/bin/rm /Library/LaunchDaemons/com.carriez.RustDesk_service.plist;"
set sh3 to "/bin/rm /Library/LaunchAgents/com.carriez.RustDesk_server.plist;"

set sh to sh1 & sh2 & sh3
do shell script sh with prompt "RustDesk want to unload daemon" with administrator privileges

set sh5 to "[ ! -f /Library/LaunchAgents/com.carriez.RustDesk_server.plist ] && launchctl remove com.carriez.RustDesk_server && sleep 1 && open /Applications/RustDesk.app"
do shell script sh5
