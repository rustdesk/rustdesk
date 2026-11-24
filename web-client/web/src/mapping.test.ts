import { describe, expect, it } from "vitest";
import { mapFromRemote, mapToRemote, videoRect } from "./mapping";

describe("coordinate mapping", () => {
  it("fills the box when the aspect matches", () => {
    const rect = videoRect(1280, 720, 1280, 720);
    expect(rect).toEqual({ x: 0, y: 0, width: 1280, height: 720 });
    expect(mapToRemote(0, 0, 1280, 720, 1280, 720)).toEqual({ x: 0, y: 0 });
    expect(mapToRemote(640, 180, 1280, 720, 1280, 720)).toEqual({ x: 640, y: 180 });
  });

  it("ignores horizontal letterbox bars", () => {
    const rect = videoRect(900, 900, 1280, 720);
    expect(rect.y).toBeGreaterThan(0);
    expect(rect.width).toBe(900);
    const mapped = mapToRemote(rect.x + rect.width / 2, rect.y + rect.height / 2, 900, 900, 1280, 720);
    expect(mapped).toEqual({ x: 640, y: 360 });
    expect(mapToRemote(10, 10, 900, 900, 1280, 720)).toBeNull();
  });

  it("round-trips a remote point through the letterbox", () => {
    const local = mapFromRemote(640, 180, 390, 500, 1280, 720);
    expect(local).not.toBeNull();
    const remote = mapToRemote(local!.x, local!.y, 390, 500, 1280, 720);
    expect(remote).not.toBeNull();
    expect(Math.abs(remote!.x - 640)).toBeLessThanOrEqual(1);
    expect(Math.abs(remote!.y - 180)).toBeLessThanOrEqual(1);
  });
});
