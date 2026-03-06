Name:       marvadesk
Version:    1.4.2
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://marvadesk.com
Vendor:     MarvaDesk <info@marvadesk.com>
Requires:   gtk3 libxcb libxdo libXfixes alsa-lib libva pam gstreamer1-plugins-base
Recommends: libayatana-appindicator-gtk3
Provides:   libdesktop_drop_plugin.so()(64bit), libdesktop_multi_window_plugin.so()(64bit), libfile_selector_linux_plugin.so()(64bit), libflutter_custom_cursor_plugin.so()(64bit), libflutter_linux_gtk.so()(64bit), libscreen_retriever_plugin.so()(64bit), libtray_manager_plugin.so()(64bit), liburl_launcher_linux_plugin.so()(64bit), libwindow_manager_plugin.so()(64bit), libwindow_size_plugin.so()(64bit), libtexture_rgba_renderer_plugin.so()(64bit)

# https://docs.fedoraproject.org/en-US/packaging-guidelines/Scriptlets/

%description
MarvaDesk Remote Desktop - Soporte remoto.

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

# %global __python %{__python3}

%install

mkdir -p "%{buildroot}/usr/share/marvadesk" && cp -r ${HBB}/flutter/build/linux/x64/release/bundle/* -t "%{buildroot}/usr/share/marvadesk"
mkdir -p "%{buildroot}/usr/bin"
install -Dm 644 $HBB/res/marvadesk.service -t "%{buildroot}/usr/share/marvadesk/files"
install -Dm 644 $HBB/res/marvadesk.desktop -t "%{buildroot}/usr/share/marvadesk/files"
install -Dm 644 $HBB/res/marvadesk-link.desktop -t "%{buildroot}/usr/share/marvadesk/files"
install -Dm 644 $HBB/res/128x128@2x.png "%{buildroot}/usr/share/icons/hicolor/256x256/apps/marvadesk.png"
install -Dm 644 $HBB/res/scalable.svg "%{buildroot}/usr/share/icons/hicolor/scalable/apps/marvadesk.svg"

%files
/usr/share/marvadesk/*
/usr/share/marvadesk/files/marvadesk.service
/usr/share/icons/hicolor/256x256/apps/marvadesk.png
/usr/share/icons/hicolor/scalable/apps/marvadesk.svg
/usr/share/marvadesk/files/marvadesk.desktop
/usr/share/marvadesk/files/marvadesk-link.desktop

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
    systemctl stop marvadesk || true
  ;;
esac

%post
cp /usr/share/marvadesk/files/marvadesk.service /etc/systemd/system/marvadesk.service
cp /usr/share/marvadesk/files/marvadesk.desktop /usr/share/applications/
cp /usr/share/marvadesk/files/marvadesk-link.desktop /usr/share/applications/
ln -sf /usr/share/marvadesk/marvadesk /usr/bin/marvadesk
systemctl daemon-reload
systemctl enable marvadesk
systemctl start marvadesk
update-desktop-database

%preun
case "$1" in
  0)
    # for uninstall
    systemctl stop marvadesk || true
    systemctl disable marvadesk || true
    rm /etc/systemd/system/marvadesk.service || true
  ;;
  1)
    # for upgrade
  ;;
esac

%postun
case "$1" in
  0)
    # for uninstall
    rm /usr/bin/marvadesk || true
    rmdir /usr/lib/marvadesk || true
    rmdir /usr/local/marvadesk || true
    rmdir /usr/share/marvadesk || true
    rm /usr/share/applications/marvadesk.desktop || true
    rm /usr/share/applications/marvadesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
    rmdir /usr/lib/marvadesk || true
    rmdir /usr/local/marvadesk || true
  ;;
esac
