#!/usr/bin/env python3

import os
import platform
import zipfile
import urllib.request
import shutil
import hashlib
import argparse

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


def parse_rc_features(feature):
    available_features = {
        'IddDriver': {
            'zip_url': 'https://github.com/fufesou/RustDeskIddDriver/releases/download/v0.1/RustDeskIddDriver_x64.zip',
            'checksum_url': 'https://github.com/fufesou/RustDeskIddDriver/releases/download/v0.1'
                            '/RustDeskIddDriver_x64.zip.checksum_md5',
        },
        'PrivacyMode': {
            'zip_url': 'https://github.com/fufesou/RustDeskTempTopMostWindow/releases/download/v0.1'
                       '/TempTopMostWindow_x64.zip',
            'checksum_url': 'https://github.com/fufesou/RustDeskTempTopMostWindow/releases/download/v0.1'
                            '/TempTopMostWindow_x64.zip.checksum_md5',
        }
    }
    apply_features = {}
    if not feature:
        return apply_features
    elif isinstance(feature, str) and feature.upper() == 'ALL':
        return available_features
    elif isinstance(feature, list):
        for feat in feature:
            if isinstance(feat, str) and feat.upper() == 'ALL':
                return available_features
            if feat in available_features:
                apply_features[feat] = available_features[feat]
            else:
                print(f'Unrecognized feature {feat}')
        return apply_features
    else:
        raise Exception(f'Unsupported features param {feature}')


def make_parser():
    parser = argparse.ArgumentParser(description='Build script.')
    parser.add_argument(
        '-f',
        '--feature',
        dest='feature',
        metavar='N',
        type=str,
        nargs='+',
        default='',
        help='Integrate features, windows only.'
             'Available: IddDriver, PrivacyMode. Special value is "ALL" and empty "". Default is empty.')
    parser.add_argument('--flutter', action='store_true',
                        help='Build flutter package', default=False)
    parser.add_argument(
        '--hwcodec',
        action='store_true',
        help='Enable feature hwcodec'
    )
    parser.add_argument(
        '--portable',
        action='store_true',
        help='Build windows portable'
    )
    return parser


def download_extract_features(features, res_dir):
    for (feat, feat_info) in features.items():
        print(f'{feat} download begin')
        checksum_md5_response = urllib.request.urlopen(feat_info['checksum_url'])
        checksum_md5 = checksum_md5_response.read().decode('utf-8').split()[0]
        download_filename = feat_info['zip_url'].split('/')[-1]
        filename, _headers = urllib.request.urlretrieve(feat_info['zip_url'], download_filename)
        md5 = hashlib.md5(open(filename, 'rb').read()).hexdigest()
        if checksum_md5 != md5:
            raise Exception(f'{feat} download failed')
        print(f'{feat} download end. extract bein')
        zip_file = zipfile.ZipFile(filename)
        zip_list = zip_file.namelist()
        for f in zip_list:
            zip_file.extract(f, res_dir)
        zip_file.close()
        os.remove(download_filename)
        print(f'{feat} extract end')


def get_rc_features(args):
    features = parse_rc_features(args.feature)
    if features:
        print(f'Build with features {list(features.keys())}')
        res_dir = 'resources'
        if os.path.isdir(res_dir) and not os.path.islink(res_dir):
            shutil.rmtree(res_dir)
        elif os.path.exists(res_dir):
            raise Exception(f'Find file {res_dir}, not a directory')
        os.makedirs(res_dir, exist_ok=True)
        download_extract_features(features, res_dir)
    return ['with_rc'] if features else []
    

def get_features(args):
    features = ['inline']
    if windows:
        features.extend(get_rc_features(args))
    if args.hwcodec:
        features.append('hwcodec')
    if args.flutter:
        features.append('flutter')
    print("features:", features)
    return features

def generate_control_file(version):
    control_file_path = "../res/DEBIAN/control"
    os.system('/bin/rm -rf %s' % control_file_path)

    content = """Package: rustdesk
Version: %s
Architecture: amd64
Maintainer: open-trade <info@rustdesk.com>
Homepage: https://rustdesk.com
Depends: libgtk-3-0, libxcb-randr0, libxdo3, libxfixes3, libxcb-shape0, libxcb-xfixes0, libasound2, libsystemd0, pulseaudio, curl
Description: A remote control software.

""" % version
    file = open(control_file_path, "w")
    file.write(content)
    file.close()

def build_flutter_deb(version):
    os.chdir('flutter')
    os.system('dpkg-deb -R rustdesk.deb tmpdeb')
    os.system('flutter build linux --release')

    os.system('mkdir -p tmpdeb/usr/bin/')
    os.system('mkdir -p tmpdeb/usr/lib/rustdesk')
    os.system('mkdir -p tmpdeb/usr/share/rustdesk/files/systemd/')
    os.system('mkdir -p tmpdeb/usr/share/applications/')
    os.system('mkdir -p tmpdeb/usr/share/polkit-1/actions')

    os.system(
        'cp -r build/linux/x64/release/bundle/* tmpdeb/usr/lib/rustdesk/')
    os.system(
        'pushd tmpdeb && ln -s /usr/lib/rustdesk/flutter_hbb usr/bin/rustdesk && popd')
    os.system(
        'cp build/linux/x64/release/liblibrustdesk.so tmpdeb/usr/lib/rustdesk/librustdesk.so')
    os.system(
        'cp ../res/rustdesk.service tmpdeb/usr/share/rustdesk/files/systemd/')
    os.system(
        'cp ../res/rustdesk.service.user tmpdeb/usr/share/rustdesk/files/systemd/')
    os.system(
        'cp ../res/128x128@2x.png tmpdeb/usr/share/rustdesk/files/rustdesk.png')
    os.system(
        'cp ../res/rustdesk.desktop tmpdeb/usr/share/applications/rustdesk.desktop')
    os.system(
        'cp ../res/com.rustdesk.RustDesk.policy tmpdeb/usr/share/polkit-1/actions/')
    os.system("echo \"#!/bin/sh\" >> tmpdeb/usr/share/rustdesk/files/polkit && chmod a+x tmpdeb/usr/share/rustdesk/files/polkit")

    os.system('mkdir -p tmpdeb/DEBIAN')
    generate_control_file(version)
    os.system('cp -a ../res/DEBIAN/* tmpdeb/DEBIAN/')
    md5_file('usr/share/rustdesk/files/systemd/rustdesk.service')
    md5_file('usr/share/rustdesk/files/systemd/rustdesk.service.user')
    os.system('dpkg-deb -b tmpdeb rustdesk.deb;')

    os.system('/bin/rm -rf tmpdeb/')
    os.system('/bin/rm -rf ../res/DEBIAN/control')
    os.rename('rustdesk.deb', '../rustdesk-%s.deb' % version)
    os.chdir("..")


def build_flutter_arch_manjaro():
    os.chdir('flutter')
    os.system('flutter build linux --release')
    os.system('strip build/linux/x64/release/liblibrustdesk.so')
    os.chdir('..')
    os.system('HBB=`pwd` FLUTTER=1 makepkg -f')

def build_flutter_windows_portable():
    os.system("cargo build --lib --features flutter --release")
    os.chdir('flutter')
    os.system("flutter build windows --release")
    os.chdir('..')
    os.chdir("libs/portable")
    os.system("pip3 install -r requirements.txt")
    os.system("python3 .\\generate.py -f ..\\..\\flutter\\build\\windows\\runner\Release\ -o . -e ..\\..\\flutter\\build\\windows\\runner\\Release\\rustdesk.exe")
    os.chdir("../..")
    if os.path.exists("./rustdesk_portable.exe"):
        os.replace("./target/release/rustdesk-portable-packer.exe", "./rustdesk_portable.exe")
    else:
        os.rename("./target/release/rustdesk-portable-packer.exe", "./rustdesk_portable.exe")
    print(f"output location: {os.path.abspath(os.curdir)}/rustdesk_portable.exe")

def main():
    parser = make_parser()
    args = parser.parse_args()

    os.system("cp Cargo.toml Cargo.toml.bk")
    os.system("cp src/main.rs src/main.rs.bk")
    if windows:
        txt = open('src/main.rs', encoding='utf8').read()
        with open('src/main.rs', 'wt', encoding='utf8') as fh:
            fh.write(txt.replace(
                '//#![windows_subsystem', '#![windows_subsystem'))
    if os.path.exists(exe_path):
        os.unlink(exe_path)
    if os.path.isfile('/usr/bin/pacman'):
        os.system('git checkout src/ui/common.tis')
    version = get_version()
    features = ",".join(get_features(args))
    flutter = args.flutter
    if not flutter:
        # not flutter, is sciter
        os.system('python3 res/inline-sciter.py')
    portable = args.portable
    if windows:
        if portable:
            build_flutter_windows_portable()
            return
        os.system('cargo build --release --features ' + features)
        # os.system('upx.exe target/release/rustdesk.exe')
        os.system('mv target/release/rustdesk.exe target/release/RustDesk.exe')
        pa = os.environ.get('P')
        if pa:
            os.system(f'signtool sign /a /v /p {pa} /debug /f .\\cert.pfx /t http://timestamp.digicert.com  '
                      'target\\release\\rustdesk.exe')
        else:
            print('Not signed')
        os.system(f'cp -rf target/release/RustDesk.exe rustdesk-{version}-setdown.exe')
    elif os.path.isfile('/usr/bin/pacman1'):
        # pacman -S -needed base-devel
        os.system("sed -i 's/pkgver=.*/pkgver=%s/g' PKGBUILD" % version)
        if flutter:
            build_flutter_arch_manjaro()
        else:
            os.system('cargo build --release --features ' + features)
            os.system('git checkout src/ui/common.tis')
            os.system('strip target/release/rustdesk')
            os.system('ln -s res/pacman_install && ln -s res/PKGBUILD')
            os.system('HBB=`pwd` makepkg -f')
        os.system('mv rustdesk-%s-0-x86_64.pkg.tar.zst rustdesk-%s-manjaro-arch.pkg.tar.zst' % (version, version))
        # pacman -U ./rustdesk.pkg.tar.zst
    elif os.path.isfile('/usr/bin/yum'):
        os.system('cargo build --release --features ' + features)
        os.system('strip target/release/rustdesk')
        os.system("sed -i 's/Version:    .*/Version:    %s/g' res/rpm.spec" % version)
        os.system('HBB=`pwd` rpmbuild -ba res/rpm.spec')
        os.system('mv $HOME/rpmbuild/RPMS/x86_64/rustdesk-%s-0.x86_64.rpm ./rustdesk-%s-fedora28-centos8.rpm' % (
            version, version))
        # yum localinstall rustdesk.rpm
    elif os.path.isfile('/usr/bin/zypper'):
        os.system('cargo build --release --features ' + features)
        os.system('strip target/release/rustdesk')
        os.system("sed -i 's/Version:    .*/Version:    %s/g' res/rpm-suse.spec" % version)
        os.system('HBB=`pwd` rpmbuild -ba res/rpm-suse.spec')
        os.system('mv $HOME/rpmbuild/RPMS/x86_64/rustdesk-%s-0.x86_64.rpm ./rustdesk-%s-suse.rpm' % (version, version))
        # yum localinstall rustdesk.rpm
    else:
        os.system('cargo bundle --release --features ' + features)
        if flutter:
            if osx:
                # todo: OSX build
                pass
            else:
                os.system(
                    'mv target/release/bundle/deb/rustdesk*.deb ./flutter/rustdesk.deb')
                build_flutter_deb(version)
        else:
            if osx:
                os.system(
                    'strip target/release/bundle/osx/RustDesk.app/Contents/MacOS/rustdesk')
                os.system(
                    'cp libsciter.dylib target/release/bundle/osx/RustDesk.app/Contents/MacOS/')
                # https://github.com/sindresorhus/create-dmg
                os.system('/bin/rm -rf *.dmg')
                plist = "target/release/bundle/osx/RustDesk.app/Contents/Info.plist"
                txt = open(plist).read()
                with open(plist, "wt") as fh:
                    fh.write(txt.replace("</dict>", """
    <key>LSUIElement</key>
    <string>1</string>
    </dict>"""))
                pa = os.environ.get('P')
                if pa:
                    os.system('''
    # buggy: rcodesign sign ... path/*, have to sign one by one
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/RustDesk.app/Contents/MacOS/rustdesk
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/RustDesk.app/Contents/MacOS/libsciter.dylib
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/RustDesk.app
    # goto "Keychain Access" -> "My Certificates" for below id which starts with "Developer ID Application:"
    codesign -s "Developer ID Application: {0}" --force --options runtime  ./target/release/bundle/osx/RustDesk.app/Contents/MacOS/*
    codesign -s "Developer ID Application: {0}" --force --options runtime  ./target/release/bundle/osx/RustDesk.app
    '''.format(pa))
                os.system('create-dmg target/release/bundle/osx/RustDesk.app')
                os.rename('RustDesk %s.dmg' %
                          version, 'rustdesk-%s.dmg' % version)
                if pa:
                    os.system('''
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./rustdesk-{1}.dmg
    codesign -s "Developer ID Application: {0}" --force --options runtime ./rustdesk-{1}.dmg
    # https://pyoxidizer.readthedocs.io/en/latest/apple_codesign_rcodesign.html
    rcodesign notarize --api-issuer 69a6de7d-2907-47e3-e053-5b8c7c11a4d1 --api-key 9JBRHG3JHT --staple ./rustdesk-{1}.dmg
    # verify:  spctl -a -t exec -v /Applications/RustDesk.app
    '''.format(pa, version))
                else:
                    print('Not signed')
            else:
                # buid deb package
                os.system(
                    'mv target/release/bundle/deb/rustdesk*.deb ./rustdesk.deb')
                os.system('dpkg-deb -R rustdesk.deb tmpdeb')
                os.system('mkdir -p tmpdeb/usr/share/rustdesk/files/systemd/')
                os.system(
                    'cp res/rustdesk.service tmpdeb/usr/share/rustdesk/files/systemd/')
                os.system(
                    'cp res/rustdesk.service.user tmpdeb/usr/share/rustdesk/files/systemd/')
                os.system(
                    'cp res/128x128@2x.png tmpdeb/usr/share/rustdesk/files/rustdesk.png')
                os.system(
                    'cp res/rustdesk.desktop tmpdeb/usr/share/applications/rustdesk.desktop')
                os.system('cp -a res/DEBIAN/* tmpdeb/DEBIAN/')
                os.system('strip tmpdeb/usr/bin/rustdesk')
                os.system('mkdir -p tmpdeb/usr/lib/rustdesk')
                os.system('cp libsciter-gtk.so tmpdeb/usr/lib/rustdesk/')
                md5_file('usr/share/rustdesk/files/systemd/rustdesk.service')
                md5_file('usr/share/rustdesk/files/systemd/rustdesk.service.user')
                md5_file('usr/lib/rustdesk/libsciter-gtk.so')
                os.system('dpkg-deb -b tmpdeb rustdesk.deb; /bin/rm -rf tmpdeb/')
                os.rename('rustdesk.deb', 'rustdesk-%s.deb' % version)
    os.system("mv Cargo.toml.bk Cargo.toml")
    os.system("mv src/main.rs.bk src/main.rs")


def md5_file(fn):
    md5 = hashlib.md5(open('tmpdeb/' + fn, 'rb').read()).hexdigest()
    os.system('echo "%s %s" >> tmpdeb/DEBIAN/md5sums' % (md5, fn))


if __name__ == "__main__":
    main()
