---
name: tabby-testflight
description: Use when shipping a Tabby iOS build to TestFlight — bumps the build number, runs `flutter build ipa`, uploads the IPA via `xcrun altool` with the App Store Connect API key. Triggers on "ship to TestFlight", "deploy Tabby", "release a Tabby build", "upload IPA".
---

# Tabby TestFlight Deployment

End-to-end TestFlight ship for the Tabby iOS app at `~/Desktop/dev/apps/ios/Tabby`.

Authoritative source for the steps: `docs/tabby/deploy-testflight.md` in the repo. Read it before deviating.

## When to Use

- User asks to "ship to TestFlight", "deploy Tabby", "upload a build", or similar.
- Only invoke for the Tabby repo (`Cargo.toml` name is `rustdesk`, but the working dir name and `pubspec.yaml` `name: flutter_hbb` plus a `flutter/ios/exportOptions.plist` with `teamID: GUW6BN8X57` confirm it).

## Preconditions

1. Working tree is clean OR the changes you intend to ship are already committed. If dirty, ask the user before continuing — uncommitted code that ships is a debugging hazard.
2. API key file exists at `~/.appstoreconnect/private_keys/AuthKey_G7S8Q6D6Z9.p8`. Recover from 1Password if missing (see repo doc).
3. System `flutter` is 3.41.8. **Never** use `fvm flutter` for Tabby builds.

## Identifiers

- Key ID: `G7S8Q6D6Z9`
- Issuer ID: `e84dd3bd-1e5d-4db4-8b57-46637e2510ff`
- Team ID: `GUW6BN8X57`

## Steps

### 1. Bump build number

Edit `flutter/pubspec.yaml`. Find the `version: 1.x.y+N` line and increment `N`. TestFlight rejects duplicate build numbers within a marketing version, so bump even for retries of the same `1.x.y`.

Commit the bump (and any other intended changes) — get explicit approval per the user's commit policy before running `git commit`.

### 2. Build IPA

From the repo root:

```bash
cd flutter
flutter pub get 2>&1 | tail -3
flutter build ipa --release \
  --export-options-plist=ios/exportOptions.plist \
  --no-pub 2>&1 | tail -12
```

Watch for `✓ Built IPA to build/ios/ipa`. Ignore the "launch image is set to the default placeholder" warning — it's pre-existing.

### 3. Upload

```bash
xcrun altool --upload-app --type ios \
  -f flutter/build/ios/ipa/*.ipa \
  --apiKey G7S8Q6D6Z9 \
  --apiIssuer e84dd3bd-1e5d-4db4-8b57-46637e2510ff
```

Successful end-state: `No errors uploading '<name>.ipa'`. Run in foreground; it can take 1–5 minutes.

### 4. Hand off to user

Tell the user:
- Bumped build number (e.g. `+72`)
- IPA path
- That Apple processing takes 5–15 minutes before the build appears as "Ready to Test"

## Anti-patterns

- **Don't** retry the upload with the same build number — bump first.
- **Don't** skip the build before re-uploading; an old IPA in `build/ios/ipa/` will silently re-upload (`*.ipa` glob).
- **Don't** add `--verbose` to `altool` unless debugging; it's very chatty.
- **Don't** commit the API key, ever.
