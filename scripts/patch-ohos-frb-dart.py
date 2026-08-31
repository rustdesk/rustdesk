#!/usr/bin/env python3
"""Repair the remaining FRB 1.80.1/Dart 3.11 OHOS binding mismatches."""

from __future__ import annotations

import re
import sys
from pathlib import Path


SYNC_TASK_RE = re.compile(
    r"FlutterRustBridgeSyncTask\(\s*"
    r"callFfi:\s*\(\)\s*=>\s*_platform\.inner\s*\.\s*"
    r"(wire_[A-Za-z0-9_]+)",
    re.DOTALL,
)


MISSING_BINDINGS = {
    "wire_main_configure_ohos_host_display": """
  void wire_main_configure_ohos_host_display(
    int port_,
    int width,
    int height,
    int displayId,
  ) {
    return _wire_main_configure_ohos_host_display(
      port_,
      width,
      height,
      displayId,
    );
  }

  late final _wire_main_configure_ohos_host_displayPtr = _lookup<
      ffi.NativeFunction<
          ffi.Void Function(
            ffi.Int64,
            ffi.UintPtr,
            ffi.UintPtr,
            ffi.Uint64,
          )>>('wire_main_configure_ohos_host_display');
  late final _wire_main_configure_ohos_host_display =
      _wire_main_configure_ohos_host_displayPtr.asFunction<
          void Function(int, int, int, int)>();
""",
    "wire_main_set_ohos_host_clipboard_enabled": """
  void wire_main_set_ohos_host_clipboard_enabled(int port_, bool enabled) {
    return _wire_main_set_ohos_host_clipboard_enabled(port_, enabled);
  }

  late final _wire_main_set_ohos_host_clipboard_enabledPtr = _lookup<
      ffi.NativeFunction<
          ffi.Void Function(
            ffi.Int64,
            ffi.Bool,
          )>>('wire_main_set_ohos_host_clipboard_enabled');
  late final _wire_main_set_ohos_host_clipboard_enabled =
      _wire_main_set_ohos_host_clipboard_enabledPtr
          .asFunction<void Function(int, bool)>();
""",
    "wire_main_update_ohos_host_clipboard_text": """
  void wire_main_update_ohos_host_clipboard_text(
    int port_,
    ffi.Pointer<wire_uint_8_list> text,
  ) {
    return _wire_main_update_ohos_host_clipboard_text(port_, text);
  }

  late final _wire_main_update_ohos_host_clipboard_textPtr = _lookup<
      ffi.NativeFunction<
          ffi.Void Function(
            ffi.Int64,
            ffi.Pointer<wire_uint_8_list>,
          )>>('wire_main_update_ohos_host_clipboard_text');
  late final _wire_main_update_ohos_host_clipboard_text =
      _wire_main_update_ohos_host_clipboard_textPtr.asFunction<
          void Function(int, ffi.Pointer<wire_uint_8_list>)>();
""",
    "wire_main_take_ohos_host_clipboard_text": """
  void wire_main_take_ohos_host_clipboard_text(int port_) {
    return _wire_main_take_ohos_host_clipboard_text(port_);
  }

  late final _wire_main_take_ohos_host_clipboard_textPtr =
      _lookup<ffi.NativeFunction<ffi.Void Function(ffi.Int64)>>(
    'wire_main_take_ohos_host_clipboard_text',
  );
  late final _wire_main_take_ohos_host_clipboard_text =
      _wire_main_take_ohos_host_clipboard_textPtr
          .asFunction<void Function(int)>();
""",
    "wire_main_start_ohos_host": """
  void wire_main_start_ohos_host(int port_) {
    return _wire_main_start_ohos_host(port_);
  }

  late final _wire_main_start_ohos_hostPtr =
      _lookup<ffi.NativeFunction<ffi.Void Function(ffi.Int64)>>(
    'wire_main_start_ohos_host',
  );
  late final _wire_main_start_ohos_host =
      _wire_main_start_ohos_hostPtr.asFunction<void Function(int)>();
""",
    "wire_main_stop_ohos_host": """
  void wire_main_stop_ohos_host(int port_) {
    return _wire_main_stop_ohos_host(port_);
  }

  late final _wire_main_stop_ohos_hostPtr =
      _lookup<ffi.NativeFunction<ffi.Void Function(ffi.Int64)>>(
    'wire_main_stop_ohos_host',
  );
  late final _wire_main_stop_ohos_host =
      _wire_main_stop_ohos_hostPtr.asFunction<void Function(int)>();
""",
    "wire_main_ohos_host_is_started": """
  ffi.Pointer<ffi.Void> wire_main_ohos_host_is_started() {
    return _wire_main_ohos_host_is_started();
  }

  late final _wire_main_ohos_host_is_startedPtr = _lookup<
      ffi.NativeFunction<
          ffi.Pointer<ffi.Void> Function()>>('wire_main_ohos_host_is_started');
  late final _wire_main_ohos_host_is_started =
      _wire_main_ohos_host_is_startedPtr
          .asFunction<ffi.Pointer<ffi.Void> Function()>();
""",
    "wire_cm_close_connection_window": """
  void wire_cm_close_connection_window(int port_, int connId) {
    return _wire_cm_close_connection_window(port_, connId);
  }

  late final _wire_cm_close_connection_windowPtr =
      _lookup<ffi.NativeFunction<ffi.Void Function(ffi.Int64, ffi.Int32)>>(
    'wire_cm_close_connection_window',
  );
  late final _wire_cm_close_connection_window =
      _wire_cm_close_connection_windowPtr
          .asFunction<void Function(int, int)>();
""",
}


FRB_RUNTIME_BINDINGS = """
  int init_frb_dart_api_dl(ffi.Pointer<ffi.Void> data) {
    return _init_frb_dart_api_dl(data);
  }

  late final _init_frb_dart_api_dlPtr = _lookup<
      ffi.NativeFunction<
          ffi.IntPtr Function(ffi.Pointer<ffi.Void>)>>('init_frb_dart_api_dl');
  late final _init_frb_dart_api_dl = _init_frb_dart_api_dlPtr
      .asFunction<int Function(ffi.Pointer<ffi.Void>)>();

  void store_dart_post_cobject(
    ffi.Pointer<
        ffi.NativeFunction<
            ffi.Bool Function(ffi.Int64, ffi.Pointer<ffi.Void>)>> ptr,
  ) {
    return _store_dart_post_cobject(ptr);
  }

  late final _store_dart_post_cobjectPtr = _lookup<
      ffi.NativeFunction<
          ffi.Void Function(
            ffi.Pointer<
                ffi.NativeFunction<
                    ffi.Bool Function(
                      ffi.Int64,
                      ffi.Pointer<ffi.Void>,
                    )>>,
          )>>('store_dart_post_cobject');
  late final _store_dart_post_cobject = _store_dart_post_cobjectPtr
      .asFunction<
          void Function(
            ffi.Pointer<
                ffi.NativeFunction<
                    ffi.Bool Function(
                      ffi.Int64,
                      ffi.Pointer<ffi.Void>,
                    )>>,
          )>();

  Object get_dart_object(int ptr) {
    return _get_dart_object(ptr);
  }

  late final _get_dart_objectPtr =
      _lookup<ffi.NativeFunction<ffi.Handle Function(ffi.UintPtr)>>(
    'get_dart_object',
  );
  late final _get_dart_object =
      _get_dart_objectPtr.asFunction<Object Function(int)>();

  void drop_dart_object(int ptr) {
    return _drop_dart_object(ptr);
  }

  late final _drop_dart_objectPtr =
      _lookup<ffi.NativeFunction<ffi.Void Function(ffi.UintPtr)>>(
    'drop_dart_object',
  );
  late final _drop_dart_object =
      _drop_dart_objectPtr.asFunction<void Function(int)>();

  int new_dart_opaque(Object obj) {
    return _new_dart_opaque(obj);
  }

  late final _new_dart_opaquePtr =
      _lookup<ffi.NativeFunction<ffi.UintPtr Function(ffi.Handle)>>(
    'new_dart_opaque',
  );
  late final _new_dart_opaque =
      _new_dart_opaquePtr.asFunction<int Function(Object)>();
"""


def has_wire_method(text: str, name: str) -> bool:
    return re.search(
        rf"\n  (?:void|int|bool|Object|WireSyncReturn|ffi\.Pointer<[^>]+>) "
        rf"{re.escape(name)}\(",
        text,
    ) is not None


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {Path(sys.argv[0]).name} GENERATED_DART", file=sys.stderr)
        return 2

    path = Path(sys.argv[1])
    text = path.read_text(encoding="utf-8")
    if "Generated by `flutter_rust_bridge`@ 1.80.1" not in text:
        raise RuntimeError("this compatibility patch is pinned to FRB 1.80.1")

    sync_names = sorted(set(SYNC_TASK_RE.findall(text)))
    if len(sync_names) < 90:
        raise RuntimeError(
            f"unexpectedly found only {len(sync_names)} synchronous FRB functions"
        )
    sync_return_is_address = re.search(
        r"void free_WireSyncReturn\(\s*int\s+\w+\s*\)", text
    ) is not None
    for name in sync_names:
        call_re = re.compile(
            rf"(callFfi:\s*\(\)\s*=>\s*_platform\.inner\s*\.\s*"
            rf"{re.escape(name)}\([\s\S]*?\))"
            rf"(?=,\s*\n\s*parseSuccessData:)",
        )
        replacement = (
            r"callFfi: () => WireSyncReturn.fromAddress(\1)"
            if sync_return_is_address
            else r"\1.cast()"
        )
        if sync_return_is_address:
            call_re = re.compile(
                rf"callFfi:\s*\(\)\s*=>\s*(_platform\.inner\s*\.\s*"
                rf"{re.escape(name)}\([\s\S]*?\))"
                rf"(?=,\s*\n\s*parseSuccessData:)",
            )
        text, count = call_re.subn(replacement, text, count=1)
        if count != 1:
            raise RuntimeError(f"unable to adapt synchronous result for {name}")

    if "WireSyncReturn" in text:
        free_re = re.compile(
            r"(?ms)^[ \t]+void free_WireSyncReturn\(.*?\)\s*"
            r"(?:\{.*?^[ \t]+\}|=>.*?;)",
        )
        free_argument = "ptr.address" if sync_return_is_address else "ptr.cast()"
        text, count = free_re.subn(
            "\n  void free_WireSyncReturn(WireSyncReturn ptr) {\n"
            f"    return _free_WireSyncReturn({free_argument});\n"
            "  }",
            text,
            count=1,
        )
        if count != 1:
            candidates = "\n".join(
                line for line in text.splitlines() if "WireSyncReturn" in line
            )
            raise RuntimeError(
                "unable to patch free_WireSyncReturn override; candidates:\n"
                + candidates
            )

    additions: list[str] = []
    if not has_wire_method(text, "init_frb_dart_api_dl"):
        additions.append(FRB_RUNTIME_BINDINGS)
    for name, binding in MISSING_BINDINGS.items():
        if not has_wire_method(text, name):
            additions.append(binding)
    if additions:
        class_match = re.search(
            r"class RustdeskWire\b[^\{]*\{",
            text,
        )
        if class_match is None:
            raise RuntimeError("unable to locate RustdeskWire class")
        text = (
            text[: class_match.end()]
            + "\n"
            + "\n".join(additions)
            + text[class_match.end() :]
        )

    path.write_text(text, encoding="utf-8")
    print(
        f"Patched {len(sync_names)} FRB sync call sites and "
        f"inserted {len(additions)} binding blocks in {path}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
