#!/usr/bin/env python3
import plistlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def read(relative_path: str) -> str:
    return (ROOT / relative_path).read_text(encoding="utf-8")


def assert_in_order(text: str, first: str, second: str) -> None:
    first_index = text.index(first)
    second_index = text.index(second)
    assert first_index < second_index, f"expected {first!r} before {second!r}"


def main() -> None:
    cargo_toml = read("Cargo.toml")
    common_rs = read("src/common.rs")
    main_rs = read("src/main.rs")
    core_main_rs = read("src/core_main.rs")
    flutter_rs = read("src/flutter.rs")
    flutter_ffi_rs = read("src/flutter_ffi.rs")
    service_rs = read("src/service.rs")
    keyboard_rs = read("src/keyboard.rs")
    input_service_rs = read("src/server/input_service.rs")
    server_rs = read("src/server.rs")
    memory_watchdog_rs = read("src/server/memory_watchdog.rs")
    mac_agent_plist = plistlib.loads(
        (ROOT / "src/platform/privileges_scripts/agent.plist").read_bytes()
    )
    macos_rs = read("src/platform/macos.rs")
    macos_mm = read("src/platform/macos.mm")
    mac_install_script = read("src/platform/privileges_scripts/install.scpt")
    mac_update_script = read("src/platform/privileges_scripts/update.scpt")
    mac_app_info = read("flutter/macos/Runner/Configs/AppInfo.xcconfig")
    mac_info_plist = read("flutter/macos/Runner/Info.plist")
    mac_project = read("flutter/macos/Runner.xcodeproj/project.pbxproj")
    mac_scheme = read(
        "flutter/macos/Runner.xcodeproj/xcshareddata/xcschemes/Runner.xcscheme"
    )
    build_py = read("build.py")
    osx_dist = read("res/osx-dist.sh")
    macos_workflow = read(".github/workflows/codex-macos-herbin.yml")

    assert 'ProductName = "RustDesk-Herbin"' in cargo_toml
    assert 'OriginalFilename = "rustdesk-herbin.exe"' in cargo_toml
    assert 'name = "RustDesk-Herbin"' in cargo_toml
    assert 'identifier = "com.herbin.rustdesk"' in cargo_toml
    assert 'pub const FORK_APP_NAME: &str = "RustDesk-Herbin";' in common_rs
    assert 'pub const FORK_ORG: &str = "com.herbin";' in common_rs
    assert "pub fn apply_fork_identity()" in common_rs
    assert "common::apply_fork_identity();" in main_rs
    assert "crate::common::apply_fork_identity();" in core_main_rs
    assert "crate::common::apply_fork_identity();" in flutter_rs
    assert "crate::common::apply_fork_identity();" in flutter_ffi_rs
    assert "crate::common::apply_fork_identity();" in service_rs

    assert "if is_custom_client() {\n        return;\n    }" in common_rs

    removed_keymap_markers = (
        "HERBIN_MACOS_KEYMAP",
        "HerbinMacosKeymap",
        "MacosShortcutRemapState",
        "try_remap_macos_shortcut",
        "herbin-keymap.json",
        "herbin-macos-keymap",
        "remap_shortcut_for_peer",
    )
    for marker in removed_keymap_markers:
        assert marker not in input_service_rs
        assert marker not in keyboard_rs

    assert "fn MacActivateApplicationAtPoint" in macos_rs
    assert "pub fn activate_application_at_point" in macos_rs
    assert 'bundleIdentifier isEqualToString:@"com.apple.dock"' in macos_mm
    assert "NSApplicationActivationPolicyRegular" in macos_mm
    assert "activateWithOptions" in macos_mm
    assert "activate_application_at_point(x, y)" in input_service_rs
    assert_in_order(
        input_service_rs,
        "activate_application_at_point(x, y)",
        "en.mouse_down(MouseButton::Left)",
    )

    assert 'mod memory_watchdog;' in server_rs
    assert 'memory_watchdog::start();' in server_rs
    assert '"rdh-memory-restart-threshold-mib"' in memory_watchdog_rs
    assert 'std::env::var("XPC_SERVICE_NAME")' in memory_watchdog_rs
    assert "DAILY_CHECK_HOUR: u32 = 6" in memory_watchdog_rs
    assert "UNATTENDED_WINDOW_START_HOUR: u32 = 0" in memory_watchdog_rs
    assert "UNATTENDED_WINDOW_END_HOUR: u32 = 7" in memory_watchdog_rs
    assert "Connection::alive_conns" not in memory_watchdog_rs
    assert "std::process::exit(RESTART_EXIT_CODE)" in memory_watchdog_rs
    assert mac_agent_plist["RunAtLoad"] is True
    assert mac_agent_plist["KeepAlive"] is True

    diagnostic_markers = (
        "macos-input-trace",
        "macos-focus-trace",
        "log_macos_click_focus",
        "mouse_focus_snapshot",
        "from_millis(120)",
    )
    diagnostic_sources = input_service_rs + macos_rs + macos_mm + read(
        "libs/enigo/src/macos/macos_impl.rs"
    )
    for marker in diagnostic_markers:
        assert marker not in diagnostic_sources

    assert "PRODUCT_NAME = RustDesk-Herbin" in mac_app_info
    assert "PRODUCT_BUNDLE_IDENTIFIER = com.herbin.rustdesk" in mac_app_info
    assert "<string>com.herbin.rustdesk</string>" in mac_info_plist
    assert "<string>rustdesk-herbin</string>" in mac_info_plist
    assert "RustDesk-Herbin.app" in mac_project
    assert "PRODUCT_BUNDLE_IDENTIFIER = com.herbin.rustdesk;" in mac_project
    assert 'BuildableName = "RustDesk-Herbin.app"' in mac_scheme

    assert 'launchctl bootstrap gui/$uid ' in mac_install_script
    assert 'launchctl kickstart -k gui/$uid/$agent_label' in mac_install_script
    assert "legacy_agent_plist" in mac_install_script
    assert "bad_agent_plist" in mac_install_script
    assert "legacy_daemon_plist" in mac_update_script
    assert "bad_daemon_plist" in mac_update_script

    for packaging_file in (build_py, osx_dist, macos_workflow):
        assert "RustDesk-Herbin.app" in packaging_file
        assert "rustdesk-herbin-" in packaging_file

    assert "Ad-hoc sign app (not notarized)" in macos_workflow
    assert "tests/test_herbin_branding.py" in macos_workflow
    assert "source_ref:" in macos_workflow
    assert "ref: ${{ inputs.source_ref }}" in macos_workflow
    assert "RDH_REVISION" in macos_workflow
    assert "shasum -a 256" in macos_workflow


if __name__ == "__main__":
    main()
