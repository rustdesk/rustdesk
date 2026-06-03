# RustDesk Customizations & GCP API Server Knowledge Base

This documentation serves as the master knowledge base for custom configurations, client repacking scripts, and the custom GCP API Server developed for the RustDesk monorepo.

---

## 1. Client Repacking & Packaging Configs

The repository contains scripts and templates to package the RustDesk desktop client with custom pre-configured server addresses and public encryption keys.

### Core PowerShell / Batch Scripts
- `Build-RustDesk-Installer.ps1`: Builds custom installer configurations using Inno Setup.
- `Deploy-RustDeskConfig.ps1`: Deploys target server and key configurations to standard local configuration paths (e.g., `AppData\Roaming\RustDesk\config\`).
- `repack-macos-dmg.sh`: Automates packaging of custom macOS DMG files with preconfigured settings.

### Configuration Template Structure
Custom configuration parameters are stored in `.toml` configurations (e.g., `RustDesk_Config_Template.toml`):
```toml
[common]
id-server = "your-hbbs-server.domain"
relay-server = "your-hbbr-server.domain"
key = "your-server-public-key"
api-server = "https://your-custom-gcp-api.run.app"
```

---

## 2. GCP RustDesk API Server Architecture

To replace the proprietary RustDesk Server Pro backend, we implemented a custom API server in [gcp-api-server](file:///D:/Rustdesk/gcp-api-server) that routes standard client HTTP calls to Firebase Authentication and Cloud Firestore.

### Technology Stack
- **Runtime**: Node.js v22 (Alpine) with TypeScript and Express.js.
- **Identity Provider**: Firebase Authentication (validating passwords via Google Identity Toolkit REST API and issuing JWT ID Tokens).
- **Database**: Cloud Firestore (for peers, address books, user access, and device group structures).
- **Deployment**: Google Cloud Run (containerized serverless environment).

---

## 3. Firestore Database Schema Design

### `users` Collection
Stores user metadata and authorization details.
- **Document ID**: `uid` (Firebase Authentication local ID)
- **Fields**:
  - `name`: String (Display name)
  - `email`: String (User email)
  - `note`: String (Admin descriptions)
  - `status`: Number (`1` = normal active, `0` = disabled)
  - `is_admin`: Boolean (Grants global visibility to all address books/groups)
  - `created_at`: Timestamp

### `address_books` Collection
Defines a specific instance of a device collection.
- **Document ID**: `guid` (Auto-generated UUID, e.g. `ab_12345`)
- **Fields**:
  - `guid`: String (GUID)
  - `name`: String (e.g., "Personal")
  - `owner`: String (Associated owner's UID)
  - `rule`: Number (Access level: `3` = full control, `2` = read/write, `1` = read)
  - `tags`: Array of Objects `[ { "name": "tag1", "color": 4287123976 } ]`
  - `created_at`: Timestamp

### `peers` Collection (Device Lists)
Houses records of individual client computers mapped to an address book.
- **Document ID**: `${ab_guid}_${device_id}` (Scoped unique path)
- **Fields**:
  - `id`: String (RustDesk Client ID, e.g., `"123456789"`)
  - `ab`: String (Associated address book GUID)
  - `alias`: String (Custom display nickname)
  - `hostname`: String (Remote device hostname)
  - `username`: String (Remote system user name)
  - `platform`: String (e.g., `windows`, `linux`, `macos`, `android`)
  - `password`: String (Stored password for direct connection)
  - `hash`: String (Hashed verification key)
  - `tags`: Array of Strings `["POS", "Groningen"]`
  - `device_group_name`: String (Assigned group list name)
  - `loginName`: String (Owner user email)
  - `same_server`: Boolean

### `device_groups` Collection (Client Group Lists)
Manages grouped clients. Auto-bootstrapped when a peer includes a new `device_group_name`.
- **Document ID**: `group_slug` (e.g., `sushi-koi-groningen`)
- **Fields**:
  - `name`: String (Display name, e.g., `"Sushi Koi Groningen"`)
  - `owner`: String (Owner's UID)
  - `accessible_users`: Array of Strings (UIDs of users allowed to access this group)
  - `created_at`: Timestamp

---

## 4. RustDesk Client API Endpoints Implementation

The server maps HTTP requests expected by the RustDesk Flutter client:

- **Authentication**:
  - `POST /api/login`: Validates credentials via Identity Toolkit and returns the Firebase token + user payload.
  - `POST /api/logout`: Handles session exit.
  - `POST /api/currentUser`: Returns user profile stats.
  - `GET /api/login-options`: Disables OIDC buttons (returns `[]`) to default to email/password forms.

- **Address Book (ab) & Tags**:
  - `GET /api/ab/settings`: Returns book size configuration (`max_peer_one_ab: 1000`).
  - `POST /api/ab/personal`: Initializes or retrieves the user's personal address book GUID.
  - `POST /api/ab/peers`: Paginated retrieval of device records inside an address book.
  - `POST /api/ab/peer/add/:guid`: Inserts a device and registers its group dynamically.
  - `PUT /api/ab/peer/update/:guid`: Modifies device details.
  - `DELETE /api/ab/peer/:guid`: Removes multiple device IDs from the book.
  - `POST /api/ab/tags/:guid` / `POST /api/ab/tag/add/:guid` / `PUT /api/ab/tag/rename/:guid` / `DELETE /api/ab/tag/:guid`: CRUD tag settings inside the Address Book config.

- **Groups & Lists**:
  - `GET /api/device-group/accessible`: Returns a paginated group list matching the user's permissions.
  - `GET /api/users`: Returns all users (for Admins) or the user profile (for standard users).
  - `GET /api/peers`: Lists all devices across all address books.
