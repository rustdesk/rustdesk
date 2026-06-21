# Open-Source Compliance Guidelines

This document provides compliance guidelines for the development and distribution of the Vestra Support client and relay server infrastructure.

> [!CAUTION]
> **Disclaimer**: This document does not constitute legal advice. It is intended for operational and engineering guidance to ensure standard open-source license compliance. For specific legal inquiries regarding license liabilities, trademark rights, or proprietary integration constraints, please consult qualified legal counsel.

---

## License Context: AGPL-3.0

The upstream RustDesk project is licensed under the **GNU Affero General Public License version 3 (AGPL-3.0)**. As a downstream fork, Vestra Support is bound by all provisions, obligations, and copyleft triggers of the AGPL-3.0.

### Key AGPL Obligations

1. **Copyleft & Derivative Works**: Any modifications, updates, or integrations built directly into the RustDesk codebase are considered derivative works and must be licensed under the AGPL-3.0.
2. **Network Interaction Trigger (Section 13)**: Unlike standard GPL, if you run a modified version of the software on a server and let users interact with it over a network (e.g., custom relays, web consoles, portal integrations), you **must** make the source code of that modified version available to those users.
3. **Attribution**: Original copyright and author notices must be preserved in all modified source files.
4. **No Additional Restrictions**: You cannot impose additional licensing terms, DRM, or proprietary wrappers that restrict the rights granted under the AGPL-3.0.

---

## Compliance Requirements

### 1. Source Disclosure
To comply with Section 13 of the AGPL-3.0, Vestra Interactive must make the complete source code of the Vestra Support fork available to all end-users who download or interact with the client:
* **Public Repository**: The repository `https://github.com/vestrainteractive/vestra-support` must remain public and contain all source modifications, build scripts, and configuration overrides.
* **Prominent Links**: Clear and visible links to the source repository must be provided:
  * In the application's **About Dialog**.
  * On the download page of the Vestra Support portal.
  * In user documentation and onboarding materials.

### 2. Attribution
Original copyright headers and licenses must remain intact:
* **License File**: The root `LICENSE` or `LICENCE` file containing the AGPL-3.0 text must be preserved.
* **Author Credit**: Copyright headers in files belonging to upstream RustDesk authors must not be removed or altered.
* **Modification Notices**: New files or heavily modified source files should contain comments indicating that they are based on RustDesk and modified by Vestra Interactive.

### 3. Trademark Considerations
To avoid trademark infringement and user confusion:
* **Product Naming**: The application must not identify itself simply as "RustDesk" to end-users. All user-facing assets must be renamed to **Vestra Support**.
* **Executables**: Executables must be compiled with a distinct name (e.g. `vestra-support.exe`) and configure distinct package IDs (e.g. `com.vestrainteractive.support` instead of `com.carriez.rustdesk`).
* **Visuals**: Standard RustDesk icons and logos must be replaced with Vestra Support visual assets.
* **Disclaimer of Affiliation**: The About dialog and documentation must state clearly that the product is *based on* RustDesk open-source software but is not affiliated with or endorsed by Purslane Ltd. (the creators of RustDesk).

---

## Release Checklist (Engineering Guidelines)

Before publishing any source release:
- [ ] Ensure all local changes, including configuration files and assets, are pushed to the public GitHub repository.
- [ ] Verify that no private credentials, database connection strings, or production signing keys are committed to the repository history.
- [ ] Confirm the presence of the `LICENSE` file in the repository root.
- [ ] Verify that all files containing modifications retain appropriate licensing notices.

---

## Binary Distribution Checklist (Production Guidelines)

Before distributing Vestra Support binaries to customers:
- [ ] Verify the binary was compiled from a git commit that is pushed and publicly visible on the GitHub repository.
- [ ] Confirm the About dialog displays the correct version, source link (`https://github.com/vestrainteractive/vestra-support`), and AGPL-3.0 license text.
- [ ] Ensure binaries are code-signed with Vestra's developer certificates to prevent OS-level execution blocks.
- [ ] Verify that the support portal download page contains a clear link to the public source code repository.
