Name:       Digi-Desk2
Version:    1.2.4
Release:    0
Summary:    RPM package
License:    GPL-3.0
Requires:   gtk3 libxcb1 xdotool libXfixes3 alsa-utils libXtst6 libappindicator-gtk3 libvdpau1 libva2 pam gstreamer-plugins-base gstreamer-plugin-pipewire
Provides:   libdesktop_drop_plugin.so()(64bit), libdesktop_multi_window_plugin.so()(64bit), libfile_selector_linux_plugin.so()(64bit), libflutter_custom_cursor_plugin.so()(64bit), libflutter_linux_gtk.so()(64bit), libscreen_retriever_plugin.so()(64bit), libtray_manager_plugin.so()(64bit), liburl_launcher_linux_plugin.so()(64bit), libwindow_manager_plugin.so()(64bit), libwindow_size_plugin.so()(64bit), libtexture_rgba_renderer_plugin.so()(64bit)

%description
The best open-source remote desktop client software, written in Rust.

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

# %global __python %{__python3}

%install

mkdir -p "%{buildroot}/usr/lib/Digi-Desk2" && cp -r ${HBB}/flutter/build/linux/x64/release/bundle/* -t "%{buildroot}/usr/lib/Digi-Desk2"
mkdir -p "%{buildroot}/usr/bin"
install -Dm 644 $HBB/res/Digi-Desk2.service -t "%{buildroot}/usr/share/Digi-Desk2/files"
install -Dm 644 $HBB/res/Digi-Desk2.desktop -t "%{buildroot}/usr/share/Digi-Desk2/files"
install -Dm 644 $HBB/res/Digi-Desk2-link.desktop -t "%{buildroot}/usr/share/Digi-Desk2/files"
install -Dm 644 $HBB/res/128x128@2x.png "%{buildroot}/usr/share/icons/hicolor/256x256/apps/Digi-Desk2.png"
install -Dm 644 $HBB/res/scalable.svg "%{buildroot}/usr/share/icons/hicolor/scalable/apps/Digi-Desk2.svg"

%files
/usr/lib/Digi-Desk2/*
/usr/share/Digi-Desk2/files/Digi-Desk2.service
/usr/share/icons/hicolor/256x256/apps/Digi-Desk2.png
/usr/share/icons/hicolor/scalable/apps/Digi-Desk2.svg
/usr/share/Digi-Desk2/files/Digi-Desk2.desktop
/usr/share/Digi-Desk2/files/Digi-Desk2-link.desktop

%changelog
# let's skip this for now

# https://www.cnblogs.com/xingmuxin/p/8990255.html
%pre
# can do something for centos7
case "$1" in
  1)
    # for install
  ;;
  2)
    # for upgrade
    systemctl stop Digi-Desk2 || true
  ;;
esac

%post
cp /usr/share/Digi-Desk2/files/Digi-Desk2.service /etc/systemd/system/Digi-Desk2.service
cp /usr/share/Digi-Desk2/files/Digi-Desk2.desktop /usr/share/applications/
cp /usr/share/Digi-Desk2/files/Digi-Desk2-link.desktop /usr/share/applications/
ln -s /usr/lib/Digi-Desk2/Digi-Desk2 /usr/bin/Digi-Desk2
systemctl daemon-reload
systemctl enable Digi-Desk2
systemctl start Digi-Desk2
update-desktop-database

%preun
case "$1" in
  0)
    # for uninstall
    systemctl stop Digi-Desk2 || true
    systemctl disable Digi-Desk2 || true
    rm /etc/systemd/system/Digi-Desk2.service || true
  ;;
  1)
    # for upgrade
  ;;
esac

%postun
case "$1" in
  0)
    # for uninstall
    rm /usr/share/applications/Digi-Desk2.desktop || true
    rm /usr/share/applications/Digi-Desk2-link.desktop || true
    rm /usr/bin/Digi-Desk2 || true
    update-desktop-database
  ;;
  1)
    # for upgrade
  ;;
esac
