Name:       techdesk
Version:    1.3.9
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://techdesk.com
Vendor:     techdesk <info@techdesk.com>
Requires:   gtk3 libxcb1 xdotool libXfixes3 alsa-utils libXtst6 libva2 pam gstreamer-plugins-base gstreamer-plugin-pipewire
Recommends: libayatana-appindicator3-1
Provides:   libdesktop_drop_plugin.so()(64bit), libdesktop_multi_window_plugin.so()(64bit), libfile_selector_linux_plugin.so()(64bit), libflutter_custom_cursor_plugin.so()(64bit), libflutter_linux_gtk.so()(64bit), libscreen_retriever_plugin.so()(64bit), libtray_manager_plugin.so()(64bit), liburl_launcher_linux_plugin.so()(64bit), libwindow_manager_plugin.so()(64bit), libwindow_size_plugin.so()(64bit), libtexture_rgba_renderer_plugin.so()(64bit)

# https://docs.fedoraproject.org/en-US/packaging-guidelines/Scriptlets/

%description
The best open-source remote desktop client software, written in Rust.

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

# %global __python %{__python3}

%install

mkdir -p "%{buildroot}/usr/share/techdesk" && cp -r ${HBB}/flutter/build/linux/x64/release/bundle/* -t "%{buildroot}/usr/share/techdesk"
mkdir -p "%{buildroot}/usr/bin"
install -Dm 644 $HBB/res/techdesk.service -t "%{buildroot}/usr/share/techdesk/files"
install -Dm 644 $HBB/res/techdesk.desktop -t "%{buildroot}/usr/share/techdesk/files"
install -Dm 644 $HBB/res/techdesk-link.desktop -t "%{buildroot}/usr/share/techdesk/files"
install -Dm 644 $HBB/res/128x128@2x.png "%{buildroot}/usr/share/icons/hicolor/256x256/apps/techdesk.png"
install -Dm 644 $HBB/res/scalable.svg "%{buildroot}/usr/share/icons/hicolor/scalable/apps/techdesk.svg"

%files
/usr/share/techdesk/*
/usr/share/techdesk/files/techdesk.service
/usr/share/icons/hicolor/256x256/apps/techdesk.png
/usr/share/icons/hicolor/scalable/apps/techdesk.svg
/usr/share/techdesk/files/techdesk.desktop
/usr/share/techdesk/files/techdesk-link.desktop

%changelog
# let's skip this for now

%pre
# can do something for centos7
case "$1" in
  1)
    # for install
  ;;
  2)
    # for upgrade
    systemctl stop techdesk || true
  ;;
esac

%post
cp /usr/share/techdesk/files/techdesk.service /etc/systemd/system/techdesk.service
cp /usr/share/techdesk/files/techdesk.desktop /usr/share/applications/
cp /usr/share/techdesk/files/techdesk-link.desktop /usr/share/applications/
ln -sf /usr/share/techdesk/techdesk /usr/bin/techdesk
systemctl daemon-reload
systemctl enable techdesk
systemctl start techdesk
update-desktop-database

%preun
case "$1" in
  0)
    # for uninstall
    systemctl stop techdesk || true
    systemctl disable techdesk || true
    rm /etc/systemd/system/techdesk.service || true
  ;;
  1)
    # for upgrade
  ;;
esac

%postun
case "$1" in
  0)
    # for uninstall
    rm /usr/bin/techdesk || true
    rmdir /usr/lib/techdesk || true
    rmdir /usr/local/techdesk || true
    rmdir /usr/share/techdesk || true
    rm /usr/share/applications/techdesk.desktop || true
    rm /usr/share/applications/techdesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
    rmdir /usr/lib/techdesk || true
    rmdir /usr/local/techdesk || true
  ;;
esac
