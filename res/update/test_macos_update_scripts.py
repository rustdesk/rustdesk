import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
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
    @unittest.skipUnless(sys.platform == "darwin", "requires macOS AppleScript")
    def test_daemon_update_rejects_stale_ipc_sockets(self):
        with tempfile.TemporaryDirectory(dir="/tmp") as directory:
            for check in ("check_service", "check_agent"):
                with self.subTest(check=check), socket.socket(
                    socket.AF_UNIX, socket.SOCK_STREAM
                ) as listener:
                    path = str(Path(directory) / check)
                    listener.bind(path)
                    listener.listen(1)
                    script = DAEMON_SCRIPT.split("  set sh to", 1)[0]
                    script += f"  return {check}\nend run\n"
                    script = script.replace("/tmp/RustDesk-service/ipc_service", path)
                    script = script.replace("/tmp/RustDesk-$uid/ipc", path)
                    command = subprocess.check_output(
                        ["osascript", "-e", script, "test", "0", "", ""],
                        text=True,
                        timeout=10,
                    )
                    command = "launchctl() { echo 'state = running'; }; " + command
                    args = ["/bin/sh", "-c", command]
                    self.assertEqual(subprocess.call(args, timeout=10), 0)
                    listener.setblocking(False)
                    with self.assertRaises(BlockingIOError):
                        connection, _ = listener.accept()
                        connection.close()
                    listener.close()
                    self.assertNotEqual(subprocess.call(args, timeout=10), 0)

    def test_candidate_identity_precedes_install_without_os_signature_checks(self):
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
                self.assertNotIn("/usr/bin/codesign", validation)
                self.assertNotIn("/usr/sbin/spctl", validation)
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

    def test_root_update_validates_candidate_identity_before_transaction(self):
        root_update = MACOS_SOURCE.split(
            "pub fn update_from_dmg_as_root", 1
        )[1]
        validator = MACOS_SOURCE.split(
            "fn verify_update_app_identity", 1
        )[1].split("\nfn ", 1)[0]

        self.assertNotIn('Command::new("/usr/bin/codesign")', validator)
        self.assertNotIn('Command::new("/usr/sbin/spctl")', validator)
        self.assertGreaterEqual(validator.count("CFBundleIdentifier"), 2)
        self.assertLess(
            root_update.index("verify_update_app_identity"),
            root_update.index("let staged_version_result"),
        )


if __name__ == "__main__":
    unittest.main()
