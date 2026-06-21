# Project Roadmap

This document outlines the development phases, release path, and upstream alignment strategy for Vestra Support.

---

## Upstream Strategy

Vestra Support is intended to remain a **thin downstream distribution** of the official RustDesk project. 
To minimize maintenance overhead and ensure long-term stability:
* **Prioritize Upstream Alignment**: Custom code changes should be restricted strictly to branding assets, configuration overrides (default server IP/DNS and keys), and minimal API integration hooks.
* **Avoid Divergence**: We will avoid refactoring core application logic, custom UI layouts, or underlying protocol structures. Any divergence makes merging new upstream releases significantly more complex.
* **Upstream Integration**: Critical bug fixes or performance enhancements discovered during development should, where appropriate, be contributed back to the upstream RustDesk repository rather than maintained locally.
* **Continuous Merges**: The project team will schedule regular reviews to pull latest stable releases, security patches, and hotfixes from the upstream `rustdesk/rustdesk` repository.

---

## Release Phases

### Phase 0: Foundations & Scaffolding (Current Phase)
* [x] **Fork Established**: Official RustDesk repository forked to `vestrainteractive/vestra-support`.
* [x] **Licensing Reviewed**: Detailed analysis of AGPL-3.0 copyleft obligations and licensing implications.
* [x] **Repository Initialized**: Workspace set up with initial monorepo scaffolding and project documentation.

### Phase 1: Relay Infrastructure & Validation
* [ ] **Deploy Relay Server**: Set up self-hosted instances of `hbbs` (ID server) and `hbbr` (Relay server) in Vestra's infrastructure environment.
* [ ] **Generate Server Keys**: Generate secure public/private key pairs for encryption and connection authentication.
* [ ] **Stock Validation**: Verify that standard, unmodified RustDesk clients can connect, register, and establish sessions using the newly deployed Vestra relay server configuration.
* [ ] **Define Deployment Checklist**: Document server setup and system configuration tasks for staging and production environments.

### Phase 2: Branding Layer & Custom Configuration
* [ ] **App Name Customization**: Modify application and executable names to "Vestra Support".
* [ ] **Visual Identity**: Replace all logos, window headers, icons, and status assets in the `res/` directory.
* [ ] **Pre-configure Connections**: Embed default Vestra ID server, relay server, and public key configurations into the build configuration.
* [ ] **About Dialog Update**: Implement the customized About dialog detailing the fork's basis, source code link, and AGPL-3.0 license.
* [ ] **Support URLs**: Update client help links, error message directions, and manual downloads links to point to `support.vestrainteractive.com`.

### Phase 3: Build & Release Pipeline
* [ ] **Multi-Platform Build Setup**: Establish automated build workflows (using GitHub Actions or local runners) for Windows (`.exe` / `.msi`) and macOS (`.dmg`).
* [ ] **Code Signing**: Integrate Vestra's developer certificates to sign binaries, preventing operating system warnings (SmartScreen / Gatekeeper) on client machines.
* [ ] **Automated Release Scaffolding**: Configure the release pipeline to generate downloadable assets when version tags are pushed.

### Phase 4: Helpdesk Portal Integration
* [ ] **Core API Integration**: Hook the support client's status logs or registration calls into the Vestra Core backend.
* [ ] **Technician Portal Dashboard**: Build a simplified console inside the Vestra Admin interface for staff to track active support IDs.
* [ ] **Session Logs**: Record session timing and connection logs to enable tracking and support history.

### Phase 5: Tactical RMM Integration
* [ ] **Launcher Script**: Create script utilities to launch Vestra Support sessions directly from the Tactical RMM workstation view.
* [ ] **Unattended Configuration**: Package the Vestra Support client for silent installation and service setup via the RMM deployment manager.
* [ ] **Bi-directional Mapping**: Sync machine status and hardware IDs between the RMM portal and Vestra support tables.
