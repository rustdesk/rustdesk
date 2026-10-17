# RustDesk MultiSession

An unofficial fork of [RustDesk](https://github.com/rustdesk/rustdesk) that adds **server profiles** (switch between ID/relay servers without retyping them) and ships its own **signed Windows releases** with in-app updates. It is not affiliated with the RustDesk project.

Everything else is stock RustDesk. The fork is kept small and merges upstream regularly.

## What's different

### Server profiles

Save the ID server, relay server, API server and key as a named profile, then switch with one click.

- **Settings > Network > Server profiles:** save the current settings, apply, edit (including the name) or delete a profile. The profile that matches your current settings is greyed out.
- **Connect page:** a profile picker above the ID field. Picking a profile switches the whole app over, so every later connection uses it.
- **Active profile:** it is whichever saved profile equals your current network settings, however they were set.
- Switching applies all four values in one step. A profile with an empty relay server really clears the relay server.
- The UI is translated into most languages the app supports (see `src/lang_fork.rs`). Others show English.

A rough command-line version also exists: `rustdesk.exe --profile list | show <name> | save <name> | use <name> | delete <name>`. It keeps its own list in `profiles.json` next to the config, which is **not shared** with the profiles in the UI.

### Updates from this repo

Release builds check [this repo's releases](../../releases) instead of RustDesk's. An installed copy shows a card when a newer release exists, and the **Update** button downloads, verifies and installs it, like stock RustDesk does.

- Releases are signed with this project's own key. A build trusts only that key, so it will not install RustDesk's official builds or anything unsigned.
- A portable copy gets a **Download** button that opens the release page instead.
- Local and untagged builds keep the stock behaviour.

## Downloads

Get them from the [Releases](../../releases) page.

| File | Use |
| --- | --- |
| `rustdesk-<version>-install.exe` | Installer. Run it and it installs. |
| `rustdesk-<version>-x86_64.exe` | Same program without the installer name. Runs portable, with an Install button inside. This is the file the updater uses. |
| `rustdesk-<version>-windows-x86_64.zip` | The unpacked app folder. |
| `rustdesk-update.json` and `.sig` | Signed update metadata, used by the app. |

Notes:

- Windows 64-bit only.
- The builds are not code-signed, so Windows SmartScreen may warn on first run.
- Builds include hardware codecs (NVIDIA, AMD, Intel). The app falls back to software encoding when the hardware is not usable.
- Installing over an existing RustDesk keeps your settings, ID, favourites and profiles.
- If the app closes right after starting, check for a leftover RustDesk process (Task Manager), or restart the PC.

## Versioning

The release tag is the app version: `v1.5.2`, exactly three numbers. The build sets the app's version to the tag.

- An update is offered only when the latest release's number is **higher** than the installed one.
- Keep the number at or above the upstream version you are based on. Other clients switch features on and off by version, so a lower number would lose features.
- Tags like `v1.5.2-1` are rejected by the update check.

## For maintainers

### Releasing

1. One-time: add a repository secret `UPDATE_SIGNING_SEED`, the base64 of a random 32-byte Ed25519 seed. Keep a backup. If it is lost, installed copies refuse updates signed with a new key.
2. Push a tag: `git tag v1.5.3 && git push origin v1.5.3`.
3. The **Release Windows portable** workflow builds, signs the update metadata and publishes the release (about an hour on the first run).

Running that workflow by hand builds without publishing and uploads the files as an artifact, which is useful as a dry run.

### Upstream sync

The **Sync upstream** workflow runs nightly. It merges `rustdesk/rustdesk` into an `upstream-sync` branch and opens a pull request into `master`. If the merge conflicts it opens an issue instead and pushes nothing. `master` is never changed by it. Build and test the branch (run the release workflow by hand on it) before merging. The workflow skips while an unmerged `upstream-sync` branch exists, so merge or delete it first.

Upstream's own workflows cannot run in this repo (they need RustDesk's secrets), so disable them in the Actions tab: Full Flutter CI, Flutter Tag Build, Flutter Nightly Build, Fdroid version file generation and Update webpki-roots. Keep CI, Release Windows portable and Sync upstream.

### Where the changes live

The fork's code is kept in separate files with thin hooks, so upstream merges stay easy:

- `src/profiles.rs`: the `--profile` command.
- `flutter/lib/common/widgets/server_profiles.dart`: the profile dialogs and the connect-page picker.
- `src/fork_update.rs`: update source and trusted key. It is only active when the release workflow sets `FORK_UPDATE_REPO` and `FORK_UPDATE_PUBLIC_KEY`. Small changes in `src/common.rs`, `src/updater/artifact.rs` and `src/update_metadata.rs` read them and otherwise use RustDesk's stock values.
- `src/lang_fork.rs`: translations for the fork's strings.
- `.github/workflows/release-windows.yml` and `sync-upstream.yml`.

### Building locally

Follow the [upstream build guide](https://rustdesk.com/docs/en/dev/build/) for Windows (Rust 1.75, LLVM 15, Flutter 3.24.5, vcpkg). The bridge files must be generated with `flutter_rust_bridge_codegen` 1.80.1 first. The release workflow is the reference for the exact steps and versions.

## License

AGPL-3.0, the same as RustDesk. See [LICENCE](../LICENCE). The original project is at [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk).
