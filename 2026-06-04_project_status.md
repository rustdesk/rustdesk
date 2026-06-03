# Project Status: RustDesk Customization & GCP Integration (2026-06-04)

This document tracks the overall status, progress, active configurations, and milestones for the custom RustDesk project as of June 4, 2026.

---

## 1. Executive Summary
The project aims to construct a custom remote desktop client (based on RustDesk) pre-configured with custom servers, coupled with a serverless GCP API backend that handles user authentication and client group management lists via Firebase Auth and Cloud Firestore.

---

## 2. Completed Milestones

### Client-Side Repacking
- Created standard Inno Setup installer scripts (`RustDesk-Installer.iss`, `RustDesk-Installer-Win7.iss`) to package preconfigured DLLs and configs.
- Created macOS repack automation scripts (`repack-macos-dmg.sh`).
- Implemented client config deployment utilities (`Deploy-RustDeskConfig.ps1`, `update-config.ps1`).

### Custom GCP API Backend Server
- Built a Node.js/TypeScript Express backend in [gcp-api-server](file:///D:/Rustdesk/gcp-api-server).
- Configured Express body parser and CORS support.
- Configured Firebase Admin SDK initialization to automatically handle Google Application Default Credentials (ADC) on Cloud Run, while allowing local service-account overrides.
- Implemented Firebase REST API login routing using standard `fetch` against Identity Toolkit endpoints.
- Implemented CRUD operations for Address Books, tags, and peers stored within Cloud Firestore.
- Implemented **dynamic group creation**: whenever a peer device reports a new `device_group_name`, a matching group document is automatically generated in the `device_groups` collection in Firestore.
- Added direct global users (`/api/users`) and peers (`/api/peers`) query endpoints matching RustDesk Flutter models.
- Containerized the server using a multi-stage `Dockerfile`.
- Completed compilation checks (`npm run build`) successfully with 0 TypeScript/compiler errors.

---

## 3. Active Configuration

| Config Item | Selected Option / Value | Notes |
| :--- | :--- | :--- |
| **Backend Runtime** | Node.js v22 (Express + TypeScript) | Compiles to ES2022 CommonJS |
| **API Code Directory** | `D:\Rustdesk\gcp-api-server` | Standalone sub-folder inside monorepo |
| **Auth Provider** | Firebase Auth (REST wrapper) | Validates email/password via REST API |
| **Database** | Cloud Firestore | Flexible, no-management NoSQL documents |
| **Group Registry Mode** | Dynamic Auto-Creation | Triggers when adding/updating devices |
| **Local Port** | `8080` | Overridden via `.env` |
| **Production Target** | Google Cloud Run (`europe-west4`) | Serverless container deployment |

---

## 4. Current Work & Unresolved Backlog
1. **Local Sandbox Testing**: Add dummy mock verification scripts to test API responses locally with a mock Firebase Auth endpoint if desired.
2. **First Cloud Run Release**: Run container builds and deploy to Cloud Run inside the `cislink-500-takeaway` GCP project.
3. **RustDesk Client Linking**: Configure repacked clients to point to the new Cloud Run API server URL and test logins with Firebase accounts.
