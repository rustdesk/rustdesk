### RustDesk | Your Remote Desktop Software

Yet another open source remote desktop software.

[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

## How To Build

* Prepare your Rust development env and C++ build env

* Install [vcpkg](https://github.com/microsoft/vcpkg), and set VCPKG_ROOT env variable correctly

   - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static
   - Linux/Osx: vcpkg install libvpx libyuv opus
   
* cargo run
