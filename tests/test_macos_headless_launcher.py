#!/usr/bin/env python3
import argparse
import subprocess
from pathlib import Path


USAGE = (
    "Usage: RustDesk-Herbin --terminal --headless "
    "[--relay] [--persistent] <peer-id>"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify that the macOS app routes headless CLI before AppKit."
    )
    parser.add_argument("app", type=Path, help="RustDesk-Herbin.app to test")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    executable = args.app / "Contents" / "MacOS" / "RustDesk-Herbin"
    if not executable.is_file():
        raise SystemExit(f"missing RDH executable: {executable}")

    completed = subprocess.run(
        [str(executable), "--terminal", "--headless"],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=15,
        check=False,
    )
    stderr = completed.stderr.decode("utf-8", errors="replace")

    assert completed.returncode == 2, (
        "headless usage error must exit through the Rust CLI before AppKit; "
        f"got status {completed.returncode}, stderr={stderr!r}"
    )
    assert USAGE in stderr, f"missing headless usage text in stderr: {stderr!r}"


if __name__ == "__main__":
    main()
