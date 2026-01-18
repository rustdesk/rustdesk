from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time

from loguru import logger
from rich.console import Console


def _run(cmd: list[str], timeout_s: int = 60) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            cmd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout_s,
            check=False,
        )
    except subprocess.TimeoutExpired as e:
        stdout = ""
        if isinstance(e.stdout, str):
            stdout = e.stdout
        elif isinstance(e.stdout, bytes):
            stdout = e.stdout.decode(errors="replace")
        return subprocess.CompletedProcess(cmd, 124, stdout)


def _adb() -> str:
    adb = os.environ.get("ADB", "").strip()
    if adb:
        return adb
    # 与 uiautomator2/adbutils 的约定对齐：Windows 侧通常通过该变量指定 adb.exe 路径
    adbutils_adb = os.environ.get("ADBUTILS_ADB_PATH", "").strip()
    if adbutils_adb:
        return adbutils_adb
    return "adb"

def _trim_one_line(text: str, max_len: int = 200) -> str:
    s = (text or "").strip().replace("\r", "")
    s = " ".join(s.splitlines()).strip()
    if len(s) > max_len:
        return s[: max_len - 1] + "…"
    return s


def _describe_adb_env() -> str:
    adb_bin = _adb()
    socket = os.environ.get("ADB_SERVER_SOCKET", "")
    parts = [f"ADB={adb_bin}"]
    if socket:
        parts.append(f"ADB_SERVER_SOCKET={socket}")
    return ", ".join(parts)


def _wait_for_device(
    console: Console,
    serial: str,
    timeout_s: int,
    *,
    poll_interval_s: float,
    cmd_timeout_s: int,
    verbose: int,
) -> None:
    deadline = time.time() + timeout_s
    last_line = ""
    last_print = 0.0
    while True:
        if time.time() > deadline:
            raise TimeoutError(f"等待设备上线超时：{serial}（{last_line}）")
        cmd = [_adb(), "-s", serial, "get-state"]
        if verbose >= 2:
            console.print(f"[dim]$ {' '.join(cmd)}[/dim]")
        result = _run(cmd, timeout_s=cmd_timeout_s)
        state = result.stdout.strip()
        last_line = f"returncode={result.returncode}, state={_trim_one_line(state)}"
        if result.returncode == 0 and state == "device":
            if verbose:
                console.print(f"[green]设备已上线[/green]（{serial}）")
            return
        now = time.time()
        if verbose and (now - last_print) >= 1.0:
            left = max(0, int(deadline - now))
            if result.returncode == 124:
                console.print(f"[yellow]等待设备…[/yellow] adb get-state 超时，剩余 {left}s")
            else:
                console.print(f"[yellow]等待设备…[/yellow] state={_trim_one_line(state) or '(空)'}，剩余 {left}s")
            last_print = now
        time.sleep(poll_interval_s)


def _wait_for_boot_completed(
    console: Console,
    serial: str,
    timeout_s: int,
    *,
    poll_interval_s: float,
    cmd_timeout_s: int,
    verbose: int,
) -> None:
    deadline = time.time() + timeout_s
    last_line = ""
    last_print = 0.0
    while True:
        if time.time() > deadline:
            raise TimeoutError(f"等待开机完成超时：{serial}（{last_line}）")

        cmd = [_adb(), "-s", serial, "shell", "getprop", "sys.boot_completed"]
        if verbose >= 2:
            console.print(f"[dim]$ {' '.join(cmd)}[/dim]")
        result = _run(cmd, timeout_s=cmd_timeout_s)
        value = result.stdout.strip()
        last_line = f"returncode={result.returncode}, sys.boot_completed={_trim_one_line(value)}"
        if result.returncode == 0 and value == "1":
            if verbose:
                console.print(f"[green]系统已开机完成[/green]（{serial}）")
            return

        now = time.time()
        if verbose and (now - last_print) >= 1.0:
            left = max(0, int(deadline - now))
            if result.returncode == 124:
                console.print(f"[yellow]等待开机…[/yellow] getprop 超时，剩余 {left}s")
            else:
                shown = _trim_one_line(value) or "(空)"
                console.print(f"[yellow]等待开机…[/yellow] sys.boot_completed={shown}，剩余 {left}s")
            last_print = now
        time.sleep(poll_interval_s)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="连接并校验 uiautomator2 设备状态")
    parser.add_argument("--serial", required=True, help="adb 设备序列号（如 emulator-5554）")
    parser.add_argument("--timeout", type=int, default=180, help="等待设备/开机完成超时秒数")
    parser.add_argument("-v", "--verbose", action="count", default=0, help="输出更详细的连接过程（可重复 -vv）")
    parser.add_argument("--poll-interval", type=float, default=1.0, help="轮询间隔秒数（默认 1.0）")
    parser.add_argument("--adb-timeout", type=int, default=10, help="单次 adb 命令超时秒数（默认 10）")
    args = parser.parse_args(argv)

    console = Console()
    logger.remove()
    logger.add(sys.stderr, level="INFO")

    console.print(f"[bold]目标设备[/bold]: {args.serial}")
    if args.verbose:
        console.print(f"[dim]{_describe_adb_env()}[/dim]")

    console.print("1) 等待设备上线…")
    _wait_for_device(
        console,
        args.serial,
        args.timeout,
        poll_interval_s=args.poll_interval,
        cmd_timeout_s=args.adb_timeout,
        verbose=args.verbose,
    )

    console.print("2) 等待系统开机完成…")
    _wait_for_boot_completed(
        console,
        args.serial,
        args.timeout,
        poll_interval_s=args.poll_interval,
        cmd_timeout_s=args.adb_timeout,
        verbose=args.verbose,
    )

    console.print("3) 尝试 uiautomator2 连接…")
    try:
        import uiautomator2 as u2

        d = u2.connect(args.serial)
        info = d.info
        console.print("[green]连接成功[/green]")
        console.print(f"品牌/机型: {info.get('brand')} {info.get('model')}")
        console.print(f"SDK: {info.get('sdkInt')}, 版本: {info.get('version')}")
    except Exception as e:  # noqa: BLE001
        console.print(f"[red]连接失败[/red]: {e}")
        return 2

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
