Name:       Digi-Desk2
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
mkdir -p %{buildroot}/usr/lib/Digi-Desk2/
mkdir -p %{buildroot}/usr/share/Digi-Desk2/files/
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps/
mkdir -p %{buildroot}/usr/share/icons/hicolor/scalable/apps/
install -m 755 $HBB/target/release/Digi-Desk2 %{buildroot}/usr/bin/Digi-Desk2
install $HBB/libsciter-gtk.so %{buildroot}/usr/lib/Digi-Desk2/libsciter-gtk.so
install $HBB/res/Digi-Desk2.service %{buildroot}/usr/share/Digi-Desk2/files/
install $HBB/res/128x128@2x.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/Digi-Desk2.png
install $HBB/res/scalable.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/Digi-Desk2.svg
install $HBB/res/Digi-Desk2.desktop %{buildroot}/usr/share/Digi-Desk2/files/
install $HBB/res/Digi-Desk2-link.desktop %{buildroot}/usr/share/Digi-Desk2/files/

%files
/usr/bin/Digi-Desk2
/usr/lib/Digi-Desk2/libsciter-gtk.so
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
    update-desktop-database
  ;;
  1)
    # for upgrade
  ;;
esac
