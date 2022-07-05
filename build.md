# Install arm64 rustdesk

Install rustdesk for x64 version. 

Extract arm64.zip from https://github.com/sj6219/rustdesk/releases/tag/1.1.10_alpha/ to C:\Program Files\RustDesk

# Build arm64 rustdesk

Install visual studio 2022 and add the following components.

MSVC v143 - VS 2022 c++ ARM64 build tools(Latest)

Install LLVM and perl and add them to the environment variable path.


Perform the following:

vcpkg install libvpx:arm64-windows-static libyuv:arm64-windows-static opus:arm64-windows-static

%comspec% /k "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsamd64_arm64.bat" 

cargo build --release --target=aarch64-pc-windows-msvc 

# sciter.dll

Download from https://github.com/c-smile/sciter-sdk/blob/master/bin.win/arm64/sciter.dll.

# Build libsodium.dll

Build ReleaseDll version at https://github.com/sj6219/libsodium/tree/1.0.18_alpha.
