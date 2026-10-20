#include "remote_window_resize.h"

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <flutter_windows.h>

#include "remote_window_geometry.h"

namespace {
class RemoteWindowResize : public flutter::Plugin {
 public:
  explicit RemoteWindowResize(flutter::PluginRegistrarWindows* registrar)
      : registrar_(registrar), channel_(registrar->messenger(),
            "org.rustdesk.rustdesk/remote-window-resize",
            &flutter::StandardMethodCodec::GetInstance()) {
    channel_.SetMethodCallHandler([this](const auto& call, auto result) {
      if (call.method_name() != "setContentGeometry") {
        result->NotImplemented();
        return;
      }
      const auto* args = call.arguments()
          ? std::get_if<flutter::EncodableList>(call.arguments()) : nullptr;
      if (!args || args->size() != 3) {
        result->Error("invalid-geometry", "Expected ratio and content insets");
        return;
      }
      double values[3];
      for (size_t i = 0; i < 3; ++i) {
        const auto* value = std::get_if<double>(&(*args)[i]);
        if (!value || !std::isfinite(*value) || *value < 0) {
          result->Error("invalid-geometry", "Expected finite nonnegative values");
          return;
        }
        values[i] = *value;
      }
      // The multi-window parent otherwise erases over Flutter's child surface
      // during resizing, briefly replacing the entire remote image with black.
      const HWND hwnd = GetParent(registrar_->GetView()->GetNativeWindow());
      const LONG_PTR style = GetWindowLongPtr(hwnd, GWL_STYLE);
      if (values[0] > 0 && !(style & WS_CLIPCHILDREN)) {
        if (!SetWindowLongPtr(hwnd, GWL_STYLE, style | WS_CLIPCHILDREN)) {
          result->Error("window-style", "Unable to protect Flutter's surface");
          return;
        }
        added_child_clipping_ = true;
      } else if (values[0] == 0 && added_child_clipping_) {
        if (!SetWindowLongPtr(hwnd, GWL_STYLE, style & ~WS_CLIPCHILDREN)) {
          result->Error("window-style", "Unable to restore window painting");
          return;
        }
        added_child_clipping_ = false;
      }
      ratio_ = values[0];
      inset_width_ = values[1];
      inset_height_ = values[2];
      result->Success();
    });
    delegate_ = registrar_->RegisterTopLevelWindowProcDelegate(
        [this](HWND hwnd, UINT message, WPARAM edge, LPARAM param)
            -> std::optional<LRESULT> {
          if (ratio_ <= 0 || IsZoomed(hwnd)) return std::nullopt;
          if (message == WM_SETCURSOR || message == WM_NCLBUTTONDOWN) {
            const int hit = message == WM_SETCURSOR
                ? LOWORD(param) : static_cast<int>(edge);
            if (hit >= HTLEFT && hit <= HTBOTTOMRIGHT) {
              RECT frame;
              POINT pointer;
              if (GetWindowRect(hwnd, &frame) && GetCursorPos(&pointer)) {
                const int corner = RemoteResizeHitTest(hit, frame, pointer);
                if (message == WM_SETCURSOR) {
                  SetCursor(LoadCursor(nullptr,
                      corner == HTTOPLEFT || corner == HTBOTTOMRIGHT
                          ? IDC_SIZENWSE : IDC_SIZENESW));
                  return TRUE;
                }
                if (corner != hit) {
                  return DefWindowProc(hwnd, message,
                      static_cast<WPARAM>(corner), param);
                }
              }
            }
          }
          if (message != WM_SIZING) return std::nullopt;
          RECT outer, client;
          if (!GetWindowRect(hwnd, &outer) || !GetClientRect(hwnd, &client)) {
            return std::nullopt;
          }
          const double dpi = FlutterDesktopGetDpiForMonitor(
              MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)) / 96.0;
          const LONG extra_width = outer.right - outer.left - client.right +
              static_cast<LONG>(std::lround(inset_width_ * dpi));
          const LONG extra_height = outer.bottom - outer.top - client.bottom +
              static_cast<LONG>(std::lround(inset_height_ * dpi));
          MINMAXINFO limits{};
          SendMessage(hwnd, WM_GETMINMAXINFO, 0, reinterpret_cast<LPARAM>(&limits));
          MONITORINFO monitor{};
          monitor.cbSize = sizeof(monitor);
          const bool have_bounds = GetMonitorInfo(
              MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &monitor) != FALSE;
          ConstrainRemoteWindow(*reinterpret_cast<RECT*>(param), edge, ratio_,
              extra_width, extra_height,
              limits.ptMinTrackSize.x, limits.ptMinTrackSize.y,
              RemoteResizeWorkArea(outer, have_bounds ? &monitor.rcWork : nullptr));
          // Preserve window_manager's resize events and its existing lifecycle.
          return std::nullopt;
        });
  }

  ~RemoteWindowResize() override {
    if (added_child_clipping_) {
      const HWND hwnd = GetParent(registrar_->GetView()->GetNativeWindow());
      const LONG_PTR style = GetWindowLongPtr(hwnd, GWL_STYLE);
      SetWindowLongPtr(hwnd, GWL_STYLE, style & ~WS_CLIPCHILDREN);
    }
    registrar_->UnregisterTopLevelWindowProcDelegate(delegate_);
    channel_.SetMethodCallHandler(nullptr);
  }

 private:
  flutter::PluginRegistrarWindows* registrar_;
  flutter::MethodChannel<> channel_;
  int delegate_;
  bool added_child_clipping_ = false;
  double ratio_ = 0;
  double inset_width_ = 0;
  double inset_height_ = 0;
};
}  // namespace

void RegisterRemoteWindowResize(flutter::FlutterEngine* engine) {
  auto* registrar = flutter::PluginRegistrarManager::GetInstance()
      ->GetRegistrar<flutter::PluginRegistrarWindows>(
          engine->GetRegistrarForPlugin("RemoteWindowResize"));
  registrar->AddPlugin(std::make_unique<RemoteWindowResize>(registrar));
}
