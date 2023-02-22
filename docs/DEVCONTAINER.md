
After the start of devcontainer in docker container, a linux binary in debug mode is created.

Currently devcontainer offers linux and android builds in both debug and release mode.

Below is the table on commands to run from root of the project for creating specific builds.

Command|Build Type|Mode
-|-|-|
`.devcontainer/build.sh --debug linux`|Linux|debug
`.devcontainer/build.sh --release linux`|Linux|release
`.devcontainer/build.sh --debug android`|android-arm64|debug
`.devcontainer/build.sh --release android`|android-arm64|debug

