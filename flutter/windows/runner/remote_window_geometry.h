#ifndef RUNNER_REMOTE_WINDOW_GEOMETRY_H_
#define RUNNER_REMOTE_WINDOW_GEOMETRY_H_

#include <windows.h>
#include <algorithm>
#include <cmath>

inline void ConstrainRemoteWindow(RECT& rect, WPARAM edge, double ratio,
                                  LONG extra_width, LONG extra_height,
                                  LONG min_width, LONG min_height) {
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
}

#endif
