
Dopo l'avvio di devcontainer nel contenitore docker, viene creato un binario linux in modalità debug.

Attualmente devcontainer consente creazione build Linux e Android sia in modalità debug che in modalità rilascio.

Di seguito è riportata la tabella dei comandi da eseguire dalla root del progetto per la creazione di build specifiche.

Comando|Tipo build|Modo
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|debug
`.devcontainer/build.sh --release linux`|Linux|release
`.devcontainer/build.sh --debug android`|android-arm64|debug
`.devcontainer/build.sh --release android`|android-arm64|release

