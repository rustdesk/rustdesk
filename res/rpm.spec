Name:       marvadesk
Version:    1.4.2
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://marvadesk.com
Vendor:     MarvaDesk <info@marvadesk.com>
Requires:   gtk3 libxcb libxdo libXfixes alsa-lib libva2 pam gstreamer1-plugins-base
Recommends: libayatana-appindicator-gtk3

# https://docs.fedoraproject.org/en-US/packaging-guidelines/Scriptlets/

%description
MarvaDesk Remote Desktop - Soporte remoto.

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

%global __python %{__python3}

%install
mkdir -p %{buildroot}/usr/bin/
mkdir -p %{buildroot}/usr/share/marvadesk/
mkdir -p %{buildroot}/usr/share/marvadesk/files/
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps/
mkdir -p %{buildroot}/usr/share/icons/hicolor/scalable/apps/
install -m 755 $HBB/target/release/marvadesk %{buildroot}/usr/bin/marvadesk
install $HBB/libsciter-gtk.so %{buildroot}/usr/share/marvadesk/libsciter-gtk.so
install $HBB/res/marvadesk.service %{buildroot}/usr/share/marvadesk/files/
install $HBB/res/128x128@2x.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/marvadesk.png
install $HBB/res/scalable.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/marvadesk.svg
install $HBB/res/marvadesk.desktop %{buildroot}/usr/share/marvadesk/files/
install $HBB/res/marvadesk-link.desktop %{buildroot}/usr/share/marvadesk/files/

%files
/usr/bin/marvadesk
/usr/share/marvadesk/libsciter-gtk.so
/usr/share/marvadesk/files/marvadesk.service
/usr/share/icons/hicolor/256x256/apps/marvadesk.png
/usr/share/icons/hicolor/scalable/apps/marvadesk.svg
/usr/share/marvadesk/files/marvadesk.desktop
/usr/share/marvadesk/files/marvadesk-link.desktop
/usr/share/marvadesk/files/__pycache__/*

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
    rm /usr/share/applications/marvadesk.desktop || true
    rm /usr/share/applications/marvadesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
  ;;
esac
