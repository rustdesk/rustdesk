#!/usr/bin/env python3

import os
import platform
import zlib
from shutil import copy2
import hashlib
import getpass

windows = platform.platform().startswith('Windows')
osx = platform.platform().startswith('Darwin') or platform.platform().startswith("macOS")
hbb_name = 'rustdesk' + ('.exe' if windows else '')
exe_path = 'target/release/' + hbb_name


def get_version():
    with open("Cargo.toml") as fh:
        for line in fh:
            if line.startswith("version"):
                return line.replace("version", "").replace("=", "").replace('"', '').strip()
    return ''


def main():
    os.system("cp Cargo.toml Cargo.toml.bk")
    os.system("cp src/main.rs src/main.rs.bk")
    if windows:
        txt = open('src/main.rs').read()
        with open('src/main.rs', 'wt') as fh:
            fh.write(txt.replace(
                '//#![windows_subsystem', '#![windows_subsystem'))
    if os.path.exists(exe_path):
        os.unlink(exe_path)
    os.system('python3 inline-sciter.py')
    txt = open('Cargo.toml').read()
    with open('Cargo.toml', 'wt') as fh:
        fh.write(txt.replace('#lto', 'lto')
                 .replace('#codegen', 'codegen')
                 .replace('#panic', 'panic')
                 .replace('[features]',
                          '[features]\ndefault=["inline"]'))
    version = get_version()
    if windows:
        os.system('cargo build --release')
        os.system('upx.exe target/release/rustdesk.exe')
        os.system('mv target/release/rustdesk.exe target/release/RustDesk.exe')
        pa = getpass.getpass()
        os.system('signtool sign /a /v /p %s /debug /f .\\cert.pfx /t http://timestamp.digicert.com  target\\release\\rustdesk.exe'%pa)
        os.system('cp -rf target/release/RustDesk.exe rustdesk-%s.exe'%version)
    else:
        os.system('cargo bundle --release')
        if osx:
            os.system(
                'strip target/release/bundle/osx/RustDesk.app/Contents/MacOS/rustdesk')
            os.system(
                'cp sciter-osx-64.dylib target/release/bundle/osx/RustDesk.app/Contents/MacOS/')
            # https://github.com/sindresorhus/create-dmg
            os.system('/bin/rm -rf *.dmg')
            os.system('create-dmg target/release/bundle/osx/RustDesk.app')
            os.rename('RustDesk %s.dmg'%version, 'rustdesk-%s.dmg'%version)
        else:
            os.system('mv target/release/bundle/deb/rustdesk*.deb ./rustdesk.deb')
            os.system('dpkg-deb -R rustdesk.deb tmpdeb')
            os.system('mkdir -p tmpdeb/usr/share/rustdesk/files/systemd/')
            os.system(
                'cp rustdesk.service tmpdeb/usr/share/rustdesk/files/systemd/')
            os.system('cp DEBIAN/* tmpdeb/DEBIAN/')
            os.system('strip tmpdeb/usr/bin/rustdesk')
            os.system('cp libsciter-gtk.so tmpdeb/usr/bin/')
            md5_file('usr/share/rustdesk/files/systemd/rustdesk.service')
            md5_file('usr/bin/libsciter-gtk.so')
            os.system('dpkg-deb -b tmpdeb rustdesk.deb; /bin/rm -rf tmpdeb/')
            os.rename('rustdesk.deb', 'rustdesk-%s.deb'%version)
    os.system("mv Cargo.toml.bk Cargo.toml")
    os.system("mv src/main.rs.bk src/main.rs")


def md5_file(fn):
    md5 = hashlib.md5(open('tmpdeb/' + fn, 'rb').read()).hexdigest()
    os.system('echo "%s %s" >> tmpdeb/DEBIAN/md5sums' % (md5, fn))


if __name__ == "__main__":
    main()
