set sh1 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist;"

set sh2 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist;"

do shell script sh1 with prompt "RustDesk 需要停止服务" with administrator privileges

do shell script sh2