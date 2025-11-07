# Building RustDesk

RustDesk is undeniably complicated to build, with lots of moving parts. There are certain key components that need to line up just right to achieve a successful build.

1. Version Table

    Here are the current version numbers needed to successfully build a working RustDesk:

    | Item           | Type            | Version      |
    |----------------|-----------------|--------------|
    | Rust           | Toolchain       | 2015 Edition |
    | Flutter        | SDK             | 3.24.5       |
    | Visual Studio* | IDE             | 2022 or 2019 |
    | vcpkg          | Package Manager | Git HEAD     |
    | LLVM/Clang     | Compiler        | (latest)     |
    | ffigen         | Flutter Package | 5.0.1        |
    | Python         | Language        | 3.x          |

    > * (Windows only)

1. Prerequisites

    - Disk space: If you are starting from scratch, this process may require up to 50GB free disk space
 BLONK SLATE: 76,724,314,112 bytes free
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
    - Rust: <https://rust-lang.org/tools/install/>
        - Be sure to select the correct architecture
        - [Windows] Will install Visual C++ build tools if needed
    - Git:
        - [Linux/OS X] Install with your distribution's package manager (e.g. `apt install git` or `brew install git`)
        - [Windows] Git for Windows: <https//git-scm.com/downloads/win>
        - Some operating systems may already include Git
    - Python: <https://www.python.org/downloads/>
        - Some operating systems may already include Python
        - A version of Python 3 is required
        - [Windows] Recommended: Enable the "Add commands directory to your PATH" option during installation
    - LLVM/Clang: <https://releases.llvm.org/download.html>
        - Your operating system may include a package manager that can install `clang` easily (e.g. `apt install clang`)
        - [Windows] LLVM can be installed with this PowerShell command: `winget install --id=LLVM.LLVM -e`
    - VCPKG: <https://learn.microsoft.com/en-us/vcpkg/get_started/get-started>
    - VCPKG Packages:
        - [Windows] Start a new command prompt window using `Developer Command Prompt for VS 2022` to ensure that the latest environment variables are loaded.
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
            - Linux:
                ```
                set VCPKG_INSTALLED_ROOT=`pwd`/vcpkg_installed
                export "INCLUDE=$INCLUDE:$VCPKG_INSTALLED_ROOT/x64-linux/include
                set VCPKG_LIB=$VCPKG_INSTALLED_ROOT/x64-linux/lib
                export "LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$VCPKG_LIB"
                export "RUSTFLAGS=-L$VCPKG_LIB"
                ```
            - OS X:
                ```
                set VCPKG_INSTALLED_ROOT=`pwd`/vcpkg_installed
                export "INCLUDE=$INCLUDE:$VCPKG_INSTALLED_ROOT/x64-osx/include
                set VCPKG_LIB=$VCPKG_INSTALLED_ROOT/x64-osx/lib
                export "DYLD_FALLBACK_LIBRARY_PATH=$DYLD_FALLBACK_LIBRARY_PATH:$VCPKG_LIB"
                export "RUSTFLAGS=-L$VCPKG_LIB"
                ```
        - Recommendation: Create a script in the RustDesk repository root with these statements to set up environment variables for VCPKG
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
        - Without Visual Studio Code: Follow instructions at <https://docs.flutter.dev/install/manual>
        - Optional: Disable Web as a build target if you don't intend to use it and don't have Google Chrome installed
            - `flutter config --no-enable-web`
        - Optional: Disable Android as a build target if you don't intend to use it and don't have the Android SDK installed
            - `flutter config --no-enable-android`
        - Sanity check: `flutter doctor`

1. Git Submodules

    When cloning the RustDesk repository, be sure to enable recursive cloning of submodules with the `--recursive` flag:

    - `git clone https://github.com/rustdesk/rustdesk --recursive`

    If you have already cloned the repository without recursion, you can enable it with the following command:

    - `git submodule update --init --recursive`

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

    Then, run the utility out of the Cargo bin folder. This is typically `.cargo/bin` off of your home directory (on Windows as well). The correct command, executed in the root of the repository, is:

    - Windows: `%HOMEDRIVE%%HOMEPATH%\.cargo\bin\flutter_rust_bridge_codegen --rust-input src\flutter_ffi.rs --dart-output flutter\lib\generated_bridge.dart`
    - Others: `~/.cargo/bin/flutter_rust_bridge_codegen --rust-input src/flutter_ffi.rs --dart-output flutter/lib/generated_bridge.dart`

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
    > The latest Flutter version has some breaking changes compared with the version the code is written against. The followign changes may be needed to resolve build issues, if your Flutter SDK version is too new:
    > 
    > 1. Update the version of the `extended_text` dependency in `flutter/pubspec.yaml` to `15.0.0`.
    > 1. Open `lib/common.dart` and make the following edits:
    >     1. Locate two lines that instantiate `DialogTheme`. Change the data type to `DialogThemeData`.
    >     1. Locate two lines that instantiate `TabBarTheme`. Change the data type to `TabBarThemeData`.
