
Na de start van devcontainer in docker container wordt een linux binaire in foutmodus aangemaakt.

Momenteel biedt devcontainer linux en android builds in zowel foutopsporing- als uitgave modus.

Hieronder staat de tabel met commando's die vanuit de root van het project moeten worden 
uitgevoerd om specifieke builds te maken.

Commando|Build Type|Modus
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|debug
`.devcontainer/build.sh --release linux`|Linux|release
`.devcontainer/build.sh --debug android`|android-arm64|debug
`.devcontainer/build.sh --release android`|android-arm64|debug

