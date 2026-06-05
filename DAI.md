# DAI Operational Log

## Active Task
- [x] Build and initialize Node.js/TypeScript Express server for RustDesk API in `D:\Rustdesk\gcp-api-server`
- [x] Implement Firebase Auth controller (`/api/login`, `/api/logout`, `/api/currentUser`, `/api/login-options`)
- [x] Implement Address Book controller (`/api/ab/settings`, `/api/ab/personal`, `/api/ab/peers`, and peer CRUD)
- [x] Implement Device Group controller (`/api/device-group/accessible`)
- [x] Create Dockerfile and local environment configuration
- [x] Document local run & GCP Cloud Run deployment steps
- [x] Implement Shared Address Book profiles controller endpoint (`/api/ab/shared/profiles`) with automatic bootstrapping of "Cislink Team" book, and secure permissions for CRUD operations.
- [x] Clean up obsolete test scripts, configuration files, key formatting logs, backup folders, and reserved system files.
- [x] Backup database file (db_v2.sqlite3) and key files (PUBLIC_KEY.txt, BACKUP_INFO.txt, cislink.ppk, server_public_key.txt) to Google Drive (smartthink-drive:RustDesk_Backup) using rclone.
- [x] Resolve hbbs Docker container key mismatch issue by copying the correct key pair from D:\RustDesk-Server\data\ to D:\Rustdesk\data\ and restarting the hbbs container.
- [x] Upload active server key pair (id_ed25519 and id_ed25519.pub) to Google Drive (smartthink-drive:RustDesk_Backup) using rclone.
- [x] Create a safety warning note in Google NotebookLM (a0831bed-56db-4f1a-8e74-c739498bd1e1) detailing hbbs key mismatch and prevention rules.
- [x] Customize RustDesk auto-update mechanism to use download.cislink.nl/rustdesk/latest.json and rebrand update cards

## Safety Intercepts & Guidelines
- **UNC blocking**: Do not use UNC paths.
- **PowerShell filtering**: Ensure commands do not contain illegal pipes or syntax.
- **Staleness Guard**: Verify file changes do not overwrite concurrent work.
