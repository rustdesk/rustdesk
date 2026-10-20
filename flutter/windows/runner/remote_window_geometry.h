#ifndef RUNNER_REMOTE_WINDOW_GEOMETRY_H_
#define RUNNER_REMOTE_WINDOW_GEOMETRY_H_

#include <windows.h>
#include <algorithm>
#include <cmath>

inline int RemoteResizeHitTest(int hit, const RECT& frame, POINT pointer) {
  const bool upper = pointer.y < frame.top + (frame.bottom - frame.top) / 2;
  const bool left = pointer.x < frame.left + (frame.right - frame.left) / 2;
  switch (hit) {
    case HTLEFT: return upper ? HTTOPLEFT : HTBOTTOMLEFT;
    case HTRIGHT: return upper ? HTTOPRIGHT : HTBOTTOMRIGHT;
    case HTTOP: return left ? HTTOPLEFT : HTTOPRIGHT;
    case HTBOTTOM: return left ? HTBOTTOMLEFT : HTBOTTOMRIGHT;
    default: return hit;
  }
}

inline const RECT* RemoteResizeWorkArea(const RECT& outer, const RECT* bounds) {
  return bounds && outer.left >= bounds->left && outer.top >= bounds->top &&
          outer.right <= bounds->right && outer.bottom <= bounds->bottom
      ? bounds : nullptr;
}

inline void ConstrainRemoteWindow(RECT& rect, WPARAM edge, double ratio,
                                  LONG extra_width, LONG extra_height,
                                  LONG min_width, LONG min_height,
                                  const RECT* bounds = nullptr) {
  const bool corner = edge >= WMSZ_TOPLEFT && edge != WMSZ_BOTTOM;
  double width = rect.right - rect.left - extra_width;
  const double requested_height = rect.bottom - rect.top - extra_height;
  if (corner) {
    // Project the dragged corner onto the aspect-ratio diagonal without switching axes.
    width = (width * ratio + requested_height) / (ratio + 1.0 / ratio);
  } else if (edge == WMSZ_TOP || edge == WMSZ_BOTTOM) {
    width = requested_height * ratio;
  }
  width = std::max(width, std::max(1.0, std::max(
      static_cast<double>(min_width - extra_width),
      (min_height - extra_height) * ratio)));
  if (bounds) {
    const double available_width = bounds->right - bounds->left - extra_width;
    const double available_height = bounds->bottom - bounds->top - extra_height;
    if (available_width > 0 && available_height > 0) {
      width = std::min(width, std::min(available_width, available_height * ratio));
    }
  }
  const LONG height = static_cast<LONG>(std::lround(width / ratio)) + extra_height;
  const LONG outer_width = static_cast<LONG>(std::lround(width)) + extra_width;
  if (edge == WMSZ_LEFT || edge == WMSZ_TOPLEFT || edge == WMSZ_BOTTOMLEFT) {
    rect.left = rect.right - outer_width;
  } else if (edge == WMSZ_TOP || edge == WMSZ_BOTTOM) {
    rect.left += (rect.right - rect.left - outer_width) / 2;
    rect.right = rect.left + outer_width;
  } else {
    rect.right = rect.left + outer_width;
  }
  if (edge == WMSZ_TOP || edge == WMSZ_TOPLEFT || edge == WMSZ_TOPRIGHT) {
    rect.top = rect.bottom - height;
  } else if (edge == WMSZ_LEFT || edge == WMSZ_RIGHT) {
    rect.top += (rect.bottom - rect.top - height) / 2;
    rect.bottom = rect.top + height;
  } else {
    rect.bottom = rect.top + height;
  }
  if (bounds) {
    // Once a moving edge reaches the monitor, let the opposite edge move instead.
    rect.left = std::max(bounds->left, std::min(rect.left, bounds->right - outer_width));
    rect.top = std::max(bounds->top, std::min(rect.top, bounds->bottom - height));
    rect.right = rect.left + outer_width;
    rect.bottom = rect.top + height;
  }
}

#endif
