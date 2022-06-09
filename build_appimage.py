#!/usr/bin/python3
import os

if __name__ == '__main__':
    os.chdir("appimage")
    ret = os.system("appimage-builder --recipe AppImageBuilder.yml --skip-test")
    if ret == 0:
        print("RustDesk AppImage build success :)")
        print("Check AppImage in '/path/to/rustdesk/appimage/RustDesk-VERSION-TARGET_PLATFORM.AppImage'")
    else:
        print("RustDesk AppImage build failed :(")
