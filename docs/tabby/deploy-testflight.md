# Tabby — TestFlight Deployment

How to ship a Tabby iOS build to TestFlight.

## Prerequisites (one-time)

- App Store Connect API key on disk at `~/.appstoreconnect/private_keys/AuthKey_G7S8Q6D6Z9.p8`. If missing, restore from 1Password (`MyDevSecrets/AppStoreConnect`, field `auth_key_b64`):
  ```bash
  mkdir -p ~/.appstoreconnect/private_keys
  op item get "AppStoreConnect" --vault "MyDevSecrets" --fields label=auth_key_b64 --reveal | tr -d '"' | base64 -d > ~/.appstoreconnect/private_keys/AuthKey_G7S8Q6D6Z9.p8
  ```
- System `flutter` (3.41.8). Do **not** use `fvm flutter` — incompatible with current deps.
- `flutter/ios/exportOptions.plist` exists and uses `method: app-store`, `teamID: GUW6BN8X57`.

## Identifiers

| Field | Value |
|---|---|
| Key ID | `G7S8Q6D6Z9` |
| Issuer ID | `e84dd3bd-1e5d-4db4-8b57-46637e2510ff` |
| Team ID | `GUW6BN8X57` |

## Steps

### 1. Bump build number

In `flutter/pubspec.yaml`, increment the part after `+`:
```yaml
version: 1.4.6+72   # was +71
```
TestFlight rejects duplicate build numbers within a `CFBundleShortVersionString`. Bump the build number on every upload, even for the same marketing version.

### 2. Build IPA

```bash
cd flutter
flutter pub get
flutter build ipa --release \
  --export-options-plist=ios/exportOptions.plist \
  --no-pub
```
Output: `flutter/build/ios/ipa/<name>.ipa` (~40 MB).

The build log will end with `✓ Built IPA to build/ios/ipa`. Treat the launch-image placeholder warning as known.

### 3. Upload to TestFlight

```bash
xcrun altool --upload-app --type ios \
  -f flutter/build/ios/ipa/*.ipa \
  --apiKey G7S8Q6D6Z9 \
  --apiIssuer e84dd3bd-1e5d-4db4-8b57-46637e2510ff
```
Successful upload prints `No errors uploading <name>.ipa`. Apple processing typically takes 5–15 minutes before the build is testable in TestFlight.

### 4. Verify

- App Store Connect → TestFlight → look for the new build under the bumped build number.
- Once "Ready to Test", install/update via the TestFlight app on the test device.

## Troubleshooting

- **"No suitable application records were found"** → bundle identifier mismatch; check `flutter/ios/Runner.xcodeproj` matches the App Store Connect record.
- **"Asset validation failed: Invalid Bundle"** → usually a missing entitlement or signing mismatch. Re-check `exportOptions.plist` and active provisioning profile.
- **`altool` hangs > 5 min** → flaky upload server; cancel and retry. Idempotent on the same build number until processing succeeds.
- **Build number already used** → bump `pubspec.yaml` again.
