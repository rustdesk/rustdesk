import { chromium, webkit, devices } from "playwright";
import { mkdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const baseUrl = process.env.WEB_CLIENT_URL ?? "http://127.0.0.1:21119";
const password = process.env.RD_E2E_PASSWORD ?? "webclient-e2e";
const outDir = resolve(process.env.SHOT_DIR ?? "/workspace/web-client/docs/screenshots");
const statePath = "/tmp/rd-e2e-state.json";

function readState() {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      return JSON.parse(readFileSync(statePath, "utf8"));
    } catch (error) {
      if (attempt === 4) throw error;
    }
  }
  return {};
}

mkdirSync(outDir, { recursive: true });

function check(bucket, name, ok, detail = "") {
  bucket.push({ name, ok: Boolean(ok), detail: String(detail) });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` — ${detail}` : ""}`);
}

async function localPoint(page, remoteX, remoteY) {
  const view = await page.evaluate(() => window.__rdView);
  if (!view?.scale) throw new Error("view transform missing");
  return {
    x: view.panX + (remoteX + 0.5) * view.scale,
    y: view.panY + (remoteY + 0.5) * view.scale,
    view,
  };
}

async function touch(cdp, type, points) {
  await cdp.send("Input.dispatchTouchEvent", {
    type,
    touchPoints: points.map((point) => ({ id: point.id ?? 1, x: point.x, y: point.y })),
  });
}

async function tap(cdp, x, y, holdMs = 40) {
  await touch(cdp, "touchStart", [{ x, y, id: 1 }]);
  await new Promise((resolve) => setTimeout(resolve, holdMs));
  await touch(cdp, "touchEnd", []);
}

async function drag(cdp, x1, y1, x2, y2, steps = 8) {
  await touch(cdp, "touchStart", [{ x: x1, y: y1, id: 1 }]);
  for (let step = 1; step <= steps; step += 1) {
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 16));
    await touch(cdp, "touchMove", [{
      x: x1 + ((x2 - x1) * step) / steps,
      y: y1 + ((y2 - y1) * step) / steps,
      id: 1,
    }]);
  }
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 20));
  await touch(cdp, "touchEnd", []);
}

async function pinch(cdp, x, y, spread) {
  await touch(cdp, "touchStart", [
    { x: x - 30, y, id: 1 },
    { x: x + 30, y, id: 2 },
  ]);
  for (let step = 1; step <= 5; step += 1) {
    const extra = (spread * step) / 5;
    await touch(cdp, "touchMove", [
      { x: x - 30 - extra, y, id: 1 },
      { x: x + 30 + extra, y, id: 2 },
    ]);
  }
  await touch(cdp, "touchEnd", []);
}

async function twoFingerPan(cdp, x, y, dx, dy) {
  await touch(cdp, "touchStart", [
    { x: x - 40, y, id: 1 },
    { x: x + 40, y, id: 2 },
  ]);
  for (let step = 1; step <= 6; step += 1) {
    await touch(cdp, "touchMove", [
      { x: x - 40 + (dx * step) / 6, y: y + (dy * step) / 6, id: 1 },
      { x: x + 40 + (dx * step) / 6, y: y + (dy * step) / 6, id: 2 },
    ]);
  }
  await touch(cdp, "touchEnd", []);
}

async function twoFingerTap(cdp, x, y) {
  await touch(cdp, "touchStart", [
    { x: x - 28, y, id: 1 },
    { x: x + 28, y, id: 2 },
  ]);
  await new Promise((resolve) => setTimeout(resolve, 50));
  await touch(cdp, "touchEnd", []);
}

async function waitState(predicate, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let state = readState();
  while (Date.now() < deadline) {
    state = readState();
    if (predicate(state)) return state;
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
  return state;
}

async function connect(page) {
  await page.goto(baseUrl, { waitUntil: "domcontentloaded" });
  await page.getByTestId("host").fill("127.0.0.1");
  await page.getByTestId("port").fill("21118");
  await page.getByTestId("password").fill(password);
  const connectBox = await page.getByTestId("connect").boundingBox();
  const viewport = page.viewportSize();
  return { connectBox, viewport };
}

async function exercise(page, cdp, bucket, shots) {
  const errors = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(String(err)));

  const opened = await connect(page);
  const fontSize = await page.getByTestId("password").evaluate((node) => parseFloat(getComputedStyle(node).fontSize));
  check(bucket, "connect input font >= 16px", fontSize >= 16, `${fontSize}px`);
  const scroll = await page.evaluate(() => ({
    y: window.scrollY,
    overflow: getComputedStyle(document.body).overflow,
    overscroll: getComputedStyle(document.documentElement).overscrollBehavior,
    touch: getComputedStyle(document.documentElement).touchAction,
  }));
  check(bucket, "page does not scroll", scroll.y === 0 && scroll.overflow === "hidden", JSON.stringify(scroll));
  const connectInView = opened.connectBox && opened.viewport
    && opened.connectBox.y + opened.connectBox.height <= opened.viewport.height + 1
    && opened.connectBox.height >= 40;
  check(bucket, "connect button reachable", connectInView, JSON.stringify(opened.connectBox));
  if (shots.connect) await page.screenshot({ path: resolve(outDir, shots.connect) });

  const box = await page.getByTestId("connect").boundingBox();
  if (box) await tap(cdp, box.x + box.width / 2, box.y + box.height / 2);
  else await page.getByTestId("connect").click();

  await page.waitForFunction(
    () => (window.__rd?.state === "live" || window.__rd?.state === "error") && (window.__rd.decoded ?? 0) >= 1,
    null,
    { timeout: 30000 },
  );
  const live = await page.evaluate(() => window.__rd);
  check(bucket, "session goes live and decodes", live?.state === "live" && (live.decoded ?? 0) >= 1, JSON.stringify(live));
  if (live?.state !== "live") return errors;

  await page.getByTestId("connection-panel").waitFor({ state: "hidden" });
  await page.getByTestId("session-handle").waitFor({ state: "visible" });
  const stage = await page.getByTestId("stage").boundingBox();
  const viewport = page.viewportSize();
  check(
    bucket,
    "stage fills the screen",
    stage && viewport && Math.abs(stage.width - viewport.width) < 3 && Math.abs(stage.height - viewport.height) < 3,
    `${stage?.width}x${stage?.height}`,
  );
  const touchMode = await page.evaluate(() => window.__rd?.touchMode);
  check(bucket, "starts in touch mode", touchMode === true, String(touchMode));
  if (shots.connected) await page.screenshot({ path: resolve(outDir, shots.connected) });

  const beforeTap = readState();
  const button = await localPoint(page, 640, 180);
  await tap(cdp, button.x, button.y);
  const afterTap = await waitState((state) => state.clicks > beforeTap.clicks);
  check(bucket, "touch tap clicks the remote button", afterTap.clicks > beforeTap.clicks, JSON.stringify(afterTap));

  const entry = await localPoint(page, 640, 486);
  await tap(cdp, entry.x, entry.y);
  await page.waitForTimeout(200);
  await page.getByTestId("session-handle").tap();
  await page.getByTestId("keyboard").tap();
  await page.getByTestId("softbar").waitFor({ state: "visible" });
  const input = page.getByTestId("soft-input");
  await input.tap();
  const focused = await input.evaluate((node) => document.activeElement === node);
  check(bucket, "soft keyboard focuses an input", focused, "");
  await page.keyboard.insertText("ab");
  const typed = await waitState((state) => String(state.text).includes("ab"));
  check(bucket, "english text reaches the remote", String(typed.text).includes("ab"), typed.text);
  await page.keyboard.press("Backspace");
  const backed = await waitState((state) => String(state.text).endsWith("a") && !String(state.text).endsWith("ab"));
  check(bucket, "backspace deletes a remote character", String(backed.text).endsWith("a") && !String(backed.text).endsWith("ab"), backed.text);
  await input.evaluate((node) => {
    node.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    node.dispatchEvent(new CompositionEvent("compositionend", { data: "你好", bubbles: true }));
  });
  const chinese = await waitState((state) => String(state.text).includes("你好"), 4000);
  check(bucket, "composition text reaches the remote", String(chinese.text).includes("你好"), chinese.text);
  if (shots.keyboard) await page.screenshot({ path: resolve(outDir, shots.keyboard) });
  await page.getByRole("button", { name: "Hide" }).tap();
  await page.getByTestId("softbar").waitFor({ state: "hidden" });
  check(bucket, "closing the keyboard returns to the session", await page.getByTestId("session-handle").isVisible(), "");

  const beforeRight = readState();
  await tap(cdp, button.x, button.y, 700);
  const afterLong = await waitState((state) => state.right > beforeRight.right);
  check(
    bucket,
    "long press is a right click",
    afterLong.right > beforeRight.right && afterLong.clicks === beforeRight.clicks,
    JSON.stringify(afterLong),
  );

  const beforeTwo = readState();
  await twoFingerTap(cdp, button.x, button.y);
  const afterTwo = await waitState((state) => state.right > beforeTwo.right);
  check(bucket, "two-finger tap is a right click", afterTwo.right > beforeTwo.right, JSON.stringify(afterTwo));

  const beforeDrag = readState();
  const dragFrom = await localPoint(page, 360, 660);
  const dragTo = await localPoint(page, 860, 660);
  await drag(cdp, dragFrom.x, dragFrom.y, dragTo.x, dragTo.y);
  const afterDrag = await waitState((state) => Math.abs((state.drag ?? 0) - (beforeDrag.drag ?? 0)) >= 80);
  check(
    bucket,
    "one-finger drag reaches the remote",
    Math.abs((afterDrag.drag ?? 0) - (beforeDrag.drag ?? 0)) >= 80,
    `${beforeDrag.drag} -> ${afterDrag.drag}`,
  );

  const scaleBefore = (await page.evaluate(() => window.__rdView.scale));
  await pinch(cdp, viewport.width / 2, viewport.height / 2, 70);
  await page.waitForTimeout(200);
  const scaleAfter = await page.evaluate(() => window.__rdView.scale);
  check(bucket, "pinch zooms around the gesture", scaleAfter > scaleBefore * 1.08, `${scaleBefore} -> ${scaleAfter}`);
  if (shots.zoomed) await page.screenshot({ path: resolve(outDir, shots.zoomed) });

  const panBefore = await page.evaluate(() => ({ ...window.__rdView }));
  await twoFingerPan(cdp, viewport.width / 2, viewport.height / 2, 80, 40);
  await page.waitForTimeout(150);
  const panAfter = await page.evaluate(() => window.__rdView);
  const panShift = Math.hypot(panAfter.panX - panBefore.panX, panAfter.panY - panBefore.panY);
  check(bucket, "two-finger drag pans when zoomed", panShift > 20, panShift.toFixed(1));

  const beforeZoomTap = readState();
  const zoomedButton = await localPoint(page, 640, 180);
  await tap(cdp, zoomedButton.x, zoomedButton.y);
  const afterZoomTap = await waitState((state) => state.clicks > beforeZoomTap.clicks);
  check(bucket, "tap after zoom and pan still hits", afterZoomTap.clicks > beforeZoomTap.clicks, JSON.stringify(afterZoomTap));

  const handle = await page.getByTestId("session-handle").boundingBox();
  const handleLeft = await page.getByTestId("session-handle").evaluate((node) => node.style.left);
  if (handle) {
    await drag(cdp, handle.x + handle.width / 2, handle.y + 12, handle.x + handle.width / 2 + 90, handle.y + 12, 6);
    const movedLeft = await page.getByTestId("session-handle").evaluate((node) => node.style.left);
    check(bucket, "session handle drags by touch", movedLeft !== handleLeft, `${handleLeft} -> ${movedLeft}`);
  } else {
    check(bucket, "session handle drags by touch", false, "handle missing");
  }

  await page.getByTestId("session-handle").tap();
  await page.getByTestId("session-menu").waitFor({ state: "visible" });
  const menu = await page.getByTestId("session-menu").boundingBox();
  const menuInView = menu && viewport && menu.x >= -1 && menu.y >= -1
    && menu.x + menu.width <= viewport.width + 2
    && menu.y + menu.height <= viewport.height + 2;
  const shortTarget = await page.getByTestId("session-menu").evaluate((node) => {
    const buttons = [...node.querySelectorAll("button, select")];
    return buttons
      .map((item) => {
        const rect = item.getBoundingClientRect();
        return { text: item.textContent?.trim() || item.tagName, h: rect.height, w: rect.width };
      })
      .filter((item) => item.h < 40 || item.w < 40);
  });
  check(bucket, "menu fits the screen", Boolean(menuInView), JSON.stringify(menu));
  check(bucket, "menu targets are at least 40px", shortTarget.length === 0, JSON.stringify(shortTarget));
  if (shots.menu) await page.screenshot({ path: resolve(outDir, shots.menu) });
  const beforeLeak = readState().text;
  await page.getByRole("button", { name: "Connection" }).tap();
  await page.getByTestId("password").tap();
  await page.keyboard.type("zzz", { delay: 20 });
  await page.waitForTimeout(300);
  check(bucket, "menu typing does not leak", readState().text === beforeLeak, readState().text);
  await page.keyboard.press("Escape");
  const backdrop = await page.getByTestId("session-backdrop").boundingBox().catch(() => null);
  if (backdrop) await tap(cdp, 12, Math.min(viewport.height - 20, backdrop.y + backdrop.height - 16));
  await page.getByTestId("session-menu").waitFor({ state: "hidden", timeout: 3000 }).catch(() => {});
  const menuGone = await page.getByTestId("session-menu").isHidden();
  check(bucket, "backdrop tap closes the menu", menuGone, "");

  const landscape = { width: viewport.height, height: viewport.width };
  await page.setViewportSize(landscape);
  await page.waitForTimeout(400);
  const landStage = await page.getByTestId("stage").boundingBox();
  check(
    bucket,
    "orientation keeps the session and refits",
    landStage && Math.abs(landStage.width - landscape.width) < 3 && Math.abs(landStage.height - landscape.height) < 3
      && (await page.evaluate(() => window.__rd?.state)) === "live",
    `${landStage?.width}x${landStage?.height}`,
  );
  const fit = await page.evaluate(() => {
    const view = window.__rdView;
    const remoteW = window.__rd?.width || 1280;
    const remoteH = window.__rd?.height || 720;
    const expected = Math.min(view.width / remoteW, view.height / remoteH);
    return { scale: view.scale, expected };
  });
  check(bucket, "landscape mapping matches fit", Math.abs(fit.scale - fit.expected) < 0.02, JSON.stringify(fit));
  const beforeLand = readState();
  const landButton = await localPoint(page, 640, 180);
  await tap(cdp, landButton.x, landButton.y);
  const afterLand = await waitState((state) => state.clicks > beforeLand.clicks);
  check(bucket, "landscape tap still hits the button", afterLand.clicks > beforeLand.clicks, JSON.stringify(afterLand));
  if (shots.landscape) await page.screenshot({ path: resolve(outDir, shots.landscape) });

  await page.getByTestId("session-handle").tap();
  const landMenu = await page.getByTestId("session-menu").boundingBox();
  const landMenuOk = landMenu && landMenu.x >= -1 && landMenu.y >= -1
    && landMenu.x + landMenu.width <= landscape.width + 2
    && landMenu.y + landMenu.height <= landscape.height + 2;
  check(bucket, "landscape menu fits", Boolean(landMenuOk), JSON.stringify(landMenu));
  await page.getByTestId("touch-mode").tap();
  await page.getByTestId("session-close").tap();
  await page.getByTestId("session-menu").waitFor({ state: "hidden" });
  await page.getByTestId("remote-cursor").waitFor({ state: "visible" });
  const beforeMouseTap = readState();
  const cursorAtTap = await page.evaluate(() => ({ x: window.__rd?.cursorX, y: window.__rd?.cursorY }));
  await tap(cdp, landscape.width / 2, landscape.height / 2);
  const afterMouseTap = await waitState((state) => state.clicks > beforeMouseTap.clicks, 2500);
  check(
    bucket,
    "mouse mode tap clicks at the cursor",
    afterMouseTap.clicks > beforeMouseTap.clicks && (cursorAtTap.y ?? 999) < 400,
    JSON.stringify({ cursorAtTap, afterMouseTap }),
  );
  const cursorBefore = await page.evaluate(() => ({ x: window.__rd?.cursorX ?? 0, y: window.__rd?.cursorY ?? 0 }));
  const fromCursor = await localPoint(page, cursorBefore.x, cursorBefore.y);
  const toDrag = await localPoint(page, cursorBefore.x, 660);
  await drag(cdp, fromCursor.x, fromCursor.y, toDrag.x, toDrag.y, 6);
  const cursorAfter = await page.evaluate(() => ({ x: window.__rd?.cursorX, y: window.__rd?.cursorY }));
  const cursorMoved = Math.hypot((cursorAfter.x ?? 0) - cursorBefore.x, (cursorAfter.y ?? 0) - cursorBefore.y);
  check(bucket, "mouse mode moves a visible cursor relatively", cursorMoved > 20, JSON.stringify({ cursorBefore, cursorAfter }));
  const beforeLock = readState();
  const lockAt = await localPoint(page, cursorAfter.x ?? cursorBefore.x, cursorAfter.y ?? 660);
  await tap(cdp, lockAt.x, lockAt.y, 40);
  await new Promise((resolve) => setTimeout(resolve, 80));
  await drag(cdp, lockAt.x, lockAt.y, lockAt.x + 100, lockAt.y, 6);
  const afterLock = await waitState((state) => Math.abs((state.drag ?? 0) - (beforeLock.drag ?? 0)) >= 20, 3000);
  check(bucket, "mouse mode double-tap drag reaches the host", Math.abs((afterLock.drag ?? 0) - (beforeLock.drag ?? 0)) >= 8, JSON.stringify(afterLock));

  const decodedLater = await page.evaluate(() => window.__rd?.decoded ?? 0);
  check(bucket, "frames keep decoding", decodedLater >= (live.decoded ?? 1), String(decodedLater));
  check(bucket, "no console errors", errors.length === 0, errors.join(" | "));
  return errors;
}

const report = { devices: [], notes: [] };

const pixel = devices["Pixel 7"];
const iphone = devices["iPhone 14"];

const browser = await chromium.launch({
  headless: true,
  args: ["--use-gl=angle", "--use-angle=swiftshader"],
});

const pixelPage = await browser.newPage({ ...pixel });
const pixelCdp = await pixelPage.context().newCDPSession(pixelPage);
const pixelChecks = [];
try {
  await exercise(pixelPage, pixelCdp, pixelChecks, {
    connect: "mobile-connect.png",
    connected: "mobile-connected.png",
    zoomed: "mobile-zoomed.png",
    menu: "mobile-menu.png",
    keyboard: "mobile-keyboard.png",
    landscape: "mobile-landscape.png",
  });
} catch (error) {
  check(pixelChecks, "pixel run completed", false, String(error));
}
report.devices.push({ name: "Pixel 7 Chromium", userAgent: pixel.userAgent, checks: pixelChecks });
await pixelPage.close();

let iphoneEngine = "chromium-iphone-profile";
try {
  const probe = await webkit.launch({ headless: true });
  const page = await probe.newPage({ ...iphone });
  await page.goto(baseUrl, { waitUntil: "domcontentloaded" });
  const codecs = await page.evaluate(async () => {
    if (!("VideoDecoder" in window)) return { ok: false, reason: "VideoDecoder missing" };
    const support = {};
    for (const codec of ["vp8", "vp09.00.10.08", "avc1.640033", "av01.0.08M.08"]) {
      try {
        support[codec] = Boolean((await VideoDecoder.isConfigSupported({ codec, optimizeForLatency: true })).supported);
      } catch (err) {
        support[codec] = String(err);
      }
    }
    return { ok: Object.values(support).some((value) => value === true), support };
  });
  await probe.close();
  report.notes.push(
    codecs.ok
      ? `WebKit can decode ${JSON.stringify(codecs.support)}, but Playwright WebKit has no CDP multi-touch. The iPhone gesture pass uses Chromium with the iPhone 14 profile (touch, mobile UA, device scale).`
      : `WebKit has no usable VideoDecoder for the negotiated codecs (${JSON.stringify(codecs)}). The iPhone pass uses Chromium with the iPhone 14 profile.`,
  );
} catch (error) {
  report.notes.push(`WebKit did not launch (${error}). The iPhone pass uses Chromium with the iPhone 14 profile.`);
}

const iphoneChecks = [];
const iphonePage = await browser.newPage({ ...iphone });
const iphoneCdp = await iphonePage.context().newCDPSession(iphonePage);
try {
  await exercise(iphonePage, iphoneCdp, iphoneChecks, {
    connect: "iphone-connect.png",
    connected: "iphone-connected.png",
    landscape: "iphone-landscape.png",
  });
} catch (error) {
  check(iphoneChecks, "iphone run completed", false, String(error));
}
report.devices.push({ name: `iPhone 14 ${iphoneEngine}`, userAgent: iphone.userAgent, checks: iphoneChecks });
await iphonePage.close();
await browser.close();

const failed = report.devices.flatMap((device) => device.checks.filter((item) => !item.ok).map((item) => `${device.name}: ${item.name}`));
report.failed = failed;
console.log(JSON.stringify(report, null, 2));
if (failed.length) process.exitCode = 1;
