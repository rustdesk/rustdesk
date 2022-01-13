set sh to "
launchctl enable system/com.carriez.rustdesk.daemon;
launchctl start system/com.carriez.rustdesk.daemon;
launchctl enable system/com.carriez.rustdesk.agent.root;
launchctl start system/com.carriez.rustdesk.agent.root;
launchctl enable system/com.carriez.rustdesk.agent.user
launchctl start system/com.carriez.rustdesk.agent.user
"

do shell script sh with prompt "RustDesk需要启动服务" with administrator privileges