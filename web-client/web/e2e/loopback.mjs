import { chromium } from "playwright";
import { readFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

const baseUrl = process.env.WEB_CLIENT_URL ?? "http://127.0.0.1:21119";
const password = process.env.RD_E2E_PASSWORD ?? "webclient-e2e";
const outDir = resolve(process.env.SHOT_DIR ?? "/workspace/web-client/docs/screenshots");
const statePath = "/tmp/rd-e2e-state.json";

function readState() {
  return JSON.parse(readFileSync(statePath, "utf8"));
}

mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({
  headless: true,
  args: ["--use-gl=angle", "--use-angle=swiftshader"],
});

const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const consoleErrors = [];
page.on("console", (msg) => {
  if (msg.type() === "error") consoleErrors.push(msg.text());
});
page.on("pageerror", (err) => consoleErrors.push(String(err)));

await page.goto(baseUrl, { waitUntil: "domcontentloaded" });
await page.getByTestId("host").fill("127.0.0.1");
await page.getByTestId("port").fill("21118");
await page.getByTestId("password").fill(password);
await page.getByTestId("connect").click();

await page.waitForFunction(
  () => (window.__rd?.state === "live" || window.__rd?.state === "error") && window.__rd.decoded >= 1,
  null,
  { timeout: 45000 },
);
const afterConnect = await page.evaluate(() => window.__rd);
if (afterConnect?.state !== "live") {
  throw new Error(`session did not go live: ${JSON.stringify(afterConnect)} ${consoleErrors.join(" | ")}`);
}
await page.waitForFunction(() => (window.__rd?.decoded ?? 0) >= 3, null, { timeout: 20000 });
await page.getByTestId("connection-panel").waitFor({ state: "hidden" });
await page.getByTestId("session-handle").waitFor({ state: "visible" });

const canvas = page.getByTestId("canvas");
const stageBox = await page.getByTestId("stage").boundingBox();
const viewport = page.viewportSize();
if (!stageBox || !viewport) throw new Error("stage or viewport missing");
if (Math.abs(stageBox.width - viewport.width) > 2 || Math.abs(stageBox.height - viewport.height) > 2) {
  throw new Error(`stage does not fill the viewport: ${stageBox.width}x${stageBox.height} vs ${viewport.width}x${viewport.height}`);
}

const beforeShot = resolve(outDir, "desktop-connected.png");
await page.screenshot({ path: beforeShot });

const remote = await page.evaluate(() => ({ width: window.__rd.width, height: window.__rd.height }));

async function localPoint(remoteX, remoteY) {
  const view = await page.evaluate(() => window.__rdView);
  if (!view || !view.scale) throw new Error("view transform missing");
  return {
    x: view.panX + (remoteX + 0.5) * view.scale,
    y: view.panY + (remoteY + 0.5) * view.scale,
  };
}

const fitBefore = await page.evaluate(() => window.__rdView.scale);
await page.getByTestId("session-handle").click();
await page.getByTestId("zoom-in").click();
await page.getByTestId("session-close").click();
await page.getByTestId("session-menu").waitFor({ state: "hidden" });
const fitAfter = await page.evaluate(() => window.__rdView.scale);
if (!(fitAfter > fitBefore * 1.05)) {
  throw new Error(`zoom did not increase scale (${fitBefore} -> ${fitAfter})`);
}

const panBefore = await page.evaluate(() => ({ ...window.__rdView }));
await page.getByTestId("session-handle").click();
await page.getByTestId("pan").click();
await page.getByTestId("session-close").click();
await page.getByTestId("session-menu").waitFor({ state: "hidden" });
await page.mouse.move(720, 420);
await page.mouse.down();
await page.mouse.move(560, 300, { steps: 8 });
await page.mouse.up();
const panAfter = await page.evaluate(() => window.__rdView);
const panShift = Math.hypot(panAfter.panX - panBefore.panX, panAfter.panY - panBefore.panY);
if (panShift < 40) {
  throw new Error(`viewport did not pan (${panShift.toFixed(1)} px) ${JSON.stringify(panAfter)}`);
}
await page.getByTestId("session-handle").click();
await page.getByTestId("pan").click();
await page.getByTestId("session-close").click();
await page.getByTestId("session-menu").waitFor({ state: "hidden" });
await page.waitForTimeout(200);
const zoomedShot = resolve(outDir, "desktop-zoomed.png");
await page.screenshot({ path: zoomedShot });

const typed = "hello-e2e";
let entry = await localPoint(640, 486);
for (let attempt = 0; attempt < 3 && !String(readState().text).includes(typed); attempt += 1) {
  entry = await localPoint(640, 486);
  await page.mouse.click(entry.x, entry.y);
  await page.waitForTimeout(200);
  await page.keyboard.type(typed, { delay: 30 });
  const typeDeadline = Date.now() + 2500;
  while (Date.now() < typeDeadline && !String(readState().text).includes(typed)) {
    await page.waitForTimeout(100);
  }
}

const clicksBefore = readState().clicks;
const button = await localPoint(640, 180);
await page.mouse.click(button.x, button.y);
await page.waitForTimeout(250);
if (readState().clicks <= clicksBefore) {
  const retry = await localPoint(640, 180);
  await page.mouse.click(retry.x, retry.y);
}

await page.waitForFunction(() => (window.__rd?.decoded ?? 0) >= 8, null, { timeout: 20000 });
const dragStart = await localPoint(420, 660);
const dragEnd = await localPoint(860, 660);
await page.mouse.move(dragStart.x, dragStart.y);
await page.mouse.down();
await page.mouse.move(dragEnd.x, dragEnd.y, { steps: 12 });
await page.mouse.up();

const deadline = Date.now() + 15000;
let state = readState();
while (Date.now() < deadline) {
  state = readState();
  if (state.clicks >= 1 && String(state.text).includes(typed) && Math.abs(state.drag ?? 0) >= 80) break;
  await page.waitForTimeout(250);
  state = readState();
}
if (!(state.clicks >= 1 && String(state.text).includes(typed) && Math.abs(state.drag ?? 0) >= 80)) {
  await page.screenshot({ path: "/tmp/miss.png" });
  const view = await page.evaluate(() => window.__rdView);
  throw new Error(`remote state was not updated: ${JSON.stringify(state)} view=${JSON.stringify(view)} entry=${JSON.stringify(entry)} button=${JSON.stringify(button)} drag=${JSON.stringify(dragStart)}->${JSON.stringify(dragEnd)}`);
}

const nonBlank = await canvas.evaluate((node) => {
  const ctx = node.getContext("2d");
  if (!ctx) return 0;
  const sample = ctx.getImageData(0, 0, node.width, node.height).data;
  let changed = 0;
  const step = 16 * 4;
  for (let i = 0; i < sample.length; i += step) {
    const r = sample[i];
    const g = sample[i + 1];
    const b = sample[i + 2];
    if (r + g + b > 30 && !(r < 8 && g < 8 && b < 8)) changed += 1;
  }
  return changed;
});
if (nonBlank < 20) {
  throw new Error(`canvas looks blank (${nonBlank} samples)`);
}

const afterShot = resolve(outDir, "desktop-after-input.png");
await page.screenshot({ path: afterShot });

const handle = await page.getByTestId("session-handle").boundingBox();
const handleLeft = handle ? await page.getByTestId("session-handle").evaluate((node) => node.style.left) : "";
if (handle) {
  await page.mouse.move(handle.x + handle.width / 2, handle.y + 4);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2 + 120, handle.y + 4, { steps: 6 });
  await page.mouse.up();
  const movedLeft = await page.getByTestId("session-handle").evaluate((node) => node.style.left);
  if (movedLeft === handleLeft) throw new Error("top handle did not move");
}
await page.keyboard.press("Escape");
await page.getByTestId("session-menu").waitFor({ state: "hidden" }).catch(() => {});

const beforeMenu = readState().text;
await page.getByTestId("session-handle").click();
await page.getByTestId("session-menu").waitFor({ state: "visible" });
await page.getByTestId("quickbar").waitFor({ state: "visible" });
await page.getByTestId("codec").waitFor({ state: "visible" });
const menuShot = resolve(outDir, "desktop-menu-open.png");
await page.screenshot({ path: menuShot });
await page.getByRole("button", { name: "Connection" }).click();
await page.getByTestId("password").click();
await page.keyboard.type("zzz", { delay: 20 });
await page.waitForTimeout(400);
if (readState().text !== beforeMenu) {
  throw new Error(`dialog typing leaked to the host: ${readState().text}`);
}
await page.keyboard.press("Escape");
await page.getByTestId("connection-panel").waitFor({ state: "hidden" });
await page.getByTestId("session-handle").waitFor({ state: "visible" });

await page.getByTestId("session-handle").click();
await page.getByRole("button", { name: "Fit", exact: true }).click();
await page.getByTestId("session-close").click();
await page.setViewportSize({ width: 390, height: 844 });
await page.waitForTimeout(400);
const mobileBox = await page.getByTestId("stage").boundingBox();
if (!mobileBox || Math.abs(mobileBox.width - 390) > 2 || Math.abs(mobileBox.height - 844) > 2) {
  throw new Error(`mobile canvas is ${mobileBox?.width}x${mobileBox?.height}`);
}
await page.getByTestId("session-handle").click();
await page.getByTestId("touch-mode").click();
const touchLabel = await page.getByTestId("touch-mode").innerText();
if (touchLabel !== "Touch") throw new Error(`touch mode toggle read ${touchLabel}`);
await page.getByTestId("touch-mode").click();
const mouseLabel = await page.getByTestId("touch-mode").innerText();
if (mouseLabel !== "Mouse") throw new Error(`mouse mode toggle read ${mouseLabel}`);
await page.getByTestId("session-close").click();
await page.getByTestId("session-menu").waitFor({ state: "hidden" });
const scaleBeforePinch = await page.evaluate(() => window.__rdView.scale);
const cdp = await page.context().newCDPSession(page);
await cdp.send("Input.dispatchTouchEvent", {
  type: "touchStart",
  touchPoints: [
    { x: 120, y: 400, id: 1 },
    { x: 260, y: 400, id: 2 },
  ],
});
for (let step = 1; step <= 5; step += 1) {
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { x: 120 - step * 18, y: 400, id: 1 },
      { x: 260 + step * 18, y: 400, id: 2 },
    ],
  });
}
await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
await page.waitForTimeout(150);
const scaleAfterPinch = await page.evaluate(() => window.__rdView.scale);
if (!(scaleAfterPinch > scaleBeforePinch * 1.05)) {
  throw new Error(`pinch did not zoom (${scaleBeforePinch} -> ${scaleAfterPinch})`);
}
await page.getByTestId("session-handle").click();
await page.getByRole("button", { name: "Fit", exact: true }).click();
await page.getByTestId("keyboard").click();
await page.getByTestId("softbar").waitFor({ state: "visible" });
await page.getByTestId("softbar").getByRole("button", { name: "Hide" }).click();
await page.getByTestId("softbar").waitFor({ state: "hidden" });
const mobileShot = resolve(outDir, "mobile-connected.png");
await page.screenshot({ path: mobileShot });

const finalStats = await page.evaluate(() => window.__rd);
const report = {
  stats: finalStats,
  remote: state,
  nonBlank,
  screenshots: [beforeShot, zoomedShot, afterShot, menuShot, mobileShot],
  consoleErrors,
};
console.log(JSON.stringify(report, null, 2));
await browser.close();
