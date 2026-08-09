#!/bin/bash
set -e

cd /app/flutter/web/js
npm install
npm run build

cd /app/flutter/web
if [ ! -d "ogvjs" ]; then
    wget -q https://github.com/rustdesk/doc.rustdesk.com/releases/download/console/web_deps.tar.gz
    tar xzf web_deps.tar.gz
    rm web_deps.tar.gz
fi

cd /app/flutter
flutter pub get
flutter build web --profile --dart2js-optimization O1 --source-maps

echo "Build complete. Output in flutter/build/web/"
