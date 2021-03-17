# hbb
cargo install cargo-bundle
set VCPKG_ROOT
sudo apt install libgtk-3-dev clang libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev librust-alsa-sys-dev libpulse-dev cmake

# 关于静态链接VC运行时, 参见.cargo/config

# 关于OSX PreLogin鼠标操作权限，参见.cargo/config, 
# https://stackoverflow.com/questions/41429524/how-to-simulate-keyboard-and-mouse-events-using-cgeventpost-in-login-window-mac

build libvpx:
read README
cygwin required for windows (mingw not work)
put yasm.exe in **"Community"** version
put msbuild.exe of **"BuildTools"** version into path variable
failed to build under virtualbox, seems some asm not supported under virtualbox
../libvpx/configure --target=x86_64-win64-vs16 --enable-static-msvcrt --disable-webm-io --disable-unit-tests --disable-examples --disable-libyuv --disable-postproc --disable-vp8 --disable-tools --disable-docs

