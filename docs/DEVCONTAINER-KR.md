
Docker 컨테이너에서 devcontainer가 시작된 후, 디버그 모드의 Linux 바이너리가 생성됩니다.

현재 devcontainer는 디버그 모드와 릴리스 모드 모두에서 Linux 및 Android 빌드를 제공합니다.

아래는 특정 빌드를 생성하기 위해 프로젝트 루트에서 실행하는 명령에 대한 표입니다.

명령|빌드 유형|모드
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|디버그
`.devcontainer/build.sh --release linux`|Linux|출시
`.devcontainer/build.sh --debug android`|android-arm64|디버그
`.devcontainer/build.sh --release android`|android-arm64|출시

