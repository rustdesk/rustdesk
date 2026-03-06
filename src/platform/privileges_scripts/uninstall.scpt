set sh1 to "launchctl unload -w /Library/LaunchDaemons/com.solucionesmarva.desk.service.plist;"
set sh2 to "/bin/rm /Library/LaunchDaemons/com.solucionesmarva.desk.service.plist;"
set sh3 to "/bin/rm /Library/LaunchAgents/com.solucionesmarva.desk.server.plist;"

set sh to sh1 & sh2 & sh3
do shell script sh with prompt "MarvaDesk wants to unload daemon" with administrator privileges