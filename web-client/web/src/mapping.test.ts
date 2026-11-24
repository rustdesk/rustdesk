import { describe, expect, it } from "vitest";
import { clampPan, fitScale, mapFromRemote, mapToRemote, zoomAround } from "./mapping";

describe("view transform", () => {
  it("fits a 1280x720 picture inside a taller box", () => {
    const scale = fitScale(900, 900, 1280, 720);
    expect(scale).toBeCloseTo(900 / 1280);
    const pan = clampPan(900, 900, 1280, 720, scale, 0, 0);
    const mapped = mapToRemote(pan.panX + (640.5 * scale), pan.panY + (360.5 * scale), 1280, 720, {
      scale,
      ...pan,
    });
    expect(mapped).toEqual({ x: 640, y: 360 });
    expect(mapToRemote(2, 2, 1280, 720, { scale, ...pan })).toBeNull();
  });

  it("maps 1:1 pixels and clamps pan so the picture stays reachable", () => {
    const scale = 1;
    const centered = clampPan(800, 600, 1280, 720, scale, -100, -40);
    expect(centered.panX).toBe(-100);
    expect(centered.panY).toBe(-40);
    const over = clampPan(800, 600, 1280, 720, scale, 40, -900);
    expect(over.panX).toBe(0);
    expect(over.panY).toBe(600 - 720);
    const mapped = mapToRemote(10, 20, 1280, 720, { scale, panX: -100, panY: -40 });
    expect(mapped).toEqual({ x: 110, y: 60 });
  });

  it("keeps the cursor's remote point fixed when zooming", () => {
    const oldScale = 1;
    const pan = { panX: -100, panY: -40 };
    const anchor = { x: 200, y: 150 };
    const next = zoomAround(anchor.x, anchor.y, oldScale, 2, pan.panX, pan.panY);
    const before = mapToRemote(anchor.x, anchor.y, 1280, 720, { scale: oldScale, ...pan });
    const after = mapToRemote(anchor.x, anchor.y, 1280, 720, { scale: 2, ...next });
    expect(before).toEqual(after);
  });

  it("round-trips a panned zoomed point", () => {
    const transform = { scale: 2.5, panX: -400, panY: -80 };
    const local = mapFromRemote(640, 180, transform);
    const remote = mapToRemote(local.x, local.y, 1280, 720, transform);
    expect(remote).toEqual({ x: 640, y: 180 });
  });
});
