# Project Status: RustDesk Customization & GCP Integration (2026-06-06)

This document tracks the overall status, progress, active configurations, and milestones for the custom RustDesk project as of June 6, 2026.

---

## 1. Executive Summary
The project constructs a custom remote desktop client pre-configured with custom server domains, coupled with a serverless GCP API backend that handles user authentication and client group/address book lists via Firebase Auth and Cloud Firestore.

---

## 2. Completed Milestones

### Client-Side Repacking
* **Installer Compiled & Verified**: Generated pre-configured client setup binary at [RustDesk_Cislink_Setup.exe](file:///D:/Rustdesk/RustDesk_Cislink_Setup.exe).
* **Inno Setup Scripting**: Completed packaging logic for automatic directory creation, config bootstrapping, custom icon branding (`res\cislink.ico`), registry protocol integration, and automatic running-process cleanup.

### Custom GCP API Backend Server
* **Build Verification**: The TypeScript project in [gcp-api-server](file:///D:/Rustdesk/gcp-api-server) compiles cleanly.
* **Shared Address Books**: Implemented dynamic shared address book profiles via `/api/ab/shared/profiles` along with permission check wrappers (`hasReadAccess`/`hasWriteAccess`).
* **Firebase & Firestore Integration**: Ready for deployment to Cloud Run.

### Server Stability & Keys
* **Key Alignment**: Verified that `hbbs` container uses key pair matching the public key `VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`.
* **Cloud Backup**: Rclone backups of Sqlite database, active server key pair, and configuration metadata completed successfully to Google Drive.

---

## 3. Active Configuration

| Config Item | Selected Option / Value | Notes |
| :--- | :--- | :--- |
| **Backend Runtime** | Node.js v22 (Express + TypeScript) | Compiles to ES2022 CommonJS |
| **API Code Directory** | `D:\Rustdesk\gcp-api-server` | Standalone sub-folder inside monorepo |
| **Auth Provider** | Firebase Auth (REST wrapper) | Validates email/password via REST API |
| **Database** | Cloud Firestore | Flexible, no-management NoSQL documents |
| **Local Port** | `8080` | Overridden via `.env` |
| **Production Target** | Google Cloud Run (`europe-west4`) | Serverless container deployment |

---

## 4. Current Work & Unresolved Backlog
1. **First Cloud Run Release**: Run container builds and deploy the API server to Cloud Run inside the `cislink-500-takeaway` GCP project.
2. **Cloud Relay Server Migration**: Migrate the desktop Docker `hbbs`/`hbbr` services to a 24/7 cloud VPS (such as Hetzner Cloud or Elestio) following the guidelines in [RELAY_SERVER_MIGRATION_PLAN.md](file:///d:/Rustdesk/RELAY_SERVER_MIGRATION_PLAN.md).
3. **Client Configuration Linking**: Configure repacked clients to point to the new Cloud Run API server URL and test logins with Firebase accounts.
