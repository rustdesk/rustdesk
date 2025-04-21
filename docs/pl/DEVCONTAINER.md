
Po uruchomieniu devcontainer w kontenerze docker, tworzony jest plik binarny linux w trybue debugowania.

Obecnie devcontainer oferuje kompilowanie wersji dla linux i android w obu trybach - debugowania i wersji finalnej.

Poniżej tabela poleceń do uruchomienia z głównego folderu do tworzenia wybranych kompilacji.

Polecenie|Typ kompilacji|Tryb
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|debug
`.devcontainer/build.sh --release linux`|Linux|release
`.devcontainer/build.sh --debug android`|android-arm64|debug
`.devcontainer/build.sh --release android`|android-arm64|debug

