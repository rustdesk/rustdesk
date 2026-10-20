// Keep assertions active in the Release configuration used by CI.
#ifdef NDEBUG
#undef NDEBUG
#endif

#include "../remote_window_geometry.h"
#include <cassert>

int main() {
  // Every edge and corner must fit the remote content, including local chrome.
  for (WPARAM edge = WMSZ_LEFT; edge <= WMSZ_BOTTOMRIGHT; ++edge) {
    RECT rect{100, 200, 1116, 840};
    const RECT before = rect;
    ConstrainRemoteWindow(rect, edge, 16.0 / 9.0, 16, 40, 300, 200);
    const double width = rect.right - rect.left - 16;
    const double height = rect.bottom - rect.top - 40;
    assert(std::abs(height - width * 9.0 / 16.0) <= 1);
    if (edge == WMSZ_LEFT || edge == WMSZ_TOPLEFT || edge == WMSZ_BOTTOMLEFT)
      assert(rect.right == before.right);
    if (edge == WMSZ_RIGHT || edge == WMSZ_TOPRIGHT || edge == WMSZ_BOTTOMRIGHT)
      assert(rect.left == before.left);
    if (edge == WMSZ_TOP || edge == WMSZ_TOPLEFT || edge == WMSZ_TOPRIGHT)
      assert(rect.bottom == before.bottom);
    if (edge == WMSZ_BOTTOM || edge == WMSZ_BOTTOMLEFT || edge == WMSZ_BOTTOMRIGHT)
      assert(rect.top == before.top);
  }
  // Corners also follow a predominantly vertical drag.
  for (WPARAM edge : {WMSZ_TOPLEFT, WMSZ_TOPRIGHT, WMSZ_BOTTOMLEFT, WMSZ_BOTTOMRIGHT}) {
    const RECT current{0, 0, 1016, 603};
    RECT resized{0, 0, 1016, 800};
    ConstrainRemoteWindow(resized, edge, 16.0 / 9.0, 16, 40, 300, 200);
    assert(resized.bottom - resized.top > current.bottom - current.top);
    assert(resized.right - resized.left > current.right - current.left);
    assert(std::abs((resized.bottom - resized.top - 40) -
                    (resized.right - resized.left - 16) * 9.0 / 16.0) <= 1);
    if (edge == WMSZ_TOPRIGHT || edge == WMSZ_BOTTOMRIGHT)
      assert(resized.left == current.left);
    else
      assert(resized.right == current.right);
  }
  // Small corner pointer movements must not jump between opposite scale changes.
  {
    RECT first{0, 0, 1100, 449};
    RECT second{0, 0, 1100, 451};
    ConstrainRemoteWindow(first, WMSZ_BOTTOMRIGHT, 2.0, 0, 0, 100, 100);
    ConstrainRemoteWindow(second, WMSZ_BOTTOMRIGHT, 2.0, 0, 0, 100, 100);
    assert(std::abs(second.right - first.right) <= 2);
    assert(std::abs(second.bottom - first.bottom) <= 2);
  }
  // Portrait content still fits when Windows' minimum dimensions stop a drag.
  RECT rect{0, 0, 100, 100};
  ConstrainRemoteWindow(rect, WMSZ_BOTTOMRIGHT, 9.0 / 16.0, 24, 60, 400, 300);
  assert(rect.right >= 400 && rect.bottom >= 300);
  assert(std::abs((rect.bottom - 60) - (rect.right - 24) * 16.0 / 9.0) <= 1);
}
