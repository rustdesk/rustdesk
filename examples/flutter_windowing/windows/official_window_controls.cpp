#include "official_window_controls.h"

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <windows.h>

#include <limits>

namespace {
using flutter::EncodableMap;
using flutter::EncodableValue;

int64_t ReadInteger(const EncodableMap& values, const char* key,
                    int64_t fallback) {
  const auto it = values.find(EncodableValue(key));
  if (it == values.end()) return fallback;
  if (const auto* value = std::get_if<int64_t>(&it->second)) return *value;
  if (const auto* value = std::get_if<int32_t>(&it->second)) return *value;
  return fallback;
}
}  // namespace

void RegisterOfficialWindowControls(FlutterDesktopPluginRegistrarRef registrar) {
  auto* plugin_registrar = flutter::PluginRegistrarManager::GetInstance()
      ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar);
  flutter::MethodChannel<EncodableValue> channel(
      plugin_registrar->messenger(), "rustdesk/desktop_multi_window/win32",
      &flutter::StandardMethodCodec::GetInstance());
  channel.SetMethodCallHandler([](const auto& call, auto result) {
    const auto* values = std::get_if<EncodableMap>(call.arguments());
    const auto hwnd = values ? reinterpret_cast<HWND>(
        static_cast<intptr_t>(ReadInteger(*values, "hwnd", 0))) : nullptr;
    DWORD process_id = 0;
    // Keep the compatibility channel scoped to top-level windows on this
    // engine's platform thread. Never operate on another application's HWND.
    if (!hwnd || GetWindowThreadProcessId(hwnd, &process_id) != GetCurrentThreadId() ||
        process_id != GetCurrentProcessId() || GetAncestor(hwnd, GA_ROOT) != hwnd) {
      result->Error("invalid_window", "Expected a top-level window on the platform thread");
      return;
    }

    const auto& method = call.method_name();
    if (method == "isHidden") {
      result->Success(EncodableValue(!IsWindowVisible(hwnd)));
      return;
    }
    if (method == "getFrame") {
      RECT rect{};
      if (!GetWindowRect(hwnd, &rect)) {
        result->Error("win32_error", "GetWindowRect failed");
        return;
      }
      result->Success(EncodableValue(EncodableMap{
          {EncodableValue("x"), EncodableValue(static_cast<double>(rect.left))},
          {EncodableValue("y"), EncodableValue(static_cast<double>(rect.top))},
          {EncodableValue("width"), EncodableValue(static_cast<double>(rect.right - rect.left))},
          {EncodableValue("height"), EncodableValue(static_cast<double>(rect.bottom - rect.top))},
      }));
      return;
    }
    if (method == "setFrame") {
      constexpr auto invalid = (std::numeric_limits<int64_t>::min)();
      const auto x = ReadInteger(*values, "x", invalid);
      const auto y = ReadInteger(*values, "y", invalid);
      const auto width = ReadInteger(*values, "width", invalid);
      const auto height = ReadInteger(*values, "height", invalid);
      constexpr auto min = (std::numeric_limits<int32_t>::min)();
      constexpr auto max = (std::numeric_limits<int32_t>::max)();
      if (x < min || x > max || y < min || y > max ||
          width <= 0 || width > max || height <= 0 || height > max ||
          x + width > max || y + height > max) {
        result->Error("invalid_frame", "Expected a positive Win32 window rectangle");
        return;
      }
      if (!SetWindowPos(hwnd, nullptr, static_cast<int>(x), static_cast<int>(y),
                        static_cast<int>(width), static_cast<int>(height),
                        SWP_NOACTIVATE | SWP_NOZORDER)) {
        result->Error("win32_error", "SetWindowPos failed");
        return;
      }
    } else if (method == "center") {
      RECT rect{};
      MONITORINFO monitor{};
      monitor.cbSize = sizeof(monitor);
      if (!GetWindowRect(hwnd, &rect) ||
          !GetMonitorInfo(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &monitor)) {
        result->Error("win32_error", "Could not determine window and monitor bounds");
        return;
      }
      const auto& bounds = monitor.rcMonitor;
      const auto x = bounds.left + (bounds.right - bounds.left - (rect.right - rect.left)) / 2;
      const auto y = bounds.top + (bounds.bottom - bounds.top - (rect.bottom - rect.top)) / 2;
      if (!SetWindowPos(hwnd, nullptr, x, y, 0, 0,
                        SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER)) {
        result->Error("win32_error", "Could not center window");
        return;
      }
    } else if (method == "showTitleBar") {
      const auto show = ReadInteger(*values, "show", -1);
      if (show != 0 && show != 1) {
        result->Error("invalid_argument", "Expected a title bar visibility flag");
        return;
      }
      const auto style = GetWindowLongPtr(hwnd, GWL_STYLE);
      SetLastError(ERROR_SUCCESS);
      const auto previous = SetWindowLongPtr(
          hwnd, GWL_STYLE, show ? (style | WS_CAPTION) : (style & ~WS_CAPTION));
      if ((!previous && GetLastError() != ERROR_SUCCESS) ||
          !SetWindowPos(hwnd, nullptr, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE |
                        SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED)) {
        result->Error("win32_error", "Could not update title bar");
        return;
      }
    } else if (method == "hide") {
      ShowWindow(hwnd, SW_HIDE);
    } else if (method == "show") {
      ShowWindow(hwnd, IsIconic(hwnd) ? SW_RESTORE : SW_SHOW);
      SetForegroundWindow(hwnd);
    } else if (method == "focus") {
      if (IsIconic(hwnd)) ShowWindow(hwnd, SW_RESTORE);
      SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE);
      // Windows may deny activation when another process owns the foreground.
      SetForegroundWindow(hwnd);
    } else {
      result->NotImplemented();
      return;
    }
    result->Success();
  });
}
