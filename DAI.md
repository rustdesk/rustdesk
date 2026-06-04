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

## Safety Intercepts & Guidelines
- **UNC blocking**: Do not use UNC paths.
- **PowerShell filtering**: Ensure commands do not contain illegal pipes or syntax.
- **Staleness Guard**: Verify file changes do not overwrite concurrent work.
