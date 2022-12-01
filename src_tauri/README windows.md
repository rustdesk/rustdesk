# Install Tauri CLI
- cargo install tauri-cli

- Prepare your Rust development env and C++ build env

- Install [vcpkg](https://github.com/microsoft/vcpkg), and set `VCPKG_ROOT` env variable correctly

## Windows: 
  In cmd:
  1) install vcpkg  
  `git clone git@github.com:microsoft/vcpkg.git`

  `./vcpkg/vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static`
  
  `./vcpkg/vcpkg integrate install`
  
  `set VCPKG_ROOT=D:\rust\tauri\vcpkg`
  
  `set VCPKGRS_DYNAMIC=1`
  2) download and install Mingw64;
  3) install [LLVM](https://rust-lang.github.io/rust-bindgen/requirements.html) 
  `pacman -S  mingw64/mingw-w64-x86_64-clang`;
  4) download and install Mingw64 and Microsoft Visual Studio: desktop development C++ extension, english language pack;
  5) set `VCINSTALLDIR` env variable correctly: `set VCINSTALLDIR=C:\Program Files\Microsoft Visual Studio\2022\Community`.
   
# Run in dev mode
`cargo tauri dev`

<!-- rustup doc --std -->