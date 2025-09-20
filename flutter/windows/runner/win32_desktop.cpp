#include "win32_desktop.h"

#include <windows.h>

#include <algorithm>

namespace Win32Desktop
{
  void GetWorkArea(Win32Window::Point& origin, Win32Window::Size& size)
  {
    RECT windowRect;

    windowRect.left = origin.x;
    windowRect.top = origin.y;
    windowRect.right = origin.x + size.width;
    windowRect.bottom = origin.y + size.height;

    HMONITOR hMonitor = MonitorFromRect(&windowRect, MONITOR_DEFAULTTONEAREST);

    if (hMonitor == NULL)
      hMonitor = MonitorFromWindow(NULL, MONITOR_DEFAULTTOPRIMARY);

    RECT workAreaRect;
    workAreaRect.left = 0;
    workAreaRect.top = 0;
    workAreaRect.right = 1280;
    workAreaRect.bottom = 1024 - 40; // default Windows 10 task bar height

    if (hMonitor != NULL)
    {
      MONITORINFO monitorInfo = {0};

      monitorInfo.cbSize = sizeof(monitorInfo);

      if (GetMonitorInfoW(hMonitor, &monitorInfo))
      {
        workAreaRect = monitorInfo.rcWork;
      }
    }

    origin.x = workAreaRect.left;
    origin.y = workAreaRect.top;

    size.width = workAreaRect.right - workAreaRect.left;
    size.height = workAreaRect.bottom - workAreaRect.top;
  }

  void FitToWorkArea(Win32Window::Point& origin, Win32Window::Size& size)
  {
    // Retrieve the work area of the monitor that contains or
    // is closed to the supplied window bounds.
    Win32Window::Point workarea_origin = origin;
    Win32Window::Size workarea_size = size;

    GetWorkArea(workarea_origin, workarea_size);

    // Translate the window so that its top/left is inside the work area.
    origin.x = std::max(origin.x, workarea_origin.x);
    origin.y = std::max(origin.y, workarea_origin.y);

    // Crop the window if it extends past the bottom/right of the work area.
    Win32Window::Point workarea_bottom_right(
      workarea_origin.x + workarea_size.width,
      workarea_origin.y + workarea_size.height);

    size.width = std::min(size.width, workarea_bottom_right.x - origin.x);
    size.height = std::min(size.height, workarea_bottom_right.y - origin.y);
  }
}
