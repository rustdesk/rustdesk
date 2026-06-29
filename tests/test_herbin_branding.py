#!/usr/bin/env python3
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = (
    Path.home()
    / ".config/superpowers/worktrees/rustdesk/codex-actions-master/.github/workflows/codex-windows-x64.yml"
)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> None:
    cargo_toml = read(ROOT / "Cargo.toml")
    common_rs = read(ROOT / "src/common.rs")
    main_rs = read(ROOT / "src/main.rs")
    core_main_rs = read(ROOT / "src/core_main.rs")
    flutter_rs = read(ROOT / "src/flutter.rs")
    flutter_ffi_rs = read(ROOT / "src/flutter_ffi.rs")
    service_rs = read(ROOT / "src/service.rs")
    keyboard_rs = read(ROOT / "src/keyboard.rs")
    input_service_rs = read(ROOT / "src/server/input_service.rs")
    flutter_pubspec = read(ROOT / "flutter/pubspec.yaml")
    mac_app_info = read(ROOT / "flutter/macos/Runner/Configs/AppInfo.xcconfig")
    mac_info_plist = read(ROOT / "flutter/macos/Runner/Info.plist")
    mac_project = read(ROOT / "flutter/macos/Runner.xcodeproj/project.pbxproj")
    mac_scheme = read(
        ROOT / "flutter/macos/Runner.xcodeproj/xcshareddata/xcschemes/Runner.xcscheme"
    )
    build_py = read(ROOT / "build.py")
    osx_dist = read(ROOT / "res/osx-dist.sh")
    flutter_build_workflow = read(ROOT / ".github/workflows/flutter-build.yml")
    playground_workflow = read(ROOT / ".github/workflows/playground.yml")
    codex_macos_workflow = read(ROOT / ".github/workflows/codex-macos-herbin.yml")

    assert 'ProductName = "RustDesk-Herbin"' in cargo_toml
    assert 'OriginalFilename = "rustdesk-herbin.exe"' in cargo_toml
    assert 'name = "RustDesk-Herbin"' in cargo_toml
    assert 'identifier = "com.herbin.rustdesk"' in cargo_toml
    assert 'pub const FORK_APP_NAME: &str = "RustDesk-Herbin";' in common_rs
    assert 'pub const FORK_ORG: &str = "com.herbin";' in common_rs
    assert "*org = FORK_ORG.to_owned();" in common_rs
    assert "pub fn apply_fork_identity()" in common_rs
    assert "common::apply_fork_identity();" in main_rs
    assert "crate::common::apply_fork_identity();" in core_main_rs
    assert "crate::common::apply_fork_identity();" in flutter_rs
    assert "crate::common::apply_fork_identity();" in flutter_ffi_rs
    assert "crate::common::apply_fork_identity();" in service_rs

    assert "ENABLE_WINDOWS_TO_MACOS_ALT_TAB_REMAP" not in keyboard_rs
    assert "remap_shortcut_for_peer" not in keyboard_rs

    assert "fn try_remap_mac_alt_tab" in input_service_rs
    assert "ck.value() == ControlKey::Tab.value()" in input_service_rs
    assert "mods.contains(&ControlKey::Alt.value())" in input_service_rs
    assert "mods.contains(&ControlKey::RAlt.value())" in input_service_rs
    assert "mods.contains(&ControlKey::Control.value())" in input_service_rs
    assert "mods.contains(&ControlKey::RControl.value())" in input_service_rs
    assert "mods.contains(&ControlKey::Meta.value())" in input_service_rs
    assert "mods.contains(&ControlKey::RWin.value())" in input_service_rs
    assert "mods.contains(&ControlKey::Shift.value())" in input_service_rs
    assert "mods.contains(&ControlKey::RShift.value())" in input_service_rs
    assert "en.key_up(Key::Alt);" in input_service_rs
    assert "en.key_up(Key::RightAlt);" in input_service_rs
    assert "en.add_flag(&Key::Meta);" in input_service_rs
    assert "en.key_down(Key::Tab).ok();" in input_service_rs
    assert "en.key_up(Key::Tab);" in input_service_rs
    assert "en.key_up(Key::Meta);" in input_service_rs

    assert "sdk: '^3.1.0'" in flutter_pubspec
    assert "s/3.1.0/2.17.0" not in playground_workflow
    for macos_workflow in [playground_workflow, codex_macos_workflow]:
        assert "Ad-hoc sign unsigned app" in macos_workflow
        assert 'codesign --force --deep --sign - --options runtime "$APP"' in macos_workflow
        assert 'codesign --verify --deep --strict --verbose=4 "$APP"' in macos_workflow

    assert "PRODUCT_NAME = RustDesk-Herbin" in mac_app_info
    assert "PRODUCT_BUNDLE_IDENTIFIER = com.herbin.rustdesk" in mac_app_info
    assert "<string>com.herbin.rustdesk</string>" in mac_info_plist
    assert "<string>rustdesk-herbin</string>" in mac_info_plist
    assert "RustDesk-Herbin.app" in mac_project
    assert "RustDesk.app" not in mac_project
    assert "PRODUCT_BUNDLE_IDENTIFIER = com.herbin.rustdesk;" in mac_project
    assert "PRODUCT_BUNDLE_IDENTIFIER = com.carriez.rustdesk;" not in mac_project
    assert 'BuildableName = "RustDesk-Herbin.app"' in mac_scheme
    assert 'BuildableName = "RustDesk.app"' not in mac_scheme

    for packaging_file in [
        build_py,
        osx_dist,
        flutter_build_workflow,
        playground_workflow,
        codex_macos_workflow,
    ]:
        assert "RustDesk-Herbin.app" in packaging_file
        assert "RustDesk.app" not in packaging_file
        assert "rustdesk-herbin-" in packaging_file

    if WORKFLOW.exists():
        workflow = read(WORKFLOW)
        assert 'APP_NAME: "RustDesk-Herbin"' in workflow
        assert 'ARTIFACT_PREFIX: "rustdesk-herbin"' in workflow
        assert "--app-name ${{ env.APP_NAME }}" in workflow
        assert "Copy-Item ./rustdesk/rustdesk.exe ./rustdesk/${{ env.APP_NAME }}.exe" in workflow
        assert "${{ env.ARTIFACT_PREFIX }}-${{ env.VERSION }}-x86_64.exe" in workflow
        assert "${{ env.ARTIFACT_PREFIX }}-${{ env.VERSION }}-x86_64.msi" in workflow


if __name__ == "__main__":
    main()
