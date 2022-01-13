set current_dir to POSIX path of ((path to me as text) & "::")

set sh1 to "cp " & current_dir & "com.carriez.rustdesk.daemon.plist /Library/LaunchDaemons/com.carriez.rustdesk.daemon.plist && chown root:wheel /Library/LaunchDaemons/com.carriez.rustdesk.daemon.plist"
set sh2 to "cp " & current_dir & "com.carriez.rustdesk.agent.root.plist /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist && chown root:wheel /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist"
set sh3 to "cp " & current_dir & "com.carriez.rustdesk.agent.user.plist /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist && chown root:wheel /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist"

set sh to sh1 & ";" & sh2 & ";" & sh3 & "
launchctl enable system/com.carriez.rustdesk.daemon;
launchctl start system/com.carriez.rustdesk.daemon;
launchctl enable system/com.carriez.rustdesk.agent.root;
launchctl start system/com.carriez.rustdesk.agent.root;
launchctl enable system/com.carriez.rustdesk.agent.user
launchctl start system/com.carriez.rustdesk.agent.user
"

do shell script sh with prompt "RustDesk需要安装服务" with administrator privileges