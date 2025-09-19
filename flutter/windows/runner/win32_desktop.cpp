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

    RECT workAreaRect = {0};
    bool haveWorkAreaRect = false;

    if (hMonitor != NULL)
    {
      MONITORINFO monitorInfo = {0};

      monitorInfo.cbSize = sizeof(monitorInfo);

      if (GetMonitorInfoW(hMonitor, &monitorInfo))
      {
        workAreaRect = monitorInfo.rcWork;
        haveWorkAreaRect = true;
      }
    }

    if (!haveWorkAreaRect)
    {
      // I don't think this is possible, but just in case, some
      // reasonably sane fallbacks.
      workAreaRect.left = 0;
      workAreaRect.top = 0;
      workAreaRect.right = 1280;
      workAreaRect.bottom = 1024 - 40; // default Windows 10 task bar height
    }

    origin.x = workAreaRect.left;
    origin.y = workAreaRect.top;

    size.width = workAreaRect.right - workAreaRect.left;
    size.height = workAreaRect.bottom - workAreaRect.top;
  }

  namespace
  {
    void FitToWorkAreaImpl(Win32Window::Point& origin, Win32Window::Size& size, Win32Window::Point& workarea_origin, Win32Window::Size& workarea_size)
    {
      // Retrieve the work area of the monitor that contains or
      // is closed to the supplied window bounds.
      workarea_origin = origin;
      workarea_size = size;

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

  void FitToWorkArea(Win32Window::Point& origin, Win32Window::Size& size)
  {
    Win32Window::Point workarea_origin(0, 0);
    Win32Window::Size workarea_size(0, 0);

    FitToWorkAreaImpl(origin, size, workarea_origin, workarea_size);
  }

  void CentreInWorkArea(Win32Window::Point& origin, Win32Window::Size& size)
  {
    Win32Window::Point workarea_origin(0, 0);
    Win32Window::Size workarea_size(0, 0);

    FitToWorkAreaImpl(origin, size, workarea_origin, workarea_size);

    Win32Window::Point relative_origin(
      (workarea_size.width - size.width) / 2,
      (workarea_size.height - size.height) / 2);

    origin.x = workarea_origin.x + relative_origin.x;
    origin.y = workarea_origin.y + relative_origin.y;
  }
}
