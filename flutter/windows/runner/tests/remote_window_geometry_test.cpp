// Keep assertions active in the Release configuration used by CI.
#ifdef NDEBUG
#undef NDEBUG
#endif

#include "../remote_window_geometry.h"
#include <cassert>

int main() {
  const RECT frame{100, 200, 1100, 800};
  assert(RemoteResizeHitTest(HTLEFT, frame, {100, 350}) == HTTOPLEFT);
  assert(RemoteResizeHitTest(HTLEFT, frame, {100, 650}) == HTBOTTOMLEFT);
  assert(RemoteResizeHitTest(HTRIGHT, frame, {1100, 350}) == HTTOPRIGHT);
  assert(RemoteResizeHitTest(HTRIGHT, frame, {1100, 650}) == HTBOTTOMRIGHT);
  assert(RemoteResizeHitTest(HTTOP, frame, {350, 200}) == HTTOPLEFT);
  assert(RemoteResizeHitTest(HTTOP, frame, {850, 200}) == HTTOPRIGHT);
  assert(RemoteResizeHitTest(HTBOTTOM, frame, {350, 800}) == HTBOTTOMLEFT);
  assert(RemoteResizeHitTest(HTBOTTOM, frame, {850, 800}) == HTBOTTOMRIGHT);
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
  // At a screen boundary, the old anchor moves while the touched boundary stays fixed.
  const RECT work_area{0, 0, 1920, 1040};
  RECT growing{100, 100, 1900, 1400};
  ConstrainRemoteWindow(growing, WMSZ_BOTTOMRIGHT, 16.0 / 9.0,
                        16, 40, 300, 200, RemoteResizeWorkArea({100, 100, 1100, 700}, &work_area));
  assert(growing.bottom == work_area.bottom && growing.top < 100);
  assert(growing.left >= work_area.left && growing.right <= work_area.right);
  assert(std::abs((growing.bottom - growing.top - 40) -
                  (growing.right - growing.left - 16) * 9.0 / 16.0) <= 1);
  // A monitor with a negative origin uses its own bounds, even for an oversized drag.
  const RECT second_monitor{-1920, -100, 0, 940};
  RECT oversized{-2500, -800, 1500, 2200};
  ConstrainRemoteWindow(oversized, WMSZ_TOPLEFT, 16.0 / 9.0,
                        16, 40, 300, 200, &second_monitor);
  assert(oversized.left >= second_monitor.left && oversized.top >= second_monitor.top);
  assert(oversized.right <= second_monitor.right && oversized.bottom <= second_monitor.bottom);
  // Spanning and partially off-screen windows retain their anchor and size freedom.
  for (const RECT initial : {RECT{-400, 100, 2300, 1650},
                             RECT{100, -100, 1100, 500}}) {
    RECT resized = initial;
    resized.right += 100;
    ConstrainRemoteWindow(resized, WMSZ_BOTTOMRIGHT, 16.0 / 9.0,
                          16, 40, 300, 200, RemoteResizeWorkArea(initial, &work_area));
    assert(resized.left == initial.left && resized.top == initial.top);
    if (initial.left < 0)
      assert(resized.right - resized.left > work_area.right - work_area.left);
    assert(std::abs((resized.bottom - resized.top - 40) -
                    (resized.right - resized.left - 16) * 9.0 / 16.0) <= 1);
  }

}
