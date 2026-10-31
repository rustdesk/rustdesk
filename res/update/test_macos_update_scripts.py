import plistlib
import select
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


def write_bundle_id(app, bundle_id):
    if bundle_id is None:
        return
    info = app / "Contents/Info.plist"
    info.parent.mkdir(parents=True)
    info.write_bytes(plistlib.dumps({"CFBundleIdentifier": bundle_id}))


class MacosUpdateScriptTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "darwin", "requires macOS process handling")
    def test_relaunch_waits_for_old_backend_exit(self):
        process_timeout = 5
        script = MACOS_SOURCE.split('const RELAUNCH: &str = r#"', 1)[1].split('"#;', 1)[0]
        backend_code = (
            "import socket, sys; listener = socket.socket(); "
            "listener.bind(('127.0.0.1', 0)); listener.listen(1); "
            "print(listener.getsockname()[1], flush=True); sys.stdin.read(1)"
        )
        backend = subprocess.Popen(
            [sys.executable, "-c", backend_code],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
        )
        self.addCleanup(backend.communicate, timeout=process_timeout)
        self.addCleanup(backend.kill)
        self.assertTrue(select.select([backend.stdout], [], [], process_timeout)[0])
        port = int(backend.stdout.readline())
        probe = (
            '"$2" -c \'import socket, sys; listener = socket.socket(); '
            'listener.bind(("127.0.0.1", int(sys.argv[1])))\' "$3"'
        )
        self.assertIn('/usr/bin/open -n "$2"', script)
        script = script.replace('/usr/bin/open -n "$2"', probe)
        relaunch = subprocess.Popen(
            ["/bin/sh", "-c", script, "rustdesk-relaunch-test",
             str(backend.pid), sys.executable, str(port)],
        )
        self.addCleanup(relaunch.wait, timeout=process_timeout)
        self.addCleanup(relaunch.kill)
        with self.assertRaises(subprocess.TimeoutExpired):
            relaunch.wait(timeout=0.5)
        backend.communicate(input="\n", timeout=process_timeout)
        self.assertEqual(relaunch.wait(timeout=process_timeout), 0)

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

    @unittest.skipUnless(sys.platform == "darwin", "requires macOS AppleScript")
    def test_manual_update_requires_matching_bundle_identity(self):
        validation = next(
            line for line in MANUAL_SCRIPT.splitlines()
            if "set validate_verified_app" in line
        )
        script = "\n".join([
            "on run {app_bundle, expected_bundle_id}",
            "set app_bundle_q to quoted form of app_bundle",
            'set installed_info_q to quoted form of (app_bundle & "/Contents/Info.plist")',
            "set expected_bundle_id_q to quoted form of expected_bundle_id",
            validation,
            "return validate_verified_app",
            "end run",
        ])
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        bundle_id = "com.carriez.rustdesk"
        cases = (
            ("new", None, bundle_id, 0),
            ("installed", bundle_id, bundle_id, 0),
            ("wrong_candidate", None, "example.other", 1),
            ("wrong_destination", "example.other", bundle_id, 1),
        )
        for name, installed_id, candidate_id, exit_code in cases:
            with self.subTest(case=name):
                installed = root / name / "installed.app"
                candidate = root / name / "candidate.app"
                write_bundle_id(installed, installed_id)
                write_bundle_id(candidate, candidate_id)
                command = subprocess.check_output(
                    ["osascript", "-e", script, str(installed), bundle_id],
                    text=True, timeout=10,
                )
                result = subprocess.run(
                    ["/bin/sh", "-c", 'set -e; verified_app="$1"; ' + command,
                     "update-test", str(candidate)],
                    text=True, capture_output=True, timeout=10,
                )
                self.assertEqual(result.returncode, exit_code, result.stderr)


if __name__ == "__main__":
    unittest.main()
