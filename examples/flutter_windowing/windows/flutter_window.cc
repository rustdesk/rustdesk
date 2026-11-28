//
// Created by yangbin on 2022/1/11.
//

#include "flutter_window.h"

#include "flutter_windows.h"

#include "tchar.h"

#include "resource.h"

#include <iostream>
#include <string>
#include <utility>

#include "include/desktop_multi_window/desktop_multi_window_plugin.h"
#include "multi_window_plugin_internal.h"

/// RustDesk deps using method channel
// #include <bitsdojo_window_windows/bitsdojo_window_plugin.h>
#include <url_launcher_windows/url_launcher_windows.h>
#include <window_size/window_size_plugin.h>
#include <texture_rgba_renderer/texture_rgba_renderer_plugin_c_api.h>
// #include <window_manager/window_manager_plugin.h>
// #include <screen_retriever/screen_retriever_plugin.h>
// #include <tray_manager/tray_manager_plugin.h>

void RustDeskRegisterPlugins(flutter::PluginRegistry* registry) {
    // BitsdojoWindowPluginRegisterWithRegistrar(
    //    registry->GetRegistrarForPlugin("BitsdojoWindowPlugin"));
    UrlLauncherWindowsRegisterWithRegistrar(
        registry->GetRegistrarForPlugin("UrlLauncherWindows"));
    WindowSizePluginRegisterWithRegistrar(registry->GetRegistrarForPlugin("WindowSizePlugin"));
    TextureRgbaRendererPluginCApiRegisterWithRegistrar(registry->GetRegistrarForPlugin("TextureRgbaRendererPlugin"));
    // WindowManagerPluginRegisterWithRegistrar(
    //     registry->GetRegistrarForPlugin("WindowManagerPlugin"));
    // ScreenRetrieverPluginRegisterWithRegistrar(
    //   registry->GetRegistrarForPlugin("ScreenRetrieverPlugin"));
    // TrayManagerPluginRegisterWithRegistrar(
    //  registry->GetRegistrarForPlugin("TrayManagerPlugin"));
}

bool IsWindows11OrGreater() {
  DWORD dwVersion = 0;
  DWORD dwBuild = 0;

#pragma warning(push)
#pragma warning(disable : 4996)
  dwVersion = GetVersion();
  // Get the build number.
  if (dwVersion < 0x80000000)
    dwBuild = (DWORD)(HIWORD(dwVersion));
#pragma warning(pop)

  return dwBuild < 22000;
}

namespace {

// If the window is resized between the creation of the Flutter surface and the
// present of the first frame - which is what the PowerToys FancyZones option
// "Move newly created windows to their last known zone" does - the embedder's
// resize synchronization enters kResizeStarted and from then on only presents
// frames that match the new size. A frame already generated for the old size
// is rejected, nothing schedules a matching one, and the window stays white
// until a real resize re-enters OnWindowSizeChanged, which resets the resize
// target and resends the window metrics. That is why minimize/restore heals
// it; ForceChildRefresh() does the same programmatically.
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

WindowCreatedCallback _g_window_created_callback = nullptr;

TCHAR kFlutterWindowClassName[] = _T("RustdeskMultiWindow");

int32_t class_registered_ = 0;

void RegisterWindowClass(WNDPROC wnd_proc) {
  if (class_registered_ == 0) {
    WNDCLASS window_class{};
    window_class.hCursor = LoadCursor(nullptr, IDC_ARROW);
    window_class.lpszClassName = kFlutterWindowClassName;
    window_class.style = CS_HREDRAW | CS_VREDRAW;
    window_class.cbClsExtra = 0;
    window_class.cbWndExtra = 0;
    window_class.hInstance = GetModuleHandle(nullptr);
    window_class.hIcon =
        LoadIcon(window_class.hInstance, MAKEINTRESOURCE(IDI_APP_ICON));
    window_class.lpszMenuName = nullptr;
    window_class.lpfnWndProc = wnd_proc;
    window_class.hbrBackground = NULL;
    RegisterClass(&window_class);
  }
  class_registered_++;
}

void UnregisterWindowClass() {
  class_registered_--;
  if (class_registered_ != 0) {
    return;
  }
  UnregisterClass(kFlutterWindowClassName, nullptr);
}

// Scale helper to convert logical scaler values to physical using passed in
// scale factor
inline int Scale(int source, double scale_factor) {
  return static_cast<int>(source * scale_factor);
}

using EnableNonClientDpiScaling = BOOL __stdcall(HWND hwnd);

// Dynamically loads the |EnableNonClientDpiScaling| from the User32 module.
// This API is only needed for PerMonitor V1 awareness mode.
void EnableFullDpiSupportIfAvailable(HWND hwnd) {
  HMODULE user32_module = LoadLibraryA("User32.dll");
  if (!user32_module) {
    return;
  }
  auto enable_non_client_dpi_scaling =
      reinterpret_cast<EnableNonClientDpiScaling *>(
          GetProcAddress(user32_module, "EnableNonClientDpiScaling"));
  if (enable_non_client_dpi_scaling != nullptr) {
    enable_non_client_dpi_scaling(hwnd);
    FreeLibrary(user32_module);
  }
}

}

FlutterWindow::FlutterWindow(
    HWND parent,
    int64_t id,
    std::string args,
    const std::shared_ptr<FlutterWindowCallback> &callback
) : callback_(callback), id_(id), window_handle_(nullptr), scale_factor_(1) {
  RegisterWindowClass(FlutterWindow::WndProc);

  const POINT target_point = {static_cast<LONG>(10),
                              static_cast<LONG>(10)};
  HMONITOR monitor = MonitorFromPoint(target_point, MONITOR_DEFAULTTONEAREST);
  UINT dpi = FlutterDesktopGetDpiForMonitor(monitor);
  scale_factor_ = dpi / 96.0;

  HWND window_handle = CreateWindow(
      kFlutterWindowClassName, L"", WS_OVERLAPPEDWINDOW,
      Scale(target_point.x, scale_factor_), Scale(target_point.y, scale_factor_),
      Scale(1280, scale_factor_), Scale(720, scale_factor_),
      nullptr, nullptr, GetModuleHandle(nullptr), this);

  RECT frame;
  GetClientRect(window_handle, &frame);
  flutter::DartProject project(L"data");
  project.set_dart_entrypoint_arguments({"multi_window", std::to_string(id), std::move(args)});
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    std::cerr << "Failed to setup FlutterViewController." << std::endl;
  }
  auto view_handle = flutter_controller_->view()->GetNativeWindow();
  SetParent(view_handle, window_handle);
  MoveWindow(view_handle, 0, 0, frame.right - frame.left, frame.bottom - frame.top, true);

  RustDeskRegisterPlugins(flutter_controller_->engine());
  InternalMultiWindowPluginRegisterWithRegistrar(
      flutter_controller_->engine()->GetRegistrarForPlugin("DesktopMultiWindowPlugin"));
  window_channel_ = WindowChannel::RegisterWithRegistrar(
      flutter_controller_->engine()->GetRegistrarForPlugin("DesktopMultiWindowPlugin"), id_);

  if (_g_window_created_callback) {
    _g_window_created_callback(flutter_controller_.get());
  }

  // See the comment on kForceRedrawTimerId above.
  flutter_controller_->engine()->SetNextFrameCallback(
      [this]() { first_frame_rendered_ = true; });
  SetTimer(window_handle, kForceRedrawTimerId, kForceRedrawIntervalMs, nullptr);

  // hide the window when created.
  ShowWindow(window_handle, SW_HIDE);
}

// static
FlutterWindow *FlutterWindow::GetThisFromHandle(HWND window) noexcept {
  return reinterpret_cast<FlutterWindow *>(
      GetWindowLongPtr(window, GWLP_USERDATA));
}

// static
LRESULT CALLBACK FlutterWindow::WndProc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
  if (message == WM_NCCREATE) {
    auto window_struct = reinterpret_cast<CREATESTRUCT *>(lparam);
    SetWindowLongPtr(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(window_struct->lpCreateParams));

    auto that = static_cast<FlutterWindow *>(window_struct->lpCreateParams);
    EnableFullDpiSupportIfAvailable(window);
    that->window_handle_ = window;
  } else if (FlutterWindow *that = GetThisFromHandle(window)) {
    return that->MessageHandler(window, message, wparam, lparam);
  }

  return DefWindowProc(window, message, wparam, lparam);
}

LRESULT FlutterWindow::MessageHandler(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result = flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam, lparam);
    if (result) {
      return *result;
    }
  }

  auto child_content_ = flutter_controller_ ? flutter_controller_->view()->GetNativeWindow() : nullptr;

  switch (message) {
    case WM_NCCALCSIZE: {
        // This must always be first or else the one of other two ifs will execute
        //  when window is in full screen and we don't want that
        if (wparam && IsFullscreen()) {
            // Note:
            // I dont know why we should -3 on the bottom.
            //
            // NCCALCSIZE_PARAMS* sz = reinterpret_cast<NCCALCSIZE_PARAMS*>(lparam);
            // sz->rgrc[0].bottom -= 3;
            return 0;
        }
        // This must always be before handling title_bar_style_ == "hidden" so
        //  the if TitleBarStyle.hidden doesn't get executed.
        if (wparam && IsFrameless()) {
            // Add borders when maximized so app doesn't get cut off.
            if (IsMaximized()) {
                adjustNCCALCSIZE(hwnd, reinterpret_cast<NCCALCSIZE_PARAMS*>(lparam));
            }
            // This cuts the app at the bottom by one pixel but that's necessary to
            // prevent jitter when resizing the app
            NCCALCSIZE_PARAMS* sz = reinterpret_cast<NCCALCSIZE_PARAMS*>(lparam);
            sz->rgrc[0].bottom += 1;
            return 0;
        }
        if (wparam && this->title_bar_style_ == "hidden") {
            // Add 8 pixel to the top border when maximized so the app isn't cut off
            if (this->IsMaximized()) {
                adjustNCCALCSIZE(hwnd, reinterpret_cast<NCCALCSIZE_PARAMS*>(lparam));
            }
            else {
                NCCALCSIZE_PARAMS* sz = reinterpret_cast<NCCALCSIZE_PARAMS*>(lparam);
                // on windows 10, if set to 0, there's a white line at the top
                // of the app and I've yet to find a way to remove that.
                sz->rgrc[0].top += IsWindows11OrGreater() ? 0 : 1;
                // We need the following code to resize the window.
                // https://github.com/rustdesk/rustdesk/discussions/9061
                sz->rgrc[0].right -= 8;
                sz->rgrc[0].bottom -= 8;
                sz->rgrc[0].left -= -8;
            }

            // Previously (WVR_HREDRAW | WVR_VREDRAW), but returning 0 or 1 doesn't
            // actually break anything so I've set it to 0. Unless someone pointed a
            // problem in the future.
            return 0;
        }
        break;
    }
    case WM_SHOWWINDOW: {
      if (wparam == TRUE) {
        // The window is created hidden and shown by the Dart side later, which
        // may be long after the creation-time force-redraw timer has given up,
        // and FancyZones moves windows exactly when they are shown. Re-arm the
        // protection on every show because a frame generated while hidden may
        // not have been presented (see kForceRedrawTimerId).
        if (flutter_controller_) {
          force_redraw_tries_ = 0;
          SetTimer(hwnd, kForceRedrawTimerId, kForceRedrawIntervalMs, nullptr);
        }
        EmitEvent("show");
      } else {
        EmitEvent("hide");
      }
      break;
    }
    case WM_FONTCHANGE: {
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
    }
    case WM_TIMER: {
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
          ForceChildRefresh();
          KillTimer(hwnd, kForceRedrawTimerId);
        } else if (++force_redraw_tries_ > kForceRedrawMaxTries) {
          // Not std::cerr: the host process only has a console when started
          // from one or under a debugger, and this fires on end-user machines.
          // OutputDebugString is readable with DebugView there.
          // log_message, not message: that would shadow the MessageHandler
          // parameter, which MSVC flags as C4457 and /WX makes fatal.
          const std::string log_message =
              "rustdesk: Flutter window " + std::to_string(id_) +
              " did not render its first frame, giving up.\n";
          OutputDebugStringA(log_message.c_str());
          KillTimer(hwnd, kForceRedrawTimerId);
        } else if (force_redraw_tries_ <= kForceRedrawCheapTries) {
          flutter_controller_->ForceRedraw();
        } else {
          ForceChildRefresh();
        }
        return 0;
      }
      break;
    }
    case WM_DESTROY:
      // prevent crash
      if (!destroyed_) {
        destroyed_ = true;
        // Give onDestroy callback to Flutter to close window gracefully
        tryInvokeChannelOnDestroy();
        if (auto callback = callback_.lock()) {
          callback->OnWindowDestroy(id_);
        }
      }
      return 0;
    case WM_CLOSE: {
      EmitEvent("close");
      if (this->IsPreventClose()) {
        return -1;
      }
      if (auto callback = callback_.lock()) {
        callback->OnWindowClose(id_);
      }
      break;
    }
    case WM_DPICHANGED: {
      auto newRectSize = reinterpret_cast<RECT *>(lparam);
      LONG newWidth = newRectSize->right - newRectSize->left;
      LONG newHeight = newRectSize->bottom - newRectSize->top;

      SetWindowPos(hwnd, nullptr, newRectSize->left, newRectSize->top, newWidth,
                   newHeight, SWP_NOZORDER | SWP_NOACTIVATE);

      ForceChildRefresh();

      return 0;
    }
    case WM_SIZE: {
      RECT rect;
      GetClientRect(window_handle_, &rect);
      if (child_content_ != nullptr) {
        // Size and position the child window.
        MoveWindow(child_content_, rect.left, rect.top, rect.right - rect.left,
                   rect.bottom - rect.top, TRUE);
      }
      LONG_PTR gwlStyle =
          GetWindowLongPtr(window_handle_, GWL_STYLE);
      if ((gwlStyle & (WS_CAPTION | WS_THICKFRAME)) == 0 &&
          wparam == SIZE_MAXIMIZED) {
          EmitEvent("enter-full-screen");
          this->last_state = STATE_FULLSCREEN_ENTERED;
      }
      else if (this->last_state == STATE_FULLSCREEN_ENTERED &&
          wparam == SIZE_RESTORED) {
          ForceChildRefresh();
          EmitEvent("leave-full-screen");
          last_state = STATE_NORMAL;
      }
      else if (wparam == SIZE_MAXIMIZED) {
          EmitEvent("maximize");
          last_state = STATE_MAXIMIZED;
      }
      else if (wparam == SIZE_MINIMIZED) {
          EmitEvent("minimize");
          last_state = STATE_MINIMIZED;
      }
      else if (wparam == SIZE_RESTORED) {
          if (last_state == STATE_MAXIMIZED) {
              EmitEvent("unmaximize");
              last_state = STATE_NORMAL;
          }
          else if (last_state == STATE_MINIMIZED) {
              EmitEvent("restore");
              last_state = STATE_NORMAL;
          }
      }
      EmitEvent("resized");
      break;
    }

    case WM_MOVE:
      EmitEvent("moved");
      break;

    case WM_ACTIVATE: {
      if (child_content_ != nullptr) {
        SetFocus(child_content_);
      }
      return 0;
    }
    case WM_SIZING: {
        EmitEvent("resize");
        break;
    }

    case WM_MOVING: {
        EmitEvent("move");
        break;
    }
    case WM_NCACTIVATE: {
        char* eventName;
        if (wparam == TRUE) {
            eventName = "focus";
        }
        else {
            eventName = "blur";
        }
        EmitEvent(eventName);
        break;
    }
    case WM_ERASEBKGND: {
        if(IsEraseTransparent()) break;
        HDC hdc = (HDC) wparam;
        HBRUSH brush = CreateSolidBrush(GetEraseBackgroundColor());
        RECT rect;
        GetClientRect(hwnd, &rect);
        FillRect(hdc, &rect, brush);
        DeleteObject(brush);
        return 1; // Background has been erased
    }
    default: break;
  }

  return DefWindowProc(window_handle_, message, wparam, lparam);
}

void FlutterWindow::tryInvokeChannelOnDestroy()
{
  if (window_channel_) {
      auto args = flutter::EncodableValue(flutter::EncodableMap());
      window_channel_->InvokeMethod(0, "onDestroy", &args);
      window_channel_->SetMethodCallHandler(nullptr);
      window_channel_.reset();
  }
}

void FlutterWindow::EmitEvent(const char* eventName)
{
    auto params = flutter::EncodableMap();
    params.emplace(flutter::EncodableValue("eventName"), flutter::EncodableValue(eventName));
    auto args = flutter::EncodableValue(std::move(params));
    window_channel_->InvokeMethod(0, "onEvent", &args);
}

void FlutterWindow::Destroy() {
  if (window_handle_) {
    KillTimer(window_handle_, kForceRedrawTimerId);
  }
  tryInvokeChannelOnDestroy();
  if (window_channel_) {
    window_channel_ = nullptr;
  }
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }
  if (window_handle_) {
    DestroyWindow(window_handle_);
    window_handle_ = nullptr;
  }
}

FlutterWindow::~FlutterWindow() {
  this->Destroy();
  if (window_handle_) {
    std::cout << "window_handle leak." << std::endl;
  }
  UnregisterWindowClass();
}

void DesktopMultiWindowSetWindowCreatedCallback(WindowCreatedCallback callback) {
  _g_window_created_callback = callback;
}
