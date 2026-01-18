from __future__ import annotations

import argparse
import os
import re
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
except ImportError:  # 支持 `python src/u2runner/story10_disable_update.py` 直接运行
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


_PKG = "com.carriez.flutter_hbb"


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


def _tap_settings_tab(console: Console, d) -> None:
    candidates = [
        {"description": "Settings\nTab 4 of 4"},
        {"description": "Settings Tab 4 of 4"},
        {"descriptionContains": "Settings"},
        {"descriptionContains": "设置"},
        {"descriptionContains": "設定"},
        {"descriptionContains": "Tab 4 of 4"},
    ]

    # App 启动后底部 Tab 可能延迟渲染：这里做一次带超时的轮询
    deadline = time.time() + 15.0
    while time.time() < deadline:
        for sel in candidates:
            obj = d(**sel)
            if not obj.exists:
                continue

            x, y = obj.center()
            # 部分机型/模拟器底部系统手势条会吞掉过低的点击：向上偏移一点点
            d.click(int(x), max(0, int(y) - 40))
            if d(**sel, selected=True).wait(timeout=5.0):
                return

        time.sleep(0.2)

    # 再兜底：直接从层级里解析 Tab bounds（适配换行/编码差异）
    try:
        xml = d.dump_hierarchy(compressed=False)
        m = re.search(r'content-desc="[^"]*Tab 4 of 4[^"]*"[^>]*bounds="([^"]+)"', xml)
        if m:
            bounds = m.group(1)
            m2 = re.match(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", bounds)
            if m2:
                x1, y1, x2, y2 = map(int, m2.groups())
                x = (x1 + x2) // 2
                y = (y1 + y2) // 2
                console.print(f"[yellow]通过层级解析到 Settings Tab bounds={bounds}，尝试中心点击[/yellow]")
                d.click(x, max(0, y - 40))
                time.sleep(0.8)
                return
    except Exception:  # noqa: BLE001
        pass

    w, h = d.window_size()
    console.print("[yellow]未能通过文本/层级定位 Settings 标签，尝试坐标点击（偏上右下角）[/yellow]")
    d.click(int(w * 0.88), int(h * 0.88))
    time.sleep(0.8)


def _assert_no_update_banner(d) -> None:
    # 连接页顶部更新条（粉色按钮）
    if d(textMatches=r"(?i)(Download new version|下载新版本)").exists:
        raise AssertionError("发现更新提示条（Download new version/下载新版本），与“禁用更新检查”不符")


def _assert_update_setting_disabled(console: Console, d) -> None:
    # Settings 页中：通过 Semantics(label) 定位，避免语言/布局差异
    target = d(descriptionContains="u2_settings_check_update_on_startup") | d(textContains="u2_settings_check_update_on_startup")

    for _ in range(8):
        if target.exists:
            break
        d.swipe_ext("up", scale=0.6)
        time.sleep(0.5)

    if not target.exists:
        # 再兜底：直接在层级 XML 中查找标识
        try:
            xml = d.dump_hierarchy(compressed=False)
            if "u2_settings_check_update_on_startup" in xml:
                console.print("[yellow]标识存在于层级 XML，但未被 selector 命中（可能为合并语义节点）[/yellow]")
            else:
                raise AssertionError("未找到设置项语义标识：u2_settings_check_update_on_startup")
        except AssertionError:
            raise
        except Exception as e:  # noqa: BLE001
            raise AssertionError("未找到设置项语义标识：u2_settings_check_update_on_startup") from e

    if not d(textMatches=r"(?i)(Disabled by custom client|已由客户端定制禁用)").exists:
        raise AssertionError("未找到禁用提示文本（已由客户端定制禁用/Disabled by custom client）")

    console.print("[green]设置页校验通过[/green]：更新检查入口已禁用并提示原因")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Story 10：验证 Android 端彻底禁用更新检查（UIAutomator2）")
    parser.add_argument("--serial", required=True, help="adb 设备序列号（如 emulator-5554）")
    parser.add_argument("--timeout", type=int, default=240, help="等待设备/开机完成超时秒数")
    parser.add_argument("-v", "--verbose", action="count", default=0, help="输出更详细的过程（可重复 -vv）")
    parser.add_argument("--poll-interval", type=float, default=1.0, help="轮询间隔秒数（默认 1.0）")
    parser.add_argument("--adb-timeout", type=int, default=10, help="单次 adb 命令超时秒数（默认 10）")
    parser.add_argument("--launch-wait", type=float, default=3.0, help="启动后等待秒数（默认 3.0）")
    parser.add_argument("--screenshot", default="", help="失败时可选：保存截图到指定路径（如 .\\u2_story10.png）")
    parser.add_argument("--dump-hierarchy", default="", help="失败时可选：保存层级 XML 到指定路径（如 .\\u2_story10.xml）")
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

    console.print("3) 启动 App 并做 Story 10 校验…")
    d = None
    try:
        import uiautomator2 as u2

        d = u2.connect(args.serial)
        d.app_stop(_PKG)
        d.app_start(_PKG)
        time.sleep(args.launch_wait)

        _assert_no_update_banner(d)

        _tap_settings_tab(console, d)
        time.sleep(0.8)
        _assert_update_setting_disabled(console, d)
    except Exception as e:  # noqa: BLE001
        if d is not None:
            if args.screenshot:
                try:
                    d.screenshot(args.screenshot)
                    console.print(f"[yellow]已保存截图[/yellow]: {args.screenshot}")
                except Exception:  # noqa: BLE001
                    pass
            if args.dump_hierarchy:
                try:
                    xml = d.dump_hierarchy(compressed=False)
                    with open(args.dump_hierarchy, "w", encoding="utf-8") as f:
                        f.write(xml)
                    console.print(f"[yellow]已保存层级 XML[/yellow]: {args.dump_hierarchy}")
                except Exception:  # noqa: BLE001
                    pass
        console.print(f"[red]Story 10 校验失败[/red]: {e}")
        return 5

    console.print("[green]OK[/green]：Story 10（禁用更新检查）UI 验收通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
