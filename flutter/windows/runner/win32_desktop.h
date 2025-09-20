#ifndef RUNNER_WIN32_DESKTOP_H_
#define RUNNER_WIN32_DESKTOP_H_

#include "win32_window.h"

namespace Win32Desktop
{
  void GetWorkArea(Win32Window::Point& origin, Win32Window::Size& size);
  void FitToWorkArea(Win32Window::Point& origin, Win32Window::Size& size);
}

#endif  // RUNNER_WIN32_DESKTOP_H_
