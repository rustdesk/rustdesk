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
    key)
    echo -e "\n$HOME/upload-keystore.jks is not created.\nLet's create it.\nRemember the password you enter in keytool!"
    keytool -genkey -v -keystore $HOME/upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload
    ;;
esac

    