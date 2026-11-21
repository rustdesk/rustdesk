import contextlib
import importlib.util
import io
from pathlib import Path
import shlex
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "rustdesk_build", Path(__file__).resolve().parents[1] / "build.py"
)
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class MacCaptureFeaturesTests(unittest.TestCase):
    def features(self, *flags):
        with patch.object(build, "osx", True), patch.object(build, "windows", False):
            args = build.make_parser().parse_args(["--flutter", *flags])
            with contextlib.redirect_stdout(io.StringIO()):
                return build.get_features(args)

    def test_screen_capture_kit_audio_is_opt_in(self):
        self.assertNotIn("screencapturekit", self.features())
        self.assertIn("screencapturekit", self.features("--screencapturekit"))

    def test_audio_keeps_legacy_deployment_target(self):
        for flags in [(), ("--screencapturekit",)]:
            with self.subTest(flags=flags):
                features = ",".join(self.features(*flags))
                with patch.object(build, "skip_cargo", False), \
                        patch.object(build, "system2") as run, \
                        patch.object(build.os, "chdir"):
                    build.build_flutter_dmg("test", features)
                cargo = next(call[0][0] for call in run.call_args_list
                             if "cargo build" in call[0][0])
                self.assertIn(features, shlex.split(cargo))
                self.assertIn("MACOSX_DEPLOYMENT_TARGET=10.14", shlex.split(cargo))
                flutter = next(call[0][0] for call in run.call_args_list
                               if "flutter build macos" in call[0][0])
                self.assertNotIn("FLUTTER_XCODE_MACOSX_DEPLOYMENT_TARGET=12.3",
                                 shlex.split(flutter))


if __name__ == "__main__":
    unittest.main()
