
Nach dem Start von Dev-Container im Docker-Container wird ein Linux-Binärprogramm im Debug-Modus erstellt.

Derzeit bietet Dev-Container Linux- und Android-Builds sowohl im Debug- als auch im Release-Modus an.

Nachfolgend finden Sie eine Tabelle mit Befehlen, die im Stammverzeichnis des Projekts ausgeführt werden müssen, um bestimmte Builds zu erstellen.

Kommando|Build-Typ|Modus
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|debug
`.devcontainer/build.sh --release linux`|Linux|release
`.devcontainer/build.sh --debug android`|android-arm64|debug
`.devcontainer/build.sh --release android`|android-arm64|release

