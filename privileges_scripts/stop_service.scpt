set sh to "
launchctl disable system/com.carriez.rustdesk.daemon;
launchctl stop system/com.carriez.rustdesk.daemon;
launchctl disable system/com.carriez.rustdesk.agent.root;
launchctl stop system/com.carriez.rustdesk.agent.root;
launchctl disable system/com.carriez.rustdesk.agent.user
launchctl stop system/com.carriez.rustdesk.agent.user
"

do shell script sh with prompt "RustDesk需要停止服务" with administrator privileges