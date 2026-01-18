from __future__ import annotations

import argparse
import os
import sys
import time

from loguru import logger
from rich.console import Console

try:
    from .connect import (
        _adb,
        _describe_adb_env,
        _run,
        _trim_one_line,
        _wait_for_boot_completed,
        _wait_for_device,
    )
except ImportError:  # 支持 `python src/u2runner/smoketest.py` 直接运行
    import pathlib

    _SRC = pathlib.Path(__file__).resolve().parents[1]
    if str(_SRC) not in sys.path:
        sys.path.insert(0, str(_SRC))
    from u2runner.connect import (  # type: ignore[no-redef]
        _adb,
        _describe_adb_env,
        _run,
        _trim_one_line,
        _wait_for_boot_completed,
        _wait_for_device,
    )


def _maybe_windows_host_ip_hint() -> str:
    if not os.environ.get("WSL_DISTRO_NAME"):
        return ""
    return "WindowsHostIP 通常可用：$(grep -m1 nameserver /etc/resolv.conf | awk '{print $2}')"


def _hint_for_adb_server(console: Console) -> None:
    hint = _maybe_windows_host_ip_hint()
    if hint:
        console.print("[yellow]WSL 提示[/yellow]: 如果你要连 Windows 侧 adb server，请先在 Windows 用 adb.exe 以 -a 启动，再在 WSL 设置：")
        console.print(f"[dim]{hint}[/dim]")
        console.print("[dim]export ADB_SERVER_SOCKET=tcp:<WindowsHostIP>:5037[/dim]")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="uiautomator2 冒烟测试（适合 AVD/真机，也适合 WSL 连 Windows adb server）")
    parser.add_argument("--serial", required=True, help="adb 设备序列号（如 emulator-5554）")
    parser.add_argument("--timeout", type=int, default=240, help="等待设备/开机完成超时秒数")
    parser.add_argument("-v", "--verbose", action="count", default=0, help="输出更详细的过程（可重复 -vv）")
    parser.add_argument("--poll-interval", type=float, default=1.0, help="轮询间隔秒数（默认 1.0）")
    parser.add_argument("--adb-timeout", type=int, default=10, help="单次 adb 命令超时秒数（默认 10）")
    parser.add_argument("--screenshot", default="", help="可选：保存截图到指定路径（如 ./u2.png）")
    args = parser.parse_args(argv)

    console = Console()
    logger.remove()
    logger.add(sys.stderr, level="INFO")

    console.print(f"[bold]目标设备[/bold]: {args.serial}")
    if args.verbose:
        console.print(f"[dim]{_describe_adb_env()}[/dim]")

    console.print("0) 检查 adb 可用性…")
    ver = _run([_adb(), "version"], timeout_s=args.adb_timeout)
    if ver.returncode != 0:
        console.print(f"[red]adb 不可用[/red]: { _trim_one_line(ver.stdout) or f'returncode={ver.returncode}' }")
        _hint_for_adb_server(console)
        return 2
    if args.verbose:
        console.print(f"[dim]{_trim_one_line(ver.stdout)}[/dim]")

    console.print("1) 等待设备上线…")
    try:
        _wait_for_device(
            console,
            args.serial,
            args.timeout,
            poll_interval_s=args.poll_interval,
            cmd_timeout_s=args.adb_timeout,
            verbose=args.verbose,
        )
    except Exception as e:  # noqa: BLE001
        console.print(f"[red]设备未就绪[/red]: {e}")
        _hint_for_adb_server(console)
        return 3

    console.print("2) 等待系统开机完成…")
    try:
        _wait_for_boot_completed(
            console,
            args.serial,
            args.timeout,
            poll_interval_s=args.poll_interval,
            cmd_timeout_s=args.adb_timeout,
            verbose=args.verbose,
        )
    except Exception as e:  # noqa: BLE001
        console.print(f"[red]系统未开机完成[/red]: {e}")
        return 4

    console.print("3) uiautomator2 基本动作测试…")
    try:
        import uiautomator2 as u2

        d = u2.connect(args.serial)
        info = d.info
        console.print("[green]连接成功[/green]")
        console.print(f"设备: {info.get('productName') or ''}  分辨率: {info.get('displayWidth')}x{info.get('displayHeight')}  SDK: {info.get('sdkInt')}")

        current = d.app_current()
        console.print(f"前台: {current.get('package')} {current.get('activity')}")

        d.press("home")
        time.sleep(0.5)
        current2 = d.app_current()
        if args.verbose:
            console.print(f"[dim]按 Home 后前台: {current2.get('package')} {current2.get('activity')}[/dim]")

        if args.screenshot:
            d.screenshot(args.screenshot)
            console.print(f"[green]截图已保存[/green]: {args.screenshot}")
    except Exception as e:  # noqa: BLE001
        console.print(f"[red]冒烟测试失败[/red]: {e}")
        _hint_for_adb_server(console)
        return 5

    console.print("[green]OK[/green]：AVD <-> adb <-> uiautomator2 可用")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
