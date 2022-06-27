# How to build and run RustDesk in AppImage

Begin by installing `appimage-builder` and predependencies mentioned in official website.

Assume that `appimage-builder` is setup correctly, run commands below, `bash` or `zsh` is recommended:

```bash
cd /path/to/rustdesk_root
./build_appimage.py
```

After a success package, you can see the message in console like:

```shell
INFO:root:AppImage created successfully
```

The AppImage package is shown in `./appimage/RustDesk-VERSION-TARGET_PLATFORM.AppImage`. 

Note: AppImage version of rustdesk is an early version which requires more test. If you find problems, please open an issue.