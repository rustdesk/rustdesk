import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
MACOS_SOURCE = (REPOSITORY_ROOT / "src/platform/macos.rs").read_text(
    encoding="utf-8"
)
DAEMON_SCRIPT = (
    REPOSITORY_ROOT / "src/platform/privileges_scripts/update.scpt"
).read_text(encoding="utf-8")
MANUAL_SCRIPT = MACOS_SOURCE.split(
    'const PRIVILEGED_UPDATE_BODY: &str = r#"', 1
)[1].split('"#;', 1)[0]


class MacosUpdateScriptTests(unittest.TestCase):
    def test_candidate_identity_and_os_signature_precede_install(self):
        for name, script in (
            ("daemon", DAEMON_SCRIPT),
            ("manual", MANUAL_SCRIPT),
        ):
            with self.subTest(script=name):
                validation = next(
                    line
                    for line in script.splitlines()
                    if "set validate_verified_app" in line
                )
                self.assertGreaterEqual(validation.count("CFBundleIdentifier"), 2)
                self.assertIn(
                    "/usr/bin/codesign --verify --deep --strict", validation
                )
                self.assertIn(
                    "/usr/sbin/spctl --assess --type execute", validation
                )
                self.assertIn(
                    "prepare_verified & validate_verified_app", script
                )
                shell = next(
                    line for line in script.splitlines() if "set sh to" in line
                )
                self.assertLess(
                    shell.index("validate_verified_app"),
                    shell.index("kill_others"),
                )
                if "copy_files" in shell:
                    self.assertLess(
                        shell.index("validate_verified_app"),
                        shell.index("copy_files"),
                    )
                else:
                    self.assertLess(
                        shell.index("validate_verified_app"),
                        shell.index('"transaction_started=1;"'),
                    )

    def test_root_update_validates_candidate_before_transaction(self):
        root_update = MACOS_SOURCE.split(
            "pub fn update_from_dmg_as_root", 1
        )[1]
        validator = MACOS_SOURCE.split(
            "fn verify_update_app_identity_and_signature", 1
        )[1].split("\nfn ", 1)[0]

        self.assertIn('Command::new("/usr/bin/codesign")', validator)
        self.assertIn('Command::new("/usr/sbin/spctl")', validator)
        self.assertGreaterEqual(validator.count("CFBundleIdentifier"), 2)
        self.assertLess(
            root_update.index("verify_update_app_identity_and_signature"),
            root_update.index("let staged_version_result"),
        )


if __name__ == "__main__":
    unittest.main()
