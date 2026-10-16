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
            BUILD.get_flutter_linux_build_command('x86_64'),
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
        with mock.patch.dict(os.environ, {'DEB_ARCH': 'loong64'}):
            self.assertEqual(BUILD.get_deb_arch(), 'loong64')

    def test_debian_architecture_from_dpkg(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            with mock.patch.object(
                BUILD.subprocess,
                'check_output',
                return_value=b'loongarch64\n',
            ):
                self.assertEqual(BUILD.get_deb_arch(), 'loongarch64')

    def test_debian_architecture_fallback(self):
        self.assertEqual(BUILD.deb_arch_from_machine('loongarch64'), 'loongarch64')
        self.assertEqual(BUILD.deb_arch_from_machine('loong64'), 'loong64')
        self.assertEqual(BUILD.deb_arch_from_machine('aarch64'), 'arm64')
        self.assertEqual(BUILD.deb_arch_from_machine('amd64'), 'amd64')

    def test_debian_architecture_fallback_rejects_unknown_machine(self):
        with self.assertRaisesRegex(RuntimeError, 'set DEB_ARCH'):
            BUILD.deb_arch_from_machine('riscv64')


if __name__ == '__main__':
    unittest.main()
