on run {daemon_file, root_agent_file, user_agent_file}

	set sh1 to "echo " & quoted form of daemon_file & " > /Library/LaunchDaemons/com.carriez.rustdesk.daemon.plist && chown root:wheel /Library/LaunchDaemons/com.carriez.rustdesk.daemon.plist;"

	set sh2 to "echo " & quoted form of root_agent_file & " > /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist && chown root:wheel /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist;"

	set sh3 to "echo " & quoted form of user_agent_file & " > /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist && chown root:wheel /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist;"

    set sh4 to "launchctl load -w /Library/LaunchDaemons/com.carriez.rustdesk.daemon.plist;"

    set sh5 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk.agent.root.plist;"

    set sh6 to "launchctl load -w /Library/LaunchAgents/com.carriez.rustdesk.agent.user.plist;"

	set sh to sh1 & sh2 & sh3 & sh4 & sh5 &sh6

	log (sh)
	do shell script sh with prompt "RustDesk 需要安装服务" with administrator privileges
end run