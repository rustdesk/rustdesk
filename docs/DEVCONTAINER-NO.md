
Etter start av devcontainer i docker konteineren, blir en linux binærfil i debug modus laget.

Nå tilbyr devcontainer linux og android builds i både debug og release modus.

Under er tabellen over kommandoer som kan kjøres fra rot-direktive for kreasjon av spesefike builds.

Kommando|Build Type|Modus
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|debug
`.devcontainer/build.sh --release linux`|Linux|release
`.devcontainer/build.sh --debug android`|android-arm64|debug
`.devcontainer/build.sh --release android`|android-arm64|release

