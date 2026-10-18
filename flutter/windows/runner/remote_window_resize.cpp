#include "remote_window_resize.h"

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>

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
      ratio_ = values[0];
      inset_width_ = values[1];
      inset_height_ = values[2];
      result->Success();
    });
    delegate_ = registrar_->RegisterTopLevelWindowProcDelegate(
        [this](HWND hwnd, UINT message, WPARAM edge, LPARAM param)
            -> std::optional<LRESULT> {
          if (message != WM_SIZING || ratio_ <= 0 || IsZoomed(hwnd)) {
            return std::nullopt;
          }
          RECT outer, client;
          if (!GetWindowRect(hwnd, &outer) || !GetClientRect(hwnd, &client)) {
            return std::nullopt;
          }
          const double dpi = GetDpiForWindow(hwnd) / 96.0;
          const LONG extra_width = outer.right - outer.left - client.right +
              static_cast<LONG>(std::lround(inset_width_ * dpi));
          const LONG extra_height = outer.bottom - outer.top - client.bottom +
              static_cast<LONG>(std::lround(inset_height_ * dpi));
          MINMAXINFO limits{};
          SendMessage(hwnd, WM_GETMINMAXINFO, 0, reinterpret_cast<LPARAM>(&limits));
          ConstrainRemoteWindow(*reinterpret_cast<RECT*>(param), edge, ratio_,
              extra_width, extra_height,
              limits.ptMinTrackSize.x, limits.ptMinTrackSize.y);
          // Preserve window_manager's resize events and its existing lifecycle.
          return std::nullopt;
        });
  }

  ~RemoteWindowResize() override {
    registrar_->UnregisterTopLevelWindowProcDelegate(delegate_);
    channel_.SetMethodCallHandler(nullptr);
  }

 private:
  flutter::PluginRegistrarWindows* registrar_;
  flutter::MethodChannel<> channel_;
  int delegate_;
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
