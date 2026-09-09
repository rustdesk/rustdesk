Name:       kassatkadesk
Version:    1.5.0
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://rustdesk.com
Vendor:     rustdesk <info@rustdesk.com>
Requires:   gtk3 libxcb libXfixes alsa-lib libva2 gstreamer1-plugins-base
Recommends: libayatana-appindicator-gtk3 libxdo

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
mkdir -p %{buildroot}/usr/share/kassatkadesk/
mkdir -p %{buildroot}/usr/share/kassatkadesk/files/
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps/
mkdir -p %{buildroot}/usr/share/icons/hicolor/scalable/apps/
install -m 755 $HBB/target/release/rustdesk %{buildroot}/usr/bin/kassatkadesk
install $HBB/libsciter-gtk.so %{buildroot}/usr/share/kassatkadesk/libsciter-gtk.so
install $HBB/res/rustdesk.service %{buildroot}/usr/share/kassatkadesk/files/kassatkadesk.service
install $HBB/res/128x128@2x.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/kassatkadesk.png
install $HBB/res/scalable.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/kassatkadesk.svg
install $HBB/res/rustdesk.desktop %{buildroot}/usr/share/kassatkadesk/files/kassatkadesk.desktop
install $HBB/res/rustdesk-link.desktop %{buildroot}/usr/share/kassatkadesk/files/kassatkadesk-link.desktop

%files
/usr/bin/kassatkadesk
/usr/share/kassatkadesk/libsciter-gtk.so
/usr/share/kassatkadesk/files/kassatkadesk.service
/usr/share/icons/hicolor/256x256/apps/kassatkadesk.png
/usr/share/icons/hicolor/scalable/apps/kassatkadesk.svg
/usr/share/kassatkadesk/files/kassatkadesk.desktop
/usr/share/kassatkadesk/files/kassatkadesk-link.desktop
/usr/share/kassatkadesk/files/__pycache__/*

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
    rm /usr/share/applications/kassatkadesk.desktop || true
    rm /usr/share/applications/kassatkadesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
  ;;
esac
