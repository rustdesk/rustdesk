import importlib.util
import os
import pathlib
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('rustdesk_build', ROOT / 'build.py')
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class BuildArchitectureTest(unittest.TestCase):
    def test_flutter_linux_arch_aliases(self):
        self.assertEqual(BUILD.get_flutter_linux_arch('loongarch64'), 'loong64')
        self.assertEqual(BUILD.get_flutter_linux_arch('loong64'), 'loong64')
        self.assertEqual(BUILD.get_flutter_linux_arch('aarch64'), 'arm64')
        self.assertEqual(BUILD.get_flutter_linux_arch('x86_64'), 'x64')
        self.assertEqual(BUILD.get_flutter_linux_arch('amd64'), 'x64')

    def test_flutter_linux_arch_rejects_unknown_machine(self):
        with self.assertRaisesRegex(RuntimeError, 'unsupported machine'):
            BUILD.get_flutter_linux_arch('riscv64')

    def test_loongarch_flutter_build_target(self):
        self.assertEqual(
            BUILD.get_flutter_linux_build_command('loongarch64'),
            'flutter build linux --release --target-platform linux-loong64',
        )
        self.assertEqual(
            BUILD.get_flutter_linux_build_command('loong64'),
            'flutter build linux --release --target-platform linux-loong64',
        )
        self.assertEqual(
            BUILD.get_flutter_linux_build_command('x86_64'),
            'flutter build linux --release',
        )
        self.assertEqual(
            BUILD.get_flutter_linux_build_command('aarch64'),
            'flutter build linux --release',
        )

    def test_flutter_linux_build_directories(self):
        self.assertEqual(
            BUILD.get_flutter_linux_build_dirs('loongarch64'),
            (
                'build/linux/loong64/release/bundle/',
                'flutter/build/linux/loong64/release/bundle/',
            ),
        )
        self.assertEqual(
            BUILD.get_flutter_linux_build_dirs('loong64'),
            (
                'build/linux/loong64/release/bundle/',
                'flutter/build/linux/loong64/release/bundle/',
            ),
        )
        self.assertEqual(
            BUILD.get_flutter_linux_build_dirs('aarch64'),
            (
                'build/linux/arm64/release/bundle/',
                'flutter/build/linux/arm64/release/bundle/',
            ),
        )
        self.assertEqual(
            BUILD.get_flutter_linux_build_dirs('x86_64'),
            (
                'build/linux/x64/release/bundle/',
                'flutter/build/linux/x64/release/bundle/',
            ),
        )

    def test_arm32_can_import_non_flutter_build(self):
        spec = importlib.util.spec_from_file_location(
            'rustdesk_build_arm32', ROOT / 'build.py')
        module = importlib.util.module_from_spec(spec)
        with mock.patch('platform.machine', return_value='armv7l'):
            spec.loader.exec_module(module)
        self.assertIsNone(module.flutter_build_dir)
        self.assertIsNone(module.flutter_build_dir_2)
        with self.assertRaisesRegex(RuntimeError, 'unsupported machine'):
            module.get_flutter_linux_build_dirs('armv7l')

    def test_debian_architecture_override(self):
        with mock.patch.dict(os.environ, {'DEB_ARCH': 'loong64'}, clear=True):
            with mock.patch.object(BUILD.subprocess, 'check_output') as check:
                self.assertEqual(BUILD.get_deb_arch(), 'loong64')
                check.assert_not_called()

    def test_debian_architecture_from_dpkg(self):
        for deb_arch in ('loongarch64', 'loong64'):
            with self.subTest(deb_arch=deb_arch):
                with mock.patch.dict(os.environ, {}, clear=True):
                    with mock.patch.object(
                        BUILD.subprocess,
                        'check_output',
                        return_value=(deb_arch + '\n').encode(),
                    ):
                        self.assertEqual(BUILD.get_deb_arch(), deb_arch)

    def test_debian_architecture_fallback(self):
        self.assertEqual(BUILD.deb_arch_from_machine('aarch64'), 'arm64')
        self.assertEqual(BUILD.deb_arch_from_machine('amd64'), 'amd64')
        self.assertEqual(BUILD.deb_arch_from_machine('armv7l'), 'armhf')
        self.assertEqual(BUILD.deb_arch_from_machine('x86_64'), 'amd64')

    def test_debian_architecture_fallback_does_not_guess_loongarch_abi(self):
        for machine in ('loongarch64', 'loong64'):
            with self.subTest(machine=machine):
                with self.assertRaisesRegex(
                    RuntimeError,
                    'cannot infer the Debian architecture.*DEB_ARCH',
                ):
                    BUILD.deb_arch_from_machine(machine)

    def test_get_debian_architecture_requires_loongarch_metadata(self):
        for machine in ('loongarch64', 'loong64'):
            with self.subTest(machine=machine):
                with mock.patch.dict(os.environ, {}, clear=True):
                    with mock.patch.object(
                        BUILD.subprocess,
                        'check_output',
                        side_effect=OSError,
                    ):
                        with mock.patch.object(
                            BUILD.platform,
                            'machine',
                            return_value=machine,
                        ):
                            with self.assertRaisesRegex(
                                RuntimeError,
                                'cannot infer the Debian architecture.*DEB_ARCH',
                            ):
                                BUILD.get_deb_arch()

    def test_debian_architecture_fallback_rejects_unknown_machine(self):
        with self.assertRaisesRegex(RuntimeError, 'set DEB_ARCH'):
            BUILD.deb_arch_from_machine('riscv64')


if __name__ == '__main__':
    unittest.main()
