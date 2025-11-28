# Gemini Code Companion: RustDesk Flutter Client

This document provides a developer-centric overview of the RustDesk Flutter client codebase.

## Project Overview

This is the Flutter-based client for RustDesk, an open-source remote desktop software. The application provides a cross-platform user interface for connecting to and managing remote devices.

**Key Technologies:**

*   **Flutter:** Used for building the user interface for mobile, desktop, and web.
*   **Rust:** The core logic of RustDesk is written in Rust. This Flutter application communicates with the Rust core.
*   **`flutter_rust_bridge`:** The primary mechanism for communication between the Flutter frontend and the Rust backend.
*   **`desktop_multi_window`:** Used on desktop platforms to manage multiple windows for different functionalities (e.g., main window, remote session, file transfer).

**Architecture:**

The application follows a typical Flutter structure, with a significant architectural consideration being the interaction with the Rust core.

*   `lib/main.dart`: The main entry point of the application. It handles platform detection (mobile vs. desktop) and initializes the appropriate UI.
*   `lib/common`: Contains shared code used across different parts of the application.
*   `lib/desktop` and `lib/mobile`: Contain platform-specific UI and logic.
*   `lib/models`: Defines data models used throughout the application.
*   `lib/native`: Contains code for interacting with the native platform, primarily through the Rust bridge.
*   `rust/`: (Not present in this directory, but part of the larger RustDesk project) The Rust core of the application.

## Building and Running

**1. Prerequisites:**

*   Flutter SDK
*   Rust toolchain (including `cargo`)

**2. Setup:**

*   Install Flutter dependencies: `flutter pub get`
*   Build the Rust core (instructions should be in the root of the RustDesk repository).

**3. Running the application:**

*   **Android/iOS:** `flutter run`
*   **Desktop (Linux, macOS, Windows):** `flutter run -d <linux|macos|windows>`

**Note:** The build process involves compiling the Rust core and making it available to the Flutter application. Refer to the main RustDesk repository's documentation for detailed instructions on building the entire project. The scripts in the root of this directory (e.g., `build_android.sh`, `build_ios.sh`) are used for building the final release versions of the application and likely depend on the Rust core being built first.

## Development Conventions

*   **State Management:** The project uses `provider` for state management.
*   **FFI:** Interaction with the Rust core is handled via `flutter_rust_bridge`. All native calls go through this bridge.
*   **Platform-Specific Code:** The codebase is organized to separate mobile and desktop concerns. Shared widgets and logic are in `lib/common`.
*   **Multi-window:** On desktop, the application uses a multi-window approach to provide a more native-like experience. Each window runs in its own isolate.
*   **Styling:** The app has its own design system, `MyTheme`, which supports both light and dark modes.
