import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("generate_update_metadata.py")
WORKFLOW = (SCRIPT.parents[2] / ".github/workflows/flutter-build.yml").read_text(
    encoding="utf-8"
)


class UpdateMetadataCliTests(unittest.TestCase):
    def test_official_workflow_passes_explicit_update_policy(self):
        self.assertEqual(2, WORKFLOW.count("--package-id rustdesk"))
        self.assertEqual(
            2,
            WORKFLOW.count(
                "--release-base-url https://github.com/rustdesk/rustdesk/releases/download"
            ),
        )

    def test_sign_and_verify_require_update_identity_policy(self):
        commands = (
            [
                "sign",
                "--artifact", "macos", "aarch64", "dmg", "missing.dmg",
                "--version", "1.4.6",
                "--release-id", "v1.4.6",
                "--published-at", "invalid",
                "--metadata-out", "metadata.json",
                "--signature-out", "signature.json",
            ],
            [
                "verify",
                "--metadata", "missing.json",
                "--signature", "missing.sig",
                "--artifact", "missing.dmg",
                "--version", "1.4.6",
                "--release-id", "v1.4.6",
            ],
        )

        for command in commands:
            with self.subTest(command=command[0]):
                result = subprocess.run(
                    [sys.executable, str(SCRIPT), *command],
                    text=True,
                    capture_output=True,
                )
                self.assertIn("the following arguments are required", result.stderr)
                self.assertIn("--package-id", result.stderr)
                self.assertIn("--release-base-url", result.stderr)

    def test_sign_rejects_empty_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "rustdesk-1.4.6-aarch64.dmg"
            artifact.touch()
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "sign",
                    "--artifact", "macos", "aarch64", "dmg", str(artifact),
                    "--version", "1.4.6",
                    "--release-id", "v1.4.6",
                    "--package-id", "rustdesk",
                    "--release-base-url",
                    "https://github.com/rustdesk/rustdesk/releases/download",
                    "--published-at", "2026-05-14T00:00:00Z",
                    "--metadata-out", str(Path(directory) / "metadata.json"),
                    "--signature-out", str(Path(directory) / "metadata.json.sig"),
                ],
                env=os.environ.copy(),
                text=True,
                capture_output=True,
            )

        self.assertNotEqual(0, result.returncode)
        self.assertIn("artifact must not be empty", result.stderr)


if __name__ == "__main__":
    unittest.main()
