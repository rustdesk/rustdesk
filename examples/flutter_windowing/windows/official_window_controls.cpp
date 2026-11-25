#include "official_window_controls.h"

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <windows.h>
#include <commctrl.h>

#include <array>
#include <limits>
#include <map>
#include <memory>

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

constexpr auto kControlsChannel = "rustdesk/desktop_multi_window/win32";

class WindowGeometryEvents {
 public:
  explicit WindowGeometryEvents(flutter::BinaryMessenger* messenger)
      : channel_(messenger, kControlsChannel,
                 &flutter::StandardMethodCodec::GetInstance()) {}

  ~WindowGeometryEvents() {
    for (const auto& window : windows_) {
      RemoveWindowSubclass(window.first, OnWindowMessage, SubclassId());
    }
  }

  bool Track(HWND window, int64_t id) {
    if (!SetWindowSubclass(window, OnWindowMessage, SubclassId(),
                           reinterpret_cast<DWORD_PTR>(this))) {
      return false;
    }
    windows_.insert_or_assign(window, id);
    return true;
  }

 private:
  UINT_PTR SubclassId() const { return reinterpret_cast<UINT_PTR>(this); }

  static LRESULT CALLBACK OnWindowMessage(HWND window, UINT message,
                                          WPARAM wparam, LPARAM lparam,
                                          UINT_PTR subclass_id,
                                          DWORD_PTR reference) {
    auto* events = reinterpret_cast<WindowGeometryEvents*>(reference);
    switch (message) {
      case WM_MOVING:
        events->Emit(window, "move");
        break;
      case WM_MOVE:
        events->Emit(window, "moved");
        break;
      case WM_SIZE:
        events->Emit(window, "resized");
        break;
      case WM_NCDESTROY:
        RemoveWindowSubclass(window, OnWindowMessage, subclass_id);
        events->windows_.erase(window);
        break;
    }
    return DefSubclassProc(window, message, wparam, lparam);
  }

  void Emit(HWND window, const char* event) {
    const auto it = windows_.find(window);
    if (it == windows_.end()) return;
    channel_.InvokeMethod("geometryEvent",
        std::make_unique<EncodableValue>(EncodableMap{
            {EncodableValue("windowId"), EncodableValue(it->second)},
            {EncodableValue("event"), EncodableValue(event)},
        }));
  }

  flutter::MethodChannel<EncodableValue> channel_;
  std::map<HWND, int64_t> windows_;
};
}  // namespace

void RegisterOfficialWindowControls(FlutterDesktopPluginRegistrarRef registrar) {
  auto* plugin_registrar = flutter::PluginRegistrarManager::GetInstance()
      ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar);
  auto events = std::make_shared<WindowGeometryEvents>(plugin_registrar->messenger());
  flutter::MethodChannel<EncodableValue> channel(
      plugin_registrar->messenger(), kControlsChannel,
      &flutter::StandardMethodCodec::GetInstance());
  channel.SetMethodCallHandler([events](const auto& call, auto result) {
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
    if (method == "trackGeometry") {
      const auto id = ReadInteger(*values, "windowId", -1);
      if (id < 0 || !events->Track(hwnd, id)) {
        result->Error("win32_error", "Could not track window geometry");
        return;
      }
      result->Success();
      return;
    }
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
    } else if (method == "startDragging" || method == "startResizing") {
      POINT cursor{};
      // A drag from Flutter's tab strip starts in the client area. Request
      // the move directly so Flutter's host cannot re-hit-test it as HTCLIENT.
      UINT message = WM_SYSCOMMAND;
      WPARAM command = SC_MOVE | HTCAPTION;
      if (method == "startResizing") {
        // Ordered like SubWindowResizeEdge on the Dart side.
        constexpr std::array<WPARAM, 8> edges = {
            HTTOP, HTLEFT, HTRIGHT, HTBOTTOM,
            HTTOPLEFT, HTBOTTOMLEFT, HTTOPRIGHT, HTBOTTOMRIGHT};
        const auto edge = ReadInteger(*values, "edge", -1);
        if (edge < 0 || edge >= static_cast<int64_t>(edges.size())) {
          result->Error("invalid_argument", "Expected a window resize edge");
          return;
        }
        message = WM_NCLBUTTONDOWN;
        command = edges[static_cast<size_t>(edge)];
      }
      if (!GetCursorPos(&cursor)) {
        result->Error("win32_error", "Could not read the pointer position");
        return;
      }
      ReleaseCapture();
      // Post the native gesture so the modal move/resize loop cannot block
      // the response to Dart on the platform thread.
      if (!PostMessage(hwnd, message, command,
                       MAKELPARAM(cursor.x, cursor.y))) {
        result->Error("win32_error", "Could not start the native window gesture");
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
