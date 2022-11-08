Name:       rustdesk 
Version:    1.2.0
Release:    0
Summary:    RPM package
License:    GPL-3.0
Requires:   gtk3 libxcb libxdo libXfixes pipewire alsa-lib curl libayatana-appindicator3-1 libvdpau1 libva2


%description
The best open-source remote desktop client software, written in Rust. 

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

# %global __python %{__python3}

%install

mkdir -p "${buildroot}/usr/lib/rustdesk" && cp -r ${HBB}/flutter/build/linux/x64/release/bundle/* -t "${buildroot}/usr/lib/rustdesk"
mkdir -p "${buildroot}/usr/bin"
pushd ${buildroot} && ln -s /usr/lib/rustdesk/rustdesk usr/bin/rustdesk && popd
install -Dm 644 $HBB/res/rustdesk.service -t "${buildroot}/usr/share/rustdesk/files"
install -Dm 644 $HBB/res/rustdesk.desktop -t "${buildroot}/usr/share/rustdesk/files"
install -Dm 644 $HBB/res/rustdesk-link.desktop -t "${buildroot}/usr/share/rustdesk/files"
install -Dm 644 $HBB/res/128x128@2x.png "${buildroot}/usr/share/rustdesk/files/rustdesk.png"


%files
/usr/bin/rustdesk
/usr/lib/rustdesk/*
/usr/share/rustdesk/files/rustdesk.service
/usr/share/rustdesk/files/rustdesk.png
/usr/share/rustdesk/files/rustdesk.desktop
/usr/share/rustdesk/files/rustdesk-link.desktop

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
    systemctl stop rustdesk || true
  ;;
esac

%post
cp /usr/share/rustdesk/files/rustdesk.service /etc/systemd/system/rustdesk.service
cp /usr/share/rustdesk/files/rustdesk.desktop /usr/share/applications/
cp /usr/share/rustdesk/files/rustdesk-link.desktop /usr/share/applications/
systemctl daemon-reload
systemctl enable rustdesk
systemctl start rustdesk
update-desktop-database

%preun
case "$1" in
  0)
    # for uninstall
    systemctl stop rustdesk || true
    systemctl disable rustdesk || true
    rm /etc/systemd/system/rustdesk.service || true
  ;;
  1)
    # for upgrade
  ;;
esac

%postun
case "$1" in
  0)
    # for uninstall
    rm /usr/share/applications/rustdesk.desktop || true
    rm /usr/share/applications/rustdesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
  ;;
esac
