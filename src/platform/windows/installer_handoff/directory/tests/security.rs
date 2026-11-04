use super::*;
use crate::platform::windows::installer_files::ProtectedInstallFile;
use std::{os::windows::process::CommandExt, process::Command};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

#[test]
fn installer_path_uses_windows_temp_unless_redirected() {
    let system = get_system_executable("").unwrap();
    let temporary = system.parent().unwrap().join("Temp");
    let expected = if has_reparse_parent(&temporary).unwrap() {
        get_known_folder(&FOLDERID_ProgramData).unwrap()
    } else {
        temporary
    };
    assert_eq!(path().unwrap().parent(), Some(expected.as_path()));
}

#[test]
fn junction_installer_parent_uses_program_data_for_staging() {
    let root = std::env::temp_dir().join(format!("rustdesk-junction-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let link = root.join("redirected");
    let mut results = Vec::new();
    for target in [
        &root,
        &get_system_executable("").unwrap(),
        &root.join("missing"),
    ] {
        let output = Command::new(get_system_executable("cmd.exe").unwrap())
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(target)
            .creation_flags(CREATE_NO_WINDOW.0)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        results.push(temporary_parent(&link));
        fs::remove_dir(&link).unwrap();
    }
    fs::remove_dir(root).unwrap();
    for result in results {
        let parent = result.unwrap();
        assert_eq!(parent, get_known_folder(&FOLDERID_ProgramData).unwrap());
        let path = parent.join(format!("rustdesk-fallback-{}.tmp", uuid::Uuid::new_v4()));
        let staged = ProtectedInstallFile::new(&path, b"installer").unwrap();
        let contents = fs::read(staged.path()).unwrap();
        drop(staged);
        assert_eq!(contents, b"installer");
        assert!(!path.exists());
    }
}
