# White-label builds

`tools/white_label.py` applies build-time branding to the Flutter clients and
generates the Rust settings used by desktop and mobile builds.

1. Copy `white_label.example.json` to a private file outside version control.
2. Add square PNG source icons and update the JSON paths.
3. Run `python tools/white_label.py path/to/your-white-label.json --generate-icons`.
4. Build each target using the normal RustDesk build workflow.

Use `python tools/white_label.py path/to/your-white-label.json --check` to
validate a config without modifying the source tree.

The script configures application metadata, platform IDs, URL schemes, launcher
icons, an optional SVG `ui_logo`, default or forced server settings, and an
optional update-check endpoint. For the legacy macOS runner, provide an optional
`mac_icns` path to replace `flutter/macos/Runner/AppIcon.icns`.

The update-check endpoint must accept the same POST request and return the same
JSON response shape as the RustDesk version endpoint. For Windows, placeholders
available in `windows_download_url_template` are `{base_url}`, `{version}` and
`{ext}`. A typical release file is `acmedesk-1.2.3-x86_64.exe`.

Signing remains a platform release step: use your own Windows code-signing
certificate, Apple Developer identity and Android keystore. Android and Apple
updates also require the same application ID or bundle ID and signing identity
as the previously installed release.

The generated Rust file is committed with empty defaults so an upstream merge
does not silently create a branded build. Re-run the script after rebasing onto
a new RustDesk version.
