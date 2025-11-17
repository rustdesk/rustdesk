# Building RustDesk

RustDesk is undeniably complicated to build, with lots of moving parts. There are certain key components that need to line up just right to achieve a successful build.

1. Version Table

    Here are the current version numbers needed to successfully build a working RustDesk:

    | Item          | Type            | Version      | Required On |
    |---------------|-----------------|--------------|-------------|
    | Rust          | Toolchain       | 2015 Edition |             |
    | Flutter       | SDK             | 3.24.5       |             |
    | Visual Studio | IDE             | 2022 or 2019 | Windows     |
    | Xcode         | IDE             | at least 15  | OS X        |
    | vcpkg         | Package Manager | Git HEAD     |             |
    | CocoaPods     | Package Manager |              | OS X        |
    | Homebrew      | Package Manager |              | OS X (rec.) |
    | LLVM/Clang    | Compiler        | (latest)     |             |
    | GCC           | Compiler        | <= 14        | Linux, OS X |
    | ffigen        | Flutter Package | 5.0.1        |             |
    | Python        | Language        | 3.x          |             |

1. Prerequisites

    > NB: Package names in parentheses below (such as "(`ninja-build`)") are hints to the actual package name within Apt for **Ubuntu** and related Linux distributions.

    - Disk space: If you are starting from scratch, this process may require up to 20GB free disk space (50GB on Windows)
    - [Windows] The RustDesk build requires that symlink support be enabled. To do this, you must enable Developer Mode in system settings.
        - `start ms-settings:developers`
    - [Windows] Flutter requires Visual Studio 2019 or 2022 to be installed
        - Download the Visual Studio 2022 installer. The Community edition is fine in this case, but if you have a license for another version, that should work too.
            - Link: <https://visualstudio.microsoft.com/vs/community/>
        - Enable the `Desktop development with C++` workload
        - Ensure these components are enabled:
            - `MSVC v143 - VS 2022 C++ x64/x86 build tools`
                - If there is a newer version available, use that instead.
            - `C++ CMake tools for Windows`
            - `Windows 10 SDK'
    - [OS X] Build tools: Ensure that Xcode and command-line build tools are installed.
        - Xcode can be installed from the Apple Store.
        - Command-line tools are installed automatically the first time you try to use them. Open Terminal and run `git`, for instance.
        - [Optional] Install iOS Simulator runtimes with: `xcodebuild -downloadPlatform iOS`
    - [OS X] CocoaPods Package Manager
        - Installs as a Ruby Gem
        - You should install a newer Ruby version first: `brew install ruby`
            - Pay attentiol to `PATH` configuration, as by default the older system installation will still be found first
        - Then the Gem should be able to install: `gem install cocoapods`
        - More information: <https://guides.cocoapods.org/using/getting-started.html#installation>
    - [Linux / OS X] Build tools: Ensure that your environment has the following build tools installed:
        - `pkg-config`
        - `autoconf`
        - `make`
        - `cmake`
        - `ninja` (`ninja-build` for Ubuntu)
        - GCC
            - Linux:
                - `gcc`
                - `g++` (version 13 or 14)
                - `libstdc++` (`libstdc++-dev`)
            - OS X:
                - `brew install gcc@13`
        - NASM
            - Linux: `nasm`
            - OS X:
                Brew installs NASM version 3.01. This version's output has changes that make it incompatible with the build of AOM (below). In order to build AOM, NASM version 2.16.03 should be used. To install this on OS X, commands like these can be used:

                ```
                curl https://www.nasm.us/pub/nasm/releasebuilds/2.16.03/macosx/nasm-2.16.03-macosx.zip -o nasm.zip
                unzip nasm.zip
                rm nasm.zip
                mv nasm-2.16.03 nasm
                export PATH=~/nasm:$PATH
                ```

                Add the last line to `~/.zprofile` to make the change persist.
        - `libtool` (`libtool-bin`)
        They are often already installed, but if not, install with your distribution's package manager (e.g. `apt install pkg-config autoconf make cmake g++-13 ...`).

        > [Linux] NB: On alternatives-based systems, if Clang is installed before GCC, then `/usr/bin/c++` might run Clang instead of GCC. If this happens, you will likely encounter build errors. One way to fix this might be to remove the Clang and GCC packages and then reinstall them starting with GCC `g++` (`g++-13`).

        > [Linux] NB: On the newest systems, as of this writing, `g++` install version 15. This is not compatible with all of the Rust crates needed by RustDesk and will cause build errors. When an earlier version is installed, however, the `/usr/bin/c++` link might not be configured to run it. You may need to explicitly configure your system's alternatives mechanism, with a command such as `update-alternatives --install /usr/bin/c++ c++ ``which g++-13`` 13`, or manually create a symbolic link from `/usr/bin/c++` to the correct path for a compatible `g++` version.

        > [Linux] NB: The previous point notwithstanding, the latest version of the development package for `libstdc++` (e.g. `apt install libstdc++-15-dev`) may still need to be installed in order for the Rust build output to link properly.
    - [Linux / OS X] UI toolkit: Flutter on Linux and OS X requires Gtk 3.
        - This can typically be installed through your distribution's package manager, e.g. `apt install libgtk-3-dev` or `brew install gtk+3`.
    - [Linux] Dependencies: Ensure that the following libraries are installed:
        - `gstreamer` (`gstreamer1.0-gtk3`, `libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`)
        - `pam` (`libpam0g-dev`)
        - `openssl` (`libssl-dev`)
        - `libxdo` (`libxdo-dev`)
        - `libxcb-randr` (`libxcb-randr0-dev`)
        These can usually be installed with your distribution's package manager (e.g. `apt install gstreamer1.0-gtk3 gstreamer1.0-video libgstreamer1.0-dev libssl-dev ...`)
    - Rust: <https://rust-lang.org/tools/install/>
        - Be sure to select the correct architecture
        - [Linux / OS X] Run `rustup` as yourself, not as `root`
        - [Windows] Will install Visual C++ build tools if needed
        - Restart your shell to bring in updates to `PATH`
    - Git:
        - [Windows] Git for Windows: <https//git-scm.com/downloads/win>
        - [Linux] Install with your distribution's package manager (e.g. `apt install git` or `brew install git`)
            - Some installations may already include Git
        - [OS X] The first time you run `git`, you will be prompted to install the Xcode Command Line Tools.
    - Python: <https://www.python.org/downloads/>
        - Some operating systems may already include Python
        - A version of Python 3 is required
        - [Linux Ubuntu-based] If `python3` works but `python` doesn't, run: `apt install python-is-python3`.
        - [Windows] Recommended: Enable the "Add commands directory to your PATH" option during installation
    - LLVM/Clang: <https://releases.llvm.org/download.html>
        - Your operating system may include a package manager that can install `clang` easily (e.g. `apt install clang`)
        - [Windows] LLVM can be installed with this PowerShell command: `winget install --id=LLVM.LLVM -e`
        - [OS X] LLVM/Clang are installed as part of Xcode.
    - VCPKG: <https://learn.microsoft.com/en-us/vcpkg/get_started/get-started>
    - VCPKG Packages:
        - [Windows] Start a new command prompt window using `Developer Command Prompt for VS 2022` to ensure that the latest environment variables are loaded.
        - [Linux, OS X] Ensure that you have an environment variable `VCPKG_ROOT` set up pointing at the path to the VCPKG root, and that this path is also on your `PATH`.
        - In the RustDesk repository root: `vcpkg install`
        - This installs packages according to a configuration laid out in `vcpkg.json`.
        - This configuration places installed packages into a subdirectory `vcpkg_installed` off of the RustDesk root. Each "triplet" gets its own path. These paths need to be added to the environment variables used to locate headers and library files.
            - Windows:
                ```
                set VCPKG_INSTALLED_ROOT=%cd%\vcpkg_installed
                set INCLUDE=%INCLUDE%;%VCPKG_INSTALLED_ROOT%\x64-windows-static\include;%VCPKG_INSTALLED_ROOT%\x64-windows\include
                set _VCPKG_STATIC_LIB=%VCPKG_INSTALLED_ROOT%\x64-windows-static\lib
                set _VCPKG_DYNAMIC_LIB=%VCPKG_INSTALLED_ROOT%\x64-windows\lib
                set _VCPKG_LIB=%_VCPKG_STATIC_LIB%;%_VCPKG_DYNAMIC_LIB%
                set _VCPKG_BIN=%VCPKG_INSTALLED_ROOT%\x64-windows\bin
                set PATH=%VCPKG_ROOT%;%PATH%;%_VCPKG_BIN%
                set LIB=%LIB%;%_VCPKG_LIB%
                set RUSTFLAGS=-L%_VCPKG_STATIC_LIB% -L%_VCPKG_DYNAMIC_LIB%
                ```
            - Linux / OS X:
                - Source in `sh`, `bash`, `zsh`:
                    ```
                    TRIPLET=x64-linux  OR  TRIPLET=x64-osx
                    export "VCPKG_INSTALLED_ROOT=`pwd`/vcpkg_installed"
                    export "INCLUDE=$INCLUDE:$VCPKG_INSTALLED_ROOT/$TRIPLET/include"
                    VCPKG_LIB=$VCPKG_INSTALLED_ROOT/$TRIPLET/lib
                    export "LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$VCPKG_LIB"
                    export "RUSTFLAGS=-L$VCPKG_LIB"
                    ```
                - Source in `csh`, `tcsh`:
                    ```
                    set TRIPLET=x64-linux  OR  TRIPLET=x64-osx
                    setenv VCPKG_INSTALLED_ROOT "`pwd`/vcpkg_installed"
                    setenv INCLUDE "$INCLUDE:$VCPKG_INSTALLED_ROOT/$TRIPLET/include"
                    set VCPKG_LIB=$VCPKG_INSTALLED_ROOT/$TRIPLET/lib
                    setenv LD_LIBRARY_PATH "$LD_LIBRARY_PATH:$VCPKG_LIB"
                    setenv RUSTFLAGS "-L$VCPKG_LIB"
                    ```
                - Not sure which shell you're using? `echo $0` shows it in most cases (though not `csh`)
        - Recommendation: Create a script in the RustDesk repository root with these statements to set up environment variables for VCPKG
    - [Linux / OS X] VCPKG Special case: `opus`
        - The Rust crate `magnum-opus` builds against and links to `libopus` through the VCPKG package `opus`. But, its build script hardcodes VCPKG classic mode (global package cache) and doesn't respect the VCPKG manifest configuration that RustDesk uses. As a result, while `vcpkg` installs `opus` into a subdirectory `vcpkg_installed` of the RustDesk repository, the build looks for the `opus` files underneath `$VCPKG_ROOT`.
        - To resolve this, change directory out of the RustDesk repository, so that no `vcpkg.json` manifest file is visible, and run:
            - `vcpkg install opus`
        - This places the `opus` files in the central VCPKG cache, which is what the `magnum-opus` crate needs to build.
    - Visual Studio Code:
        - Note that Visual Studio 2022 and Visual Studio Code are completely different, unrelated products.
        - Visual Studio Code is Optional but recommended for development and debugging of RustDesk
        - Provides an easy way to install Flutter
        - Free from Microsoft: <https://code.visualstudio.com/download>
        - [Windows] Note that if you have just installed Git for Windows and then you launch Visual Studio Code directly from the installer, the system environment changes for Git might not be in effect. Close Visual Studio Code and launch it manually to resolve this, or simply don't launch Visual Studio Code directly from the installation.
    - Flutter:
        - Within Visual Studio Code:
            - Install the Flutter extension. This also enables comprehension and debugging of Dart code within the IDE.
            - Then, open the Command Palette (default: Ctrl-Shift-P), type "flutter" and select `Flutter: New Project`
            - You will be prompted automatically to download and install the Flutter SDK
            - Note: The installation retrieves the Flutter SDK files using a Git clone. You must select a directory within which `git clone` will be run. A subdirectory called `flutter` will be created.
            - Recommended: If prompted to add the Flutter SDK to PATH, select `Add SDK to PATH`
                - [Linux] You will need to manually update your `PATH` variable to include the Flutter SDK. The SDK was downloaded as a `git clone` into a subdirectory called `flutter` of the directory you chose earlier, and the binaries for the installation are in a subdirectory of that called `bin`. For instance, if you cloned `flutter` to `/home/username/flutter`, then the directory you need to add to `PATH` is `/home/username/flutter/bin`. If you cloned to `/code/flutter`, then the directory is `/code/flutter/bin`.
                    - Add a command to your profile initialization script to update `PATH`
                    - `sh` style shells (including `bash` and `zsh`): `export PATH="$PATH:/path/to/flutter/bin"`
                    - `csh` style shells (including `tcsh`): `setenv PATH "$PATH:/path/to/flutter/bin"`
        - Without Visual Studio Code: Follow instructions at <https://docs.flutter.dev/install/manual>
        - Optional: Disable Web as a build target if you don't intend to use it and don't have Google Chrome installed
            - `flutter config --no-enable-web`
        - Optional: Disable Android as a build target if you don't intend to use it and don't have the Android SDK installed
            - `flutter config --no-enable-android`
        - [OS X] Optional: Disable iOS as a build target if you don't intend to use it
            - `flutter config --no-enable-ios`
            - But `flutter doctor` will still complain if you don't have any installed Simulator runtimes in Xcode
        - Sanity check: `flutter doctor`

1. Git Submodules

    When cloning the RustDesk repository, be sure to enable recursive cloning of submodules with the `--recursive` flag:

    - `git clone https://github.com/rustdesk/rustdesk --recursive`

    If you have already cloned the repository without recursion, you can enable it with the following command:

    - `git submodule update --init --recursive`

    Git remembers the exact commit of each submodule with something called a "gitlink". If the link is pointing at an old commit (which could very well be the case), then you may get out-of-date code for `hbb_common` that will not compile. To prevent this, run the following command:

    - `git submodule update --remote`

    You should use this command after cloning, and also any time that `hbb_common` receives important or breaking changes.

1. Flutter Version

    As of this writing, Continuous Integration builds use Flutter SDK version **3.24.5**. You can use the following commands to enable FVM (Flutter Version Management) and ensure that the correct Flutter version is used:

    ```
    dart pub global activate fvm
    fvm install 3.24.5
    ```

    Note that FVM installs into a cache folder in the user profile. The `dart pub global activate fvm` command outputs the binary path, which needs to be added to your `PATH` environment variable.

1. Rust/Flutter Bridge

    The interface between the Flutter front-end, written in Dart, and the underlying Rust code is an autogenerated bridge. This bridge is generated by a Rust crate called `flutter_rust_bridge_codegen` which must be version **1.80.1**. This crate depends in turn on a Flutter package called `ffigen`. The version of `ffigen` is important, because the code it generates must match the API of the target Flutter version. The `ffigen` version is fixed by an invocation of `dart pub global activate` in `build.py`. As of this writing, the correct version is **5.0.1**.

    To generate the bridge, you need to execute the `flutter_rust_bridge_codegen` utility.

    To activate the correct `ffigen` version and install `flutter_rust_bridge_codegen`, use the following commands:

    ```
    dart pub global activate ffigen --version 5.0.1
    cargo install flutter_rust_bridge_codegen --version 1.80.1
    ```

    Then, run the utility out of the Cargo bin folder. The correct command, executed in the root of the `rustdesk` repository, is:

    - Windows: `flutter_rust_bridge_codegen --rust-input src\flutter_ffi.rs --dart-output flutter\lib\generated_bridge.dart`
    - Linux: `flutter_rust_bridge_codegen --rust-input src/flutter_ffi.rs --dart-output flutter/lib/generated_bridge.dart`
    - OS X: `flutter_rust_bridge_codegen --rust-input src/flutter_ffi.rs --dart-output flutter/lib/generated_bridge.dart --c-output flutter/macos/Runner/bridge_generated.h`

    **You may need to rerun this command after pulling updates to the underlying Rust code in the RustDesk repository or in `hbb_common`.**

1. Underlying Rust Code

    1. The core of RustDesk is written in Rust. Most of the Rust code is local to the `rustdesk` repository, but there is a shared library `hbb_common` that is a separate crate, with its own independent Git repository under the RustDesk umbrella. When you do a fresh clone, the `hbb_common` submodule is initialized to the latest code in that repository, but if you have been developing for a while, it might fall out-of-date, and this can result in build errors or problems at runtime. So, if you are encountering problems, ensure that you have pulled the latest changes for `hbb_common`.

    1. The build of the Rust core crate underlying RustDesk is done using Cargo. The exact parameters to Cargo are important. The `build.py` script does the correct builds in sequence. At a minimum, the `--flutter` flag must be passed into `build.py`; without it, the resulting `librustdesk.dll` lacks the numerous exported functions that the Flutter front-end relies upon.

        - Windows: The build processes underlying Rust crate compilation will require that the Visual C++ build tools be available in the ambient environment. Run these commands from a `Developer Command Prompt for Visual Studio`, or explicitly run `vsvars.

        - The output from the build is placed into a subdirectory of `target` named after the build profile -- `debug` or `release`. Note that Rust builds can consume a very large amount of disk space. As of this writing, a build output folder in `target` may require 40GB or more of disk space for a full build.

        - The `build.py` script by default performs `release` builds. If you want a `debug` build (you probably do), then add the `--debug` command-line switch:
            - `python build.py --flutter --debug`

        - `build.py` first builds the underlying Rust code then builds the Flutter front-end. In Release configuration (without `--debug`), it also automatically tries to build a platform-appropriate installation package.

        - `build.py` is used for continuous integration builds.

1. Flutter Front End

    1. To run the Flutter front end directly for debugging purposes, you can run the following command:

        ```
        cd flutter
        flutter run
        ```

        Note that this uses build output from `target/debug`. Ensure that you have not done a **Release mode** build of the Rust layer, otherwise its build output will be in `target/release` instead of `target/debug`. (You could also choose to launch the Flutter components in Release mode using `flutter run --release`.)

    1. To build the Flutter front end to a stand-alone component, you can run the following command:

        ```
        cd flutter
        flutter build linux
          OR
        flutter build windows
          OR
        flutter build macos
        ```

    > **Flutter Version Compatibility**
    >
    > The latest Flutter version has some breaking changes compared with the version the code is written against. The following local changes may be needed to resolve build issues, if your Flutter SDK version is too new:
    > 
    > 1. Update the version of the `extended_text` dependency in `flutter/pubspec.yaml` to `15.0.0`.
    > 1. Open `lib/common.dart` and make the following edits:
    >     1. Locate two lines that instantiate `DialogTheme`. Change the data type to `DialogThemeData`.
    >     1. Locate two lines that instantiate `TabBarTheme`. Change the data type to `TabBarThemeData`.
