#!/bin/bash
set -e
case $1 in
    android)
    # install deps
    cd $WORKDIR/flutter
    flutter pub get
    wget https://github.com/rustdesk/doc.rustdesk.com/releases/download/console/so.tar.gz
    tar xzf so.tar.gz
    rm so.tar.gz
    sudo chown -R $(whoami) $ANDROID_HOME
    echo "Setup is Done."
    ;;
    linux)
    echo "Linux Setup"
    ;;
esac

    