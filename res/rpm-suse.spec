Name:       stardesk
Version:    1.1.9
Release:    0
Summary:    RPM package
License:    GPL-3.0
Requires:   gtk3 libxcb1 xdotool libXfixes3 alsa-utils libXtst6 libayatana-appindicator3-1 libvdpau1 libva2 pam gstreamer-plugins-base gstreamer-plugin-pipewire

%description
The best open-source remote desktop client software, written in Rust.

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

%global __python %{__python3}

%install
mkdir -p %{buildroot}/usr/bin/
mkdir -p %{buildroot}/usr/lib/stardesk/
mkdir -p %{buildroot}/usr/share/stardesk/files/
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps/
mkdir -p %{buildroot}/usr/share/icons/hicolor/scalable/apps/
install -m 755 $HBB/target/release/stardesk %{buildroot}/usr/bin/stardesk
install $HBB/libsciter-gtk.so %{buildroot}/usr/lib/stardesk/libsciter-gtk.so
install $HBB/res/stardesk.service %{buildroot}/usr/share/stardesk/files/
install $HBB/res/128x128@2x.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/stardesk.png
install $HBB/res/scalable.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/stardesk.svg
install $HBB/res/stardesk.desktop %{buildroot}/usr/share/stardesk/files/
install $HBB/res/stardesk-link.desktop %{buildroot}/usr/share/stardesk/files/

%files
/usr/bin/stardesk
/usr/lib/stardesk/libsciter-gtk.so
/usr/share/stardesk/files/stardesk.service
/usr/share/icons/hicolor/256x256/apps/stardesk.png
/usr/share/icons/hicolor/scalable/apps/stardesk.svg
/usr/share/stardesk/files/stardesk.desktop
/usr/share/stardesk/files/stardesk-link.desktop

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
    systemctl stop stardesk || true
  ;;
esac

%post
cp /usr/share/stardesk/files/stardesk.service /etc/systemd/system/stardesk.service
cp /usr/share/stardesk/files/stardesk.desktop /usr/share/applications/
cp /usr/share/stardesk/files/stardesk-link.desktop /usr/share/applications/
systemctl daemon-reload
systemctl enable stardesk
systemctl start stardesk
update-desktop-database

%preun
case "$1" in
  0)
    # for uninstall
    systemctl stop stardesk || true
    systemctl disable stardesk || true
    rm /etc/systemd/system/stardesk.service || true
  ;;
  1)
    # for upgrade
  ;;
esac

%postun
case "$1" in
  0)
    # for uninstall
    rm /usr/share/applications/stardesk.desktop || true
    rm /usr/share/applications/stardesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
  ;;
esac
