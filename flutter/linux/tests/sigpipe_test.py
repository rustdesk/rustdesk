import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest


class LinuxRunnerSignalTest(unittest.TestCase):
    def test_closed_ipc_returns_error_in_rust_core(self):
        runner = Path(__file__).resolve().parents[1]
        compiler = shlex.split(os.environ.get("CXX", "c++"))
        gtk_flags = shlex.split(subprocess.check_output(
            ["pkg-config", "--cflags", "--libs", "gtk+-3.0"], text=True))

        with tempfile.TemporaryDirectory(prefix="rustdesk-sigpipe-") as tmp:
            tmp = Path(tmp)
            lib_dir = tmp / "lib"
            lib_dir.mkdir()
            core = tmp / "core.cc"
            core.write_text(r'''
#include <errno.h>
#include <stdio.h>
#include <sys/socket.h>
#include <unistd.h>

extern "C" bool rustdesk_core_main() {
  int sockets[2];
  if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets) != 0) _exit(2);
  close(sockets[1]);
  // Rust 1.75 UnixStream::write reaches write(), without MSG_NOSIGNAL.
  ssize_t result = write(sockets[0], "x", 1);
  if (result != -1 || errno != EPIPE) _exit(3);
  close(sockets[0]);
  puts("closed IPC returned EPIPE");
  return false;
}
''')
            application = tmp / "application.cc"
            application.write_text('''
#include "my_application.h"
#include <stdlib.h>

MyApplication* my_application_new() { abort(); }
''')
            subprocess.run(compiler + [
                "-shared", "-fPIC", str(core),
                "-o", str(lib_dir / "librustdesk.so"),
            ], check=True)
            executable = tmp / "rustdesk"
            subprocess.run(compiler + [
                "-std=c++14", "-I", str(runner), str(runner / "main.cc"),
                str(application), "-o", str(executable),
            ] + gtk_flags + ["-ldl"], check=True)

            env = os.environ.copy()
            if sys.platform == "darwin":
                # Exercise the same entry point on development hosts without /proc.
                env["DYLD_LIBRARY_PATH"] = str(lib_dir)
            result = subprocess.run(
                [str(executable), "--server"], env=env, capture_output=True,
                text=True, restore_signals=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("closed IPC returned EPIPE", result.stdout)


if __name__ == "__main__":
    unittest.main()
