//
// Created by yangbin on 2022/1/27.
//

#include "base_flutter_window.h"

#include <dwmapi.h>
#include <iostream>
// #include <shobjidl_core.h>

#pragma comment(lib, "dwmapi.lib")

namespace {
void CenterRectToMonitor(LPRECT prc) {
  HMONITOR hMonitor;
  MONITORINFO mi;
  RECT rc;
  int w = prc->right - prc->left;
  int h = prc->bottom - prc->top;

  //
  // get the nearest monitor to the passed rect.
  //
  hMonitor = MonitorFromRect(prc, MONITOR_DEFAULTTONEAREST);

  //
  // get the work area or entire monitor rect.
  //
  mi.cbSize = sizeof(mi);
  GetMonitorInfo(hMonitor, &mi);

  rc = mi.rcMonitor;

  prc->left = rc.left + (rc.right - rc.left - w) / 2;
  prc->top = rc.top + (rc.bottom - rc.top - h) / 2;
  prc->right = prc->left + w;
  prc->bottom = prc->top + h;

}

std::wstring Utf16FromUtf8(const std::string &string) {
  int size_needed = MultiByteToWideChar(CP_UTF8, 0, string.c_str(), -1, nullptr, 0);
  if (size_needed == 0) {
    return {};
  }
  std::wstring wstrTo(size_needed, 0);
  int converted_length = MultiByteToWideChar(CP_UTF8, 0, string.c_str(), -1, &wstrTo[0], size_needed);
  if (converted_length == 0) {
    return {};
  }
  return wstrTo;
}

}

void BaseFlutterWindow::Center() {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  RECT rc;
  GetWindowRect(handle, &rc);
  CenterRectToMonitor(&rc);
  SetWindowPos(handle, nullptr, rc.left, rc.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
}

void BaseFlutterWindow::Focus() {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  if (IsMinimized()) {
     Restore();
  }
  SetWindowPos(handle, HWND_TOP, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE);
  SetForegroundWindow(handle);
}

void BaseFlutterWindow::SetFullscreen(bool fullscreen) {
    auto window = GetWindowHandle();
    if (!window) {
        return;
    }

    // Inspired by how Chromium does this
    // https://src.chromium.org/viewvc/chrome/trunk/src/ui/views/win/fullscreen_handler.cc?revision=247204&view=markup

    // Save current window state if not already fullscreen.
    if (!g_is_window_fullscreen) {
        // Save current window information.
        g_maximized_before_fullscreen = !!::IsZoomed(window);
        g_style_before_fullscreen = GetWindowLong(window, GWL_STYLE);
        g_ex_style_before_fullscreen = GetWindowLong(window, GWL_EXSTYLE);
        if (g_maximized_before_fullscreen) {
            SendMessage(window, WM_SYSCOMMAND, SC_RESTORE, 0);
        }
        ::GetWindowRect(window, &g_frame_before_fullscreen);
        g_title_bar_style_before_fullscreen = title_bar_style_;
        g_is_frameless_before_fullscreen = is_frameless_;
    }

    if (IsWindowVisible(window) == TRUE) {
      g_style_before_fullscreen = g_style_before_fullscreen | WS_VISIBLE;
    } else {
      g_style_before_fullscreen = g_style_before_fullscreen & ~WS_VISIBLE;
    }

    // this variable should be set before telling windows to change the fullscreen status.
    // Or the right and bottom area would be cut off after cancelling the fullscreen.
    g_is_window_fullscreen = fullscreen;
    if (fullscreen) {
        flutter::EncodableMap args2 = flutter::EncodableMap();
        args2[flutter::EncodableValue("titleBarStyle")] =
            flutter::EncodableValue("normal");
        SetTitleBarStyle(args2);

        // Set new window style and size.
        ::SetWindowLong(window, GWL_STYLE,
            g_style_before_fullscreen & ~(WS_CAPTION | WS_THICKFRAME));
        ::SetWindowLong(window, GWL_EXSTYLE,
            g_ex_style_before_fullscreen &
            ~(WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE |
                WS_EX_CLIENTEDGE | WS_EX_STATICEDGE));

        MONITORINFO monitor_info;
        monitor_info.cbSize = sizeof(monitor_info);
        ::GetMonitorInfo(::MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
            &monitor_info);
        ::SetWindowPos(window, NULL, monitor_info.rcMonitor.left,
            monitor_info.rcMonitor.top,
            monitor_info.rcMonitor.right - monitor_info.rcMonitor.left,
            monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        // ::SendMessage(window, WM_SYSCOMMAND, SC_MAXIMIZE, 0);
    }
    else {
        ::SetWindowLong(window, GWL_STYLE, g_style_before_fullscreen);
        ::SetWindowLong(window, GWL_EXSTYLE, g_ex_style_before_fullscreen);

        // SendMessage(window, WM_SYSCOMMAND, SC_RESTORE, 0);

        if (title_bar_style_ != g_title_bar_style_before_fullscreen) {
            flutter::EncodableMap args2 = flutter::EncodableMap();
            args2[flutter::EncodableValue("titleBarStyle")] =
                flutter::EncodableValue(g_title_bar_style_before_fullscreen);
            SetTitleBarStyle(args2);
        }

        if (g_is_frameless_before_fullscreen)
            SetAsFrameless();

        if (g_maximized_before_fullscreen) {
            flutter::EncodableMap args2 = flutter::EncodableMap();
            args2[flutter::EncodableValue("vertically")] =
                flutter::EncodableValue(false);
            Maximize(args2);
        }
        else {
            ::SetWindowPos(
                window, NULL, g_frame_before_fullscreen.left,
                g_frame_before_fullscreen.top,
                g_frame_before_fullscreen.right - g_frame_before_fullscreen.left,
                g_frame_before_fullscreen.bottom - g_frame_before_fullscreen.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
    }
}

void BaseFlutterWindow::StartDragging() {
    auto window = GetWindowHandle();
    if (!window) {
        return;
    }
    ReleaseCapture();
    SendMessage(window, WM_SYSCOMMAND, SC_MOVE | HTCAPTION, 0);
}

bool BaseFlutterWindow::IsMaximized() {
    auto window = GetWindowHandle();
    if (!window) {
        return false;
    }
    WINDOWPLACEMENT windowPlacement;
    GetWindowPlacement(window, &windowPlacement);

    return windowPlacement.showCmd == SW_SHOWMAXIMIZED;
}

void BaseFlutterWindow::Maximize() {
    auto window = GetWindowHandle();
    if (!window) {
        return;
    }
    WINDOWPLACEMENT windowPlacement;
    GetWindowPlacement(window, &windowPlacement);
    // non vertical now
    if (windowPlacement.showCmd != SW_SHOWMAXIMIZED) {
        PostMessage(window, WM_SYSCOMMAND, SC_MAXIMIZE, 0);
    }
}

void BaseFlutterWindow::Unmaximize() {
    auto window = GetWindowHandle();
    if (!window) {
        return;
    }
    WINDOWPLACEMENT windowPlacement;
    GetWindowPlacement(window, &windowPlacement);

    if (windowPlacement.showCmd != SW_SHOWNORMAL) {
        PostMessage(window, WM_SYSCOMMAND, SC_RESTORE, 0);
    }
}

void BaseFlutterWindow::Minimize() {
    auto window = GetWindowHandle();
    if (!window) {
        return;
    }
    WINDOWPLACEMENT windowPlacement;
    GetWindowPlacement(window, &windowPlacement);

    if (windowPlacement.showCmd != SW_SHOWMINIMIZED) {
        PostMessage(window, WM_SYSCOMMAND, SC_MINIMIZE, 0);
    }
}

bool BaseFlutterWindow::IsMinimized()
{
    auto window = GetWindowHandle();
    if (!window) {
        return false;
    }
    WINDOWPLACEMENT windowPlacement;
    GetWindowPlacement(window, &windowPlacement);

    return windowPlacement.showCmd == SW_SHOWMINIMIZED;
}

void BaseFlutterWindow::ShowTitlebar(bool show) {
    auto window = GetWindowHandle();
    if (!window) {
        return;
    }
    this->title_bar_style_ = show ? "normal" : "hidden";
    this->is_frameless_ = false;

    // if (!show) {
    //     LONG lStyle = GetWindowLong(window, GWL_STYLE);
    //     SetWindowLong(window, GWL_STYLE, lStyle & ~WS_CAPTION);
    //     SetWindowPos(window, NULL, 0, 0, 0, 0, SWP_NOSIZE
    //         | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    // }
    // else {
    //     LONG lStyle = GetWindowLong(window, GWL_STYLE);
    //     SetWindowLong(window, GWL_STYLE, lStyle | WS_CAPTION);
    //     SetWindowPos(window, NULL, 0, 0, 0, 0, SWP_NOSIZE
    //         | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    // }
    MARGINS margins = {0, 0, 0, 0};
    RECT rect;
    GetWindowRect(window, &rect);
    DwmExtendFrameIntoClientArea(window, &margins);
    SetWindowPos(window, nullptr, rect.left, rect.top, 0, 0,
                SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_NOSIZE |
                    SWP_FRAMECHANGED);
}

void BaseFlutterWindow::Maximize(const flutter::EncodableMap& args) {
    bool vertically =
        std::get<bool>(args.at(flutter::EncodableValue("vertically")));

    HWND hwnd = GetWindowHandle();
    WINDOWPLACEMENT windowPlacement;
    GetWindowPlacement(hwnd, &windowPlacement);

    if (vertically) {
        POINT cursorPos;
        GetCursorPos(&cursorPos);
        PostMessage(hwnd, WM_NCLBUTTONDBLCLK, HTTOP,
            MAKELPARAM(cursorPos.x, cursorPos.y));
    }
    else {
        if (windowPlacement.showCmd != SW_SHOWMAXIMIZED) {
            PostMessage(hwnd, WM_SYSCOMMAND, SC_MAXIMIZE, 0);
        }
    }
}

void BaseFlutterWindow::SetTitleBarStyle(const flutter::EncodableMap& args) {
    title_bar_style_ =
        std::get<std::string>(args.at(flutter::EncodableValue("titleBarStyle")));
    // Enables the ability to go from setAsFrameless() to
    // TitleBarStyle.normal/hidden
    is_frameless_ = false;

    MARGINS margins = { 0, 0, 0, 0 };
    HWND hWnd = GetWindowHandle();
    RECT rect;
    GetWindowRect(hWnd, &rect);
    DwmExtendFrameIntoClientArea(hWnd, &margins);
    SetWindowPos(hWnd, nullptr, rect.left, rect.top, 0, 0,
        SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_NOSIZE |
        SWP_FRAMECHANGED);
    // std::cout << "set title bar styled" << std::endl;
}

void BaseFlutterWindow::SetInitBackgroundColor(const flutter::EncodableMap *args) {
  int colorA = std::get<int>(args->at(flutter::EncodableValue("a")));
  int colorR = std::get<int>(args->at(flutter::EncodableValue("r")));
  int colorG = std::get<int>(args->at(flutter::EncodableValue("g")));
  int colorB = std::get<int>(args->at(flutter::EncodableValue("b")));
  bool isTransparent = colorA == 0 && colorR == 0 && colorG == 0 && colorB == 0;
  erase_transparent_ = isTransparent;
  erase_background_color_ = RGB(colorR, colorG, colorB);
}

void BaseFlutterWindow::SetAsFrameless() {
    is_frameless_ = true;
    HWND hWnd = GetWindowHandle();

    RECT rect;

    GetWindowRect(hWnd, &rect);
    SetWindowPos(hWnd, nullptr, rect.left, rect.top, rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_NOSIZE |
        SWP_FRAMECHANGED);
}


void BaseFlutterWindow::Restore() {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  WINDOWPLACEMENT windowPlacement;
  GetWindowPlacement(handle, &windowPlacement);

  if (windowPlacement.showCmd != SW_SHOWNORMAL) {
    PostMessage(handle, WM_SYSCOMMAND, SC_RESTORE, 0);
  }
}

void BaseFlutterWindow::SetBounds(double_t x, double_t y, double_t width, double_t height) {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  // A simple workaround for the first problem https://github.com/rustdesk/rustdesk/issues/5791
  // If is the first call to move window, we do not call ShowWindow(handle, SW_RESTORE), to avoid the blank window.
  if (is_first_move_) {
    is_first_move_ = false;
  } else {
    // We do need the following call or `SetWindowPlacement` to set the window `showCmd` value.
    // MoveWindow will not change the `showCmd` value of `GetWindowPlacement`.
    // So the state of the window will be wrong after the window is maximized or minimized and then moved.
    WINDOWPLACEMENT windowPlacement;
    GetWindowPlacement(handle, &windowPlacement);
    if (windowPlacement.showCmd == SW_SHOWMAXIMIZED || windowPlacement.showCmd == SW_SHOWMINIMIZED) {
      // Both `PostMessage(handle, WM_SYSCOMMAND, SC_RESTORE, 0);` and `ShowWindow(handle, SW_RESTORE);`
      // have a side effect that the window will be show if it is hidden.
      ShowWindow(handle, SW_RESTORE);
    }
  }
  MoveWindow(handle, int32_t(x), int32_t(y),
             static_cast<int>(width),
             static_cast<int>(height),
             TRUE);
}

flutter::EncodableMap BaseFlutterWindow::GetBounds() {
  flutter::EncodableMap resultMap = flutter::EncodableMap();
  auto handle = GetWindowHandle();
  if (handle) {
    RECT rect;
    if (GetWindowRect(handle, &rect)) {
      double x = rect.left;
      double y = rect.top;
      double width = (rect.right - rect.left);
      double height = (rect.bottom - rect.top);
      resultMap[flutter::EncodableValue("x")] = flutter::EncodableValue(x);
      resultMap[flutter::EncodableValue("y")] = flutter::EncodableValue(y);
      resultMap[flutter::EncodableValue("width")] =
          flutter::EncodableValue(width);
      resultMap[flutter::EncodableValue("height")] =
          flutter::EncodableValue(height);
    }
  }
  return resultMap;
}

void BaseFlutterWindow::SetTitle(const std::string &title) {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  SetWindowText(handle, Utf16FromUtf8(title).c_str());
}

void BaseFlutterWindow::Close() {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  PostMessage(handle, WM_SYSCOMMAND, SC_CLOSE, 0);
}

void BaseFlutterWindow::Show() {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  ShowWindowAsync(handle, SW_SHOW);
  SetForegroundWindow(handle);
}

void BaseFlutterWindow::Hide() {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  ShowWindow(handle, SW_HIDE);
}

bool BaseFlutterWindow::IsHidden() {
    auto window = GetWindowHandle();
    if (!window) {
        return false;
    }
    return IsWindowVisible(window) != TRUE;
}

void BaseFlutterWindow::StartResizing(const flutter::EncodableMap *param) {
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }

  bool top = std::get<bool>(param->at(flutter::EncodableValue("top")));
  bool bottom = std::get<bool>(param->at(flutter::EncodableValue("bottom")));
  bool left = std::get<bool>(param->at(flutter::EncodableValue("left")));
  bool right = std::get<bool>(param->at(flutter::EncodableValue("right")));

  ReleaseCapture();
  LONG command;
  if (top && !bottom && !right && !left) {
    command = HTTOP;
  } else if (top && left && !bottom && !right) {
    command = HTTOPLEFT;
  } else if (left && !top && !bottom && !right) {
    command = HTLEFT;
  } else if (right && !top && !left && !bottom) {
    command = HTRIGHT;
  } else if (top && right && !left && !bottom) {
    command = HTTOPRIGHT;
  } else if (bottom && !top && !right && !left) {
    command = HTBOTTOM;
  } else if (bottom && left && !top && !right) {
    command = HTBOTTOMLEFT;
  } else
    command = HTBOTTOMRIGHT;
  POINT cursorPos;
  GetCursorPos(&cursorPos);
  PostMessage(handle, WM_NCLBUTTONDOWN, command,
              MAKELPARAM(cursorPos.x, cursorPos.y));
}


bool BaseFlutterWindow::IsPreventClose() {
  return this->is_prevent_close_;
}

void BaseFlutterWindow::SetPreventClose(bool setPreventClose) {
  this->is_prevent_close_ = setPreventClose;
}

bool BaseFlutterWindow::IsFrameless()
{
    return is_frameless_;
}


void BaseFlutterWindow::ForceChildRefresh() {
  constexpr int kNonRenderingWidth = 0;
  auto handle = GetWindowHandle();
  if (!handle) {
    return;
  }
  handle = GetWindow(handle, GW_CHILD);
  // FancyZones can queue a hide before the Flutter view is reparented.
  if ((GetWindowLongPtr(handle, GWL_STYLE) & WS_VISIBLE) == 0) {
    ShowWindow(handle, SW_SHOWNA);
  }
  RECT rect;
  GetWindowRect(handle, &rect);
  // A nonzero intermediate width can time out before Flutter resizes its
  // surface. Restoring its old width then leaves that intermediate resize
  // pending. Zero width sends metrics without creating a new resize target.
  SetWindowPos(
      handle, nullptr, rect.left, rect.top, kNonRenderingWidth,
      rect.bottom - rect.top,
      SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_FRAMECHANGED);
  SetWindowPos(
      handle, nullptr, rect.left, rect.top, rect.right - rect.left,
      rect.bottom - rect.top,
      SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_FRAMECHANGED);
}

bool BaseFlutterWindow::IsFullscreen() { return g_is_window_fullscreen; }
