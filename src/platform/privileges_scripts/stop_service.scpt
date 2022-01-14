set sh1 to "launchctl unload -w /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist;"

set sh2 to "launchctl unload -w /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist;"

do shell script sh1 with prompt "RustDesk want to stop services" with administrator privileges

do shell script sh2