# Branding & Asset Customization Guide

This document defines the visual branding and identity specifications for the custom Vestra Support client build. These changes are planned for **Phase 2** of the project roadmap.

---

## What to Modify

To customize the client and distance it from standard RustDesk branding, the following modifications must be performed:

### 1. Application Name & Product Metadata
Replace all instances of the product name **RustDesk** with **Vestra Support**:
* **Cargo Configuration (`Cargo.toml`)**:
  * Update `[package]` metadata (`name`, `description`).
  * Update `[package.metadata.winres]` properties:
    * `ProductName = "Vestra Support"`
    * `FileDescription = "Vestra Support Client"`
    * `OriginalFilename = "vestra-support.exe"`
  * Update `[package.metadata.bundle]` configuration (`name`, `identifier = "com.vestrainteractive.support"`).
* **Flutter Configs (`flutter/pubspec.yaml`)**:
  * Update application names, package names, and build descriptions for target platforms.

### 2. Branding Assets (Logos & Icons)
Replace the default icons and visual logos located in the repository:
* **Resource Directory (`res/`)**:
  * Overwrite standard header banners, logo SVGs, and icon PNGs.
  * Windows App Icon files: Replace `res/32x32.png`, `res/128x128.png`, and related multi-resolution icon profiles.
* **Platform Assets**:
  * **Flutter Assets**: Replace assets in `flutter/assets/` and platform-specific resource folders (e.g. `flutter/android/app/src/main/res/` and `flutter/ios/Runner/Assets.xcassets/`).

### 3. Support & Project URLs
Ensure the client redirects users to Vestra resources on action/error:
* **Help & Website links**: Modify menu links pointing to `https://rustdesk.com` to point to Vestra support `https://support.vestrainteractive.com`.
* **Download references**: Change self-update and direct download URLs.

### 4. Custom Default Settings
To enable "zero-configuration" connections, the build configuration must pre-load Vestra's private network parameters. Hardcode the default values for:
* **ID Server**: `id.support.vestrainteractive.com`
* **Relay Server**: `relay.support.vestrainteractive.com`
* **Public Key**: The public encryption key generated in Phase 1 (extracted from server `id_ed25519.pub`).

---

## About Dialog Specifications

The standard application About Dialog must be replaced with the following copy and links:

```text
Vestra Support

Based on RustDesk Open Source Software

Source Code:
https://github.com/vestrainteractive/vestra-support

License:
AGPL-3.0
```

*Ensure the Source Code link remains fully clickable and navigates to the public repository to fulfill AGPL source disclosure obligations.*

---

## What NOT to Modify

> [!CAUTION]
> Under no circumstances should the following components be altered or modified:
> 1. **Encryption & Cryptography**: Keep standard NaCl/libsodium implementations intact. Modifications risk breaking connection security.
> 2. **Networking Protocols**: The custom client must communicate using standard RustDesk network protocols to maintain compatibility with upstream relay servers and official APIs.
> 3. **Session Authentication Hooks**: The default access token, server public key validation, and password exchange routines must be preserved.
> 4. **Third-Party Copyrights**: Original licensing tags, upstream source file headers, and copyright notices for RustDesk contributors must be preserved exactly as-is.
