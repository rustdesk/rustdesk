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

    assert 'ProductName = "RustDesk-Herbin"' in cargo_toml
    assert 'OriginalFilename = "rustdesk-herbin.exe"' in cargo_toml
    assert 'name = "RustDesk-Herbin"' in cargo_toml
    assert 'identifier = "com.herbin.rustdesk"' in cargo_toml
    assert 'pub const FORK_APP_NAME: &str = "RustDesk-Herbin";' in common_rs
    assert "pub fn apply_fork_identity()" in common_rs
    assert "common::apply_fork_identity();" in main_rs
    assert "crate::common::apply_fork_identity();" in core_main_rs
    assert "crate::common::apply_fork_identity();" in flutter_rs
    assert "crate::common::apply_fork_identity();" in flutter_ffi_rs
    assert "crate::common::apply_fork_identity();" in service_rs

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
