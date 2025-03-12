Name:       techdesk
Version:    1.1.9
Release:    0
Summary:    RPM package
License:    GPL-3.0
Requires:   gtk3 libxcb1 xdotool libXfixes3 alsa-utils libXtst6 libva2 pam gstreamer-plugins-base gstreamer-plugin-pipewire
Recommends: libayatana-appindicator3-1

# https://docs.fedoraproject.org/en-US/packaging-guidelines/Scriptlets/

%description
The best open-source remote desktop client software, written in Rust.

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

%global __python %{__python3}

%install
mkdir -p %{buildroot}/usr/bin/
mkdir -p %{buildroot}/usr/share/techdesk/
mkdir -p %{buildroot}/usr/share/techdesk/files/
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps/
mkdir -p %{buildroot}/usr/share/icons/hicolor/scalable/apps/
install -m 755 $HBB/target/release/techdesk %{buildroot}/usr/bin/techdesk
install $HBB/libsciter-gtk.so %{buildroot}/usr/share/techdesk/libsciter-gtk.so
install $HBB/res/techdesk.service %{buildroot}/usr/share/techdesk/files/
install $HBB/res/128x128@2x.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/techdesk.png
install $HBB/res/scalable.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/techdesk.svg
install $HBB/res/techdesk.desktop %{buildroot}/usr/share/techdesk/files/
install $HBB/res/techdesk-link.desktop %{buildroot}/usr/share/techdesk/files/

%files
/usr/bin/techdesk
/usr/share/techdesk/libsciter-gtk.so
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
    rm /usr/share/applications/techdesk.desktop || true
    rm /usr/share/applications/techdesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
  ;;
esac
