Name:       kassatkadesk
Version:    1.5.0
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://rustdesk.com
Vendor:     rustdesk <info@rustdesk.com>
Requires:   gtk3 libxcb libXfixes alsa-lib libva gstreamer1-plugins-base
Recommends: libayatana-appindicator-gtk3 libxdo
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

mkdir -p "%{buildroot}/usr/share/kassatkadesk" && cp -r ${HBB}/flutter/build/linux/x64/release/bundle/* -t "%{buildroot}/usr/share/kassatkadesk"
mkdir -p "%{buildroot}/usr/bin"
install -Dm 644 $HBB/res/rustdesk.service "%{buildroot}/usr/share/kassatkadesk/files/kassatkadesk.service"
install -Dm 644 $HBB/res/rustdesk.desktop "%{buildroot}/usr/share/kassatkadesk/files/kassatkadesk.desktop"
install -Dm 644 $HBB/res/rustdesk-link.desktop "%{buildroot}/usr/share/kassatkadesk/files/kassatkadesk-link.desktop"
install -Dm 644 $HBB/res/128x128@2x.png "%{buildroot}/usr/share/icons/hicolor/256x256/apps/kassatkadesk.png"
install -Dm 644 $HBB/res/scalable.svg "%{buildroot}/usr/share/icons/hicolor/scalable/apps/kassatkadesk.svg"

%files
/usr/share/kassatkadesk/*
/usr/share/kassatkadesk/files/kassatkadesk.service
/usr/share/icons/hicolor/256x256/apps/kassatkadesk.png
/usr/share/icons/hicolor/scalable/apps/kassatkadesk.svg
/usr/share/kassatkadesk/files/kassatkadesk.desktop
/usr/share/kassatkadesk/files/kassatkadesk-link.desktop

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
    systemctl stop kassatkadesk || true
  ;;
esac

%post
cp /usr/share/kassatkadesk/files/kassatkadesk.service /etc/systemd/system/kassatkadesk.service
cp /usr/share/kassatkadesk/files/kassatkadesk.desktop /usr/share/applications/kassatkadesk.desktop
cp /usr/share/kassatkadesk/files/kassatkadesk-link.desktop /usr/share/applications/kassatkadesk-link.desktop
ln -sf /usr/share/kassatkadesk/rustdesk /usr/bin/kassatkadesk
systemctl daemon-reload
systemctl enable kassatkadesk
systemctl start kassatkadesk
update-desktop-database

%preun
case "$1" in
  0)
    # for uninstall
    systemctl stop kassatkadesk || true
    systemctl disable kassatkadesk || true
    rm /etc/systemd/system/kassatkadesk.service || true
  ;;
  1)
    # for upgrade
  ;;
esac

%postun
case "$1" in
  0)
    # for uninstall
    rm /usr/bin/kassatkadesk || true
    rmdir /usr/lib/rustdesk || true
    rmdir /usr/local/rustdesk || true
    rmdir /usr/share/kassatkadesk || true
    rm /usr/share/applications/kassatkadesk.desktop || true
    rm /usr/share/applications/kassatkadesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
    rmdir /usr/lib/rustdesk || true
    rmdir /usr/local/rustdesk || true
  ;;
esac
