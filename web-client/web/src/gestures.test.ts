import { describe, expect, it } from "vitest";
import { DOUBLE_TAP_MS, fingerDown, fingerUp, isTwoFingerTap, keyboardInset, TAP_SLOP_PX } from "./gestures";

describe("phone gestures", () => {
  it("treats a still finger as a left click and a long-press as already handled", () => {
    expect(fingerUp({ moved: 2, longFired: false, dragging: false })).toBe("left-click");
    expect(fingerUp({ moved: 0, longFired: true, dragging: false })).toBe("none");
    expect(fingerUp({ moved: TAP_SLOP_PX + 1, longFired: false, dragging: true })).toBe("end-drag");
  });

  it("locks a drag only for a quick second tap in mouse mode", () => {
    expect(fingerDown("mouse", DOUBLE_TAP_MS - 1)).toBe("drag-lock");
    expect(fingerDown("mouse", DOUBLE_TAP_MS)).toBe("pending");
    expect(fingerDown("touch", 40)).toBe("pending");
    expect(fingerDown("mouse", null)).toBe("pending");
  });

  it("counts a two-finger tap only when the fingers stay put", () => {
    expect(isTwoFingerTap(2, 3)).toBe(true);
    expect(isTwoFingerTap(20, 0)).toBe(false);
    expect(isTwoFingerTap(0, 30)).toBe(false);
  });

  it("measures the soft-keyboard overlap and ignores a closed keyboard", () => {
    expect(keyboardInset(844, 500, 0)).toBe(344);
    expect(keyboardInset(844, 844, 0)).toBe(0);
    expect(keyboardInset(844, 820, 24)).toBe(0);
  });
});