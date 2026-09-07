import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_TEXT = (REPOSITORY_ROOT / ".github/workflows/flutter-build.yml").read_text(
    encoding="utf-8"
)


def step_body(step_name):
    start = WORKFLOW_TEXT.index(f"      - name: {step_name}")
    remainder = WORKFLOW_TEXT[start + 1 :]
    next_step = remainder.find("\n      - name:")
    return WORKFLOW_TEXT[start:] if next_step < 0 else WORKFLOW_TEXT[start : start + next_step + 1]


class FlutterBuildWorkflowTests(unittest.TestCase):
    def test_update_validation_job_runs_all_update_tests(self):
        test_step = step_body("Test update metadata signer")

        for test_file in (
            "res/update/test_generate_update_metadata.py",
            "res/update/test_update_metadata_cli.py",
            "res/update/test_flutter_build_workflow.py",
            "res/update/test_macos_update_scripts.py",
        ):
            self.assertIn(test_file, test_step)

    def test_published_metadata_is_never_removed_or_replaced(self):
        publish_job = WORKFLOW_TEXT.split("  publish-signed-update-metadata:", 1)[1].split(
            "\n  publish_unsigned:", 1
        )[0]
        guard_step = step_body("Refuse to replace published update metadata")

        self.assertIn(
            "Signed update metadata is already published",
            guard_step,
        )
        self.assertIn("exit 1", guard_step)
        self.assertNotIn("--method DELETE", publish_job)

    def test_macos_update_tests_use_matrix_features(self):
        test_step = step_body("Test verified updates")

        self.assertIn("${{ matrix.job.extra-cargo-features }}", test_step)
        self.assertNotIn(
            "--features flutter,hwcodec,unix-file-copy-paste,screencapturekit",
            test_step,
        )


if __name__ == "__main__":
    unittest.main()
