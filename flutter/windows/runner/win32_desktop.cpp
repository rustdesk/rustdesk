#include "win32_desktop.h"

#include <windows.h>

namespace Win32Desktop
{
  void GetWorkArea(Win32Window::Point& origin, Win32Window::Size& size)
  {
    RECT workAreaRect;

    if (!SystemParametersInfoA(SPI_GETWORKAREA, 0, &workAreaRect, 0))
    {
      // I don't think this function can fail, but just in case, some
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
}
