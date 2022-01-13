set sh1 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist;"

set sh2 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist;"

set sh to sh1 & sh2

do shell script sh with prompt "RustDesk 需要停止服务" with administrator privileges