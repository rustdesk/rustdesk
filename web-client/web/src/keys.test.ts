import { describe, expect, it } from "vitest";
import { isBrowserReserved, keyDownAction, keyUpAction } from "./keys";

const bare = { ctrl: false, alt: false, meta: false, shift: false, repeat: false };

describe("physical keyboard", () => {
  it("holds Space for a possible pan and sends a plain tap on release", () => {
    expect(keyDownAction({ ...bare, code: "Space", key: " " })).toEqual({ type: "space-down" });
    expect(keyUpAction({ ...bare, code: "Space", key: " " })).toEqual({ type: "space-up" });
  });

  it("sends Ctrl+A with the Control modifier", () => {
    expect(keyDownAction({ ...bare, code: "KeyA", key: "a", ctrl: true })).toEqual({
      type: "combo",
      chr: "a",
      modifiers: ["Control"],
    });
    expect(keyUpAction({ ...bare, code: "KeyA", key: "a", ctrl: true })).toEqual({
      type: "combo",
      chr: "a",
      modifiers: ["Control"],
    });
  });

  it("leaves browser tab chords to the browser", () => {
    for (const code of ["KeyW", "KeyT", "KeyN", "Tab"]) {
      expect(isBrowserReserved({ code, ctrl: true, meta: false })).toBe(true);
      expect(keyDownAction({ ...bare, code, key: code, ctrl: true })).toEqual({ type: "reserved" });
      expect(keyUpAction({ ...bare, code, key: code, ctrl: true })).toEqual({ type: "ignore" });
    }
  });
});
