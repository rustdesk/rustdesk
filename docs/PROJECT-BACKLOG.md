# Project Backlog

This document defines critical backlog items and work specifications required to move Vestra Support through its roadmap phases.

---

## 1. Branding Layer Customization (Phase 2)

**Title**: Implement Vestra Support Branding Layer  
**Type**: Task / Feature  
**Target Milestone**: Phase 2  

### Description
Replace default RustDesk visual assets and metadata with Vestra Support branding across desktop configurations.

### Requirements
* Modify `Cargo.toml` properties:
  * Update binary name and product metadata under `[package.metadata.winres]` and `[package.metadata.bundle]`.
* Replace asset images in `res/` directory:
  * SVGs and PNG icon sizes (32x32, 128x128).
* Update Flutter bundle and app settings for desktop (Windows/macOS).
* Override standard About Dialog text:
  ```text
  Vestra Support
  Based on RustDesk Open Source Software
  Source Code: https://github.com/vestrainteractive/vestra-support
  License: AGPL-3.0
  ```
* Ensure the repository link in the dialog matches our fork and is clickable.

---

## 2. Relay Infrastructure Deployment (Phase 1)

**Title**: Deploy Self-Hosted ID and Relay Servers  
**Type**: Infrastructure / DevOps  
**Target Milestone**: Phase 1  

### Description
Deploy self-hosted instances of `hbbs` (ID server) and `hbbr` (Relay server) to anchor all connections to private Vestra hardware.

### Requirements
* Provision a public VPS or cloud instance for hosting.
* Set up Docker containers for `hbbs` and `hbbr` or install standalone binaries.
* Configure DNS A records:
  * `id.support.vestrainteractive.com` -> pointing to `hbbs` instance.
  * `relay.support.vestrainteractive.com` -> pointing to `hbbr` instance.
* Configure Firewall security groups to expose necessary TCP/UDP ports:
  * TCP: 21115, 21116, 21117, 21118, 21119
  * UDP: 21116
* Generate keypairs (`id_ed25519` and `id_ed25519.pub`) on startup and backup key assets.

---

## 3. First Internal Build (Phase 3)

**Title**: Establish Automated Multi-Platform Build Pipeline  
**Type**: DevOps / Build  
**Target Milestone**: Phase 3  

### Description
Create automated build configurations to compile, code-sign, and output signed Vestra Support client binaries.

### Requirements
* Configure GitHub Actions workflows or local build runners for Windows and macOS.
* Set up dependencies (vcpkg, rustup, Flutter SDK, Sciter dependencies where appropriate).
* Pre-configure build parameters to embed target connection defaults:
  * Server DNS: `id.support.vestrainteractive.com`
  * Server Public Key: (Phase 1 public key string)
* Integrate Vestra developer credentials to code-sign executables (`.exe`, `.msi`, `.dmg`) to prevent operating system security blocks on client launch.

---

## 4. Compliance Review (Phase 5 / Continuous)

**Title**: Complete Open-Source License and Compliance Audit  
**Type**: Operations / Compliance  
**Target Milestone**: Continuous  

### Description
Review the codebase, documentation, and distribution channels to guarantee absolute alignment with AGPL-3.0 license requirements and trademark protections.

### Requirements
* Audit the public GitHub repository to confirm the presence of the complete matching source code for any distributed binaries.
* Verify that copyright headers are preserved exactly as required by the AGPL-3.0 license.
* Ensure all links pointing to source code (in About Dialogs, client portal pages, and emails) are functional.
* Review build profiles to confirm "RustDesk" trademarks are not used in user-facing components, renaming all binary resources to "Vestra Support".
