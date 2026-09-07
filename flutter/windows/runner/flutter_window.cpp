#include "flutter_window.h"

#include <desktop_multi_window/desktop_multi_window_plugin.h>
#include <texture_rgba_renderer/texture_rgba_renderer_plugin_c_api.h>
#include <flutter_gpu_texture_renderer/flutter_gpu_texture_renderer_plugin_c_api.h>

#include "flutter/generated_plugin_registrant.h"

#include <flutter/event_channel.h>
#include <flutter/event_sink.h>
#include <flutter/event_stream_handler_functions.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>

#include <windows.h>

#include <optional>
#include <memory>

#include "win32_desktop.h"

namespace {

// If the window is resized between the creation of the Flutter surface and the
// present of the first frame - which is what the PowerToys FancyZones option
// "Move newly created windows to their last known zone" does - the embedder's
// resize synchronization enters kResizeStarted and from then on only presents
// frames that match the new size. A frame already generated for the old size
// is rejected, nothing schedules a matching one, and the window stays white
// until a real resize re-enters OnWindowSizeChanged, which resets the resize
// target and resends the window metrics. That is why minimize/restore heals
// it; ForceChildRefresh() below does the same programmatically.
// https://github.com/rustdesk/rustdesk/issues/6756
// https://github.com/flutter/flutter/issues/159630
//
// The timer below drives that recovery. Two subtleties, verified against the
// embedder sources (identical in 3.24.5 and 3.44.0):
// - FlutterViewController::ForceRedraw() only schedules a frame when NO resize
//   is pending (resize_status_ == kDone), so it cannot heal the wedge above.
//   It is kept as a cheap first kick for the case it was designed for: a
//   window created hidden and shown later, with nothing scheduling a frame.
// - The SetNextFrameCallback used to detect the first frame fires when a frame
//   is GENERATED (raster thread), even if the resize gate then rejects its
//   present. So it must not be the only stop condition: one final
//   ForceChildRefresh() is issued to guarantee a present at the current size.
//   Note this premise is not load-bearing, and the redundancy is deliberate:
//   if the callback in fact only fired on a successful present, then
//   first_frame_rendered_ would stay false and the timer below would keep
//   nudging until it healed.
// This also relies on HandleTopLevelWindowProc not consuming WM_TIMER (no
// plugin registers a delegate for it today).
constexpr UINT_PTR kForceRedrawTimerId = 0xFB15;
constexpr UINT kForceRedrawIntervalMs = 200;
// Give up eventually (with a log), so a genuinely stuck engine doesn't keep a
// timer alive forever. 25 * 200ms covers slow starts comfortably.
constexpr UINT kForceRedrawMaxTries = 25;
// The first ticks use the cheap ForceRedraw(); later ticks use
// ForceChildRefresh(), which may block the platform thread for up to 2x100ms
// per call (each nudge re-enters the 100ms resize wait).
constexpr UINT kForceRedrawCheapTries = 2;

// Re-enters the embedder's OnWindowSizeChanged by nudging the Flutter child
// window by 1px and back: this resets the resize target and resends the window
// metrics. Same as BaseFlutterWindow::ForceChildRefresh() on the
// rustdesk_desktop_multi_window side.
void ForceChildRefresh(HWND child) {
  if (!child) {
    return;
  }
  RECT rect;
  GetWindowRect(child, &rect);
  LONG width = rect.right - rect.left;
  LONG height = rect.bottom - rect.top;
  SetWindowPos(child, nullptr, 0, 0, width + 1, height,
               SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_FRAMECHANGED);
  SetWindowPos(child, nullptr, 0, 0, width, height,
               SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_FRAMECHANGED);
}

}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());

  flutter::MethodChannel<> channel(
    flutter_controller_->engine()->messenger(),
    "org.rustdesk.rustdesk/host",
    &flutter::StandardMethodCodec::GetInstance());

  channel.SetMethodCallHandler(
    [](const flutter::MethodCall<>& call, std::unique_ptr<flutter::MethodResult<>> result) {
      if (call.method_name() == "bumpMouse") {
        auto arguments = call.arguments();

        int dx = 0, dy = 0;

        if (std::holds_alternative<flutter::EncodableMap>(*arguments)) {
          auto argsMap = std::get<flutter::EncodableMap>(*arguments);

          auto dxIt = argsMap.find(flutter::EncodableValue("dx"));
          auto dyIt = argsMap.find(flutter::EncodableValue("dy"));

          if ((dxIt != argsMap.end()) && std::holds_alternative<int>(dxIt->second)) {
            dx = std::get<int>(dxIt->second);
          }
          if ((dyIt != argsMap.end()) && std::holds_alternative<int>(dyIt->second)) {
            dy = std::get<int>(dyIt->second);
          }
        } else if (std::holds_alternative<flutter::EncodableList>(*arguments)) {
          auto argsList = std::get<flutter::EncodableList>(*arguments);

          if ((argsList.size() >= 1) && std::holds_alternative<int>(argsList[0])) {
            dx = std::get<int>(argsList[0]);
          }
          if ((argsList.size() >= 2) && std::holds_alternative<int>(argsList[1])) {
            dy = std::get<int>(argsList[1]);
          }
        }

        bool succeeded = Win32Desktop::BumpMouse(dx, dy);

        result->Success(succeeded);
      }
    });

  DesktopMultiWindowSetWindowCreatedCallback([](void *controller) {
    auto *flutter_view_controller =
        reinterpret_cast<flutter::FlutterViewController *>(controller);
    auto *registry = flutter_view_controller->engine();
    TextureRgbaRendererPluginCApiRegisterWithRegistrar(
        registry->GetRegistrarForPlugin("TextureRgbaRendererPlugin"));
    FlutterGpuTextureRendererPluginCApiRegisterWithRegistrar(
        registry->GetRegistrarForPlugin("FlutterGpuTextureRendererPluginCApi"));
  });
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  // See the comment on kForceRedrawTimerId above.
  flutter_controller_->engine()->SetNextFrameCallback(
      [this]() { first_frame_rendered_ = true; });
  SetTimer(GetHandle(), kForceRedrawTimerId, kForceRedrawIntervalMs, nullptr);

  return true;
}

void FlutterWindow::OnDestroy() {
  KillTimer(GetHandle(), kForceRedrawTimerId);
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
    case WM_TIMER:
      if (wparam == kForceRedrawTimerId) {
        if (!flutter_controller_) {
          KillTimer(hwnd, kForceRedrawTimerId);
        } else if (first_frame_rendered_) {
          // A frame was generated, which does not mean it was presented: if a
          // resize was pending, the gate rejected it (see the comment on
          // kForceRedrawTimerId). One child refresh guarantees a present at the
          // current size. Unconditional because gating it bought nothing: the
          // WM_SIZE that CreateWindow() sends already arrives before the first
          // frame, so the flag this used to check was always set by the time we
          // got here. Doing it unconditionally is safe either way - at worst it
          // is one extra nudge, and it is cheap once the engine is running.
          ForceChildRefresh(flutter_controller_->view()->GetNativeWindow());
          KillTimer(hwnd, kForceRedrawTimerId);
        } else if (++force_redraw_tries_ > kForceRedrawMaxTries) {
          // Not std::cerr: the runner only attaches a console when started from
          // one or under a debugger (see main.cpp), and this fires on end-user
          // machines. OutputDebugString is readable with DebugView there.
          OutputDebugStringA(
              "rustdesk: Flutter window did not render its first frame, "
              "giving up.\n");
          KillTimer(hwnd, kForceRedrawTimerId);
        } else if (force_redraw_tries_ <= kForceRedrawCheapTries) {
          flutter_controller_->ForceRedraw();
        } else {
          ForceChildRefresh(flutter_controller_->view()->GetNativeWindow());
        }
        return 0;
      }
      break;
    case WM_SHOWWINDOW:
      // A window created hidden (e.g. the connection manager) may be shown
      // long after the creation-time force-redraw timer has given up, and
      // FancyZones moves windows exactly when they are shown. Re-arm the
      // protection if the first frame still hasn't been rendered by now (see
      // kForceRedrawTimerId).
      if (wparam == TRUE && !first_frame_rendered_ && flutter_controller_) {
        force_redraw_tries_ = 0;
        SetTimer(hwnd, kForceRedrawTimerId, kForceRedrawIntervalMs, nullptr);
      }
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
