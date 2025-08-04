#!/bin/bash

# RustDesk - Custom resources script for Linux only

app_name0=$1
app_name=$2

# Update flatpak configuration
sed -i 's/..\/res\/scalable.svg/..\/res\/128x128@2x.png/g' flatpak/rustdesk.json
sed -i 's/*.desktop",/*.desktop","install -Dm644 128x128@2x.png \/app\/share\/icons\/hicolor\/256x256\/apps\/com.rustdesk.RustDesk.png"/g' flatpak/rustdesk.json

# Process resource directories
for p in res appimage flatpak; do
    if [ -d "$p" ]; then
        find $p -type f -exec sed -i '/rustdesk.svg/d' {} \;
        find $p -type f -exec sed -i '/scalable.svg/d' {} \;
        find $p -type f -exec sed -i "s/RustDesk/${app_name0}/g" {} \;
        find $p -type f -exec grep -l "rustdesk" {} \; | xargs -I{} sh -c 'sed -i "/com\.rustdesk/!s/rustdesk/'${app_name}'/g" {}'
    else
        echo "Directory $p does not exist, skipping"
    fi
done

mv res/rustdesk.service res/${app_name}.service
mv res/rustdesk.desktop res/${app_name}.desktop
mv res/rustdesk-link.desktop res/${app_name}-link.desktop
mv flatpak/com.rustdesk.RustDesk.metainfo.xml flatpak/com.rustdesk.${app_name0}.metainfo.xml
