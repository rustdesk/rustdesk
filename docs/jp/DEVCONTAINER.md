
docker コンテナで devcontainer を起動すると、デバッグモードの linux バイナリが作成されます。

現在 devcontainer では、Linux と android のビルドをデバッグモードとリリースモードの両方で提供しています。

以下は、特定のビルドを作成するためにプロジェクトのルートから実行するコマンドの表になります。

コマンド|ビルド タイプ|モード
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|debug
`.devcontainer/build.sh --release linux`|Linux|release
`.devcontainer/build.sh --debug android`|android-arm64|debug
`.devcontainer/build.sh --release android`|android-arm64|release

