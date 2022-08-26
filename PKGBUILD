pkgname=rustdesk
pkgver=1.1.10
pkgrel=0
epoch=
pkgdesc=""
arch=('x86_64')
url=""
license=('GPL-3.0')
groups=()
depends=('gtk3' 'xdotool' 'libxcb' 'libxfixes' 'alsa-lib' 'pulseaudio' 'ttf-arphic-uming' 'python-pip' 'curl')
makedepends=()
checkdepends=()
optdepends=()
provides=()
conflicts=()
replaces=()
backup=()
options=()
install=pacman_install
changelog=
noextract=()
md5sums=() #generate with 'makepkg -g'

package() {
	install -Dm 755 ${HBB}/target/release/${pkgname} -t "${pkgdir}/usr/bin"
	install -Dm 644 ${HBB}/libsciter-gtk.so -t "${pkgdir}/usr/lib/rustdesk"
  install -Dm 644 $HBB/rustdesk.service -t "${pkgdir}/usr/share/rustdesk/files"
  install -Dm 644 $HBB/rustdesk.desktop -t "${pkgdir}/usr/share/rustdesk/files"
  install -Dm 644 $HBB/pynput_service.py -t "${pkgdir}/usr/share/rustdesk/files"
  install -Dm 644 $HBB/128x128@2x.png "${pkgdir}/usr/share/rustdesk/files/rustdesk.png"
}
