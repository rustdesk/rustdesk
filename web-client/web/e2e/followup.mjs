import { chromium } from "playwright";
import { readFileSync } from "node:fs";
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

function assertFull(name, rect, width, height) {
  if (!rect) throw new Error(`${name} missing`);
  const ok = Math.abs(rect.x) <= 1 && Math.abs(rect.y) <= 1
    && Math.abs(rect.width - width) <= 2 && Math.abs(rect.height - height) <= 2;
  if (!ok) throw new Error(`${name} is ${rect.x},${rect.y} ${rect.width}x${rect.height}, expected 0,0 ${width}x${height}`);
}

async function metrics(page) {
  return page.evaluate(() => {
    const stage = document.querySelector("[data-testid=stage]").getBoundingClientRect();
    const canvas = document.querySelector("[data-testid=canvas]");
    const style = getComputedStyle(canvas);
    const backdrop = document.querySelector("[data-testid=connection-backdrop], [data-testid=session-backdrop]");
    const box = backdrop ? backdrop.getBoundingClientRect() : null;
    return {
      stage: { x: stage.x, y: stage.y, width: stage.width, height: stage.height },
      canvasMargin: style.margin,
      canvasPadding: style.padding,
      canvasBorder: style.borderTopWidth,
      backdrop: box && { x: box.x, y: box.y, width: box.width, height: box.height },
      cursorHidden: document.querySelector("[data-testid=remote-cursor]").hasAttribute("hidden"),
    };
  });
}

async function localPoint(page, remoteX, remoteY) {
  const view = await page.evaluate(() => window.__rdView);
  return {
    x: view.panX + (remoteX + 0.5) * view.scale,
    y: view.panY + (remoteY + 0.5) * view.scale,
  };
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

const browser = await chromium.launch({
  headless: true,
  args: ["--use-gl=angle", "--use-angle=swiftshader"],
});
try {
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const cdp = await page.context().newCDPSession(page);
await page.goto(baseUrl, { waitUntil: "domcontentloaded" });

const before = await metrics(page);
assertFull("desktop connection stage", before.stage, 1280, 720);
assertFull("desktop connection backdrop", before.backdrop, 1280, 720);
if (before.canvasMargin !== "0px") throw new Error(`canvas margin ${before.canvasMargin}`);
if (before.canvasPadding !== "0px") throw new Error(`canvas padding ${before.canvasPadding}`);
if (before.canvasBorder !== "0px") throw new Error(`canvas border ${before.canvasBorder}`);

await page.getByTestId("host").fill("127.0.0.1");
await page.getByTestId("port").fill("21118");
await page.getByTestId("password").fill(password);
await page.getByTestId("connect").click();
await page.waitForFunction(() => window.__rd?.state === "live" && (window.__rd.decoded ?? 0) >= 1, null, { timeout: 30000 });
await page.getByTestId("connection-panel").waitFor({ state: "hidden" });
const live = await metrics(page);
assertFull("desktop live stage", live.stage, 1280, 720);
if (!live.cursorHidden) throw new Error("cursor dot is visible on a fine-pointer desktop");
await page.screenshot({ path: resolve(outDir, "desktop-connected.png") });

const clicksBefore = readState().clicks;
const button = await localPoint(page, 640, 180);
const viewAtClick = await page.evaluate(() => window.__rdView);
await page.mouse.click(button.x, button.y);
await page.mouse.click(button.x, button.y);
const clicked = await waitState((state) => state.clicks > clicksBefore);
if (!(clicked.clicks > clicksBefore)) throw new Error(`click missed at ${JSON.stringify(button)} view ${JSON.stringify(viewAtClick)}: ${JSON.stringify(clicked)}`);

const dragBefore = readState().drag ?? 0;
const from = await localPoint(page, 360, 660);
const to = await localPoint(page, 860, 660);
await page.mouse.move(from.x, from.y);
await page.mouse.down();
await page.mouse.move(to.x, to.y, { steps: 12 });
await page.mouse.up();
const dragged = await waitState((state) => Math.abs((state.drag ?? 0) - dragBefore) >= 80);
if (Math.abs((dragged.drag ?? 0) - dragBefore) < 80) throw new Error(`drag missed: ${dragBefore} -> ${dragged.drag}`);

const rightBefore = readState().right ?? 0;
const start = await localPoint(page, 200, 120);
await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ id: 1, x: start.x, y: start.y }] });
for (let step = 1; step <= 8; step += 1) {
  await new Promise((resolve) => setTimeout(resolve, 80));
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [{ id: 1, x: start.x + step * 24, y: start.y }],
  });
}
await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
await page.waitForTimeout(250);
const rightAfter = readState().right ?? 0;
if (rightAfter !== rightBefore) throw new Error(`long move fired a right click (${rightBefore} -> ${rightAfter})`);

const entry = await localPoint(page, 640, 486);
await page.mouse.click(entry.x, entry.y);
await page.waitForTimeout(200);
await page.keyboard.press("Space");
await page.keyboard.type("ab", { delay: 30 });
const spaced = await waitState((state) => String(state.text).includes(" "));
if (!String(spaced.text).includes(" ")) throw new Error(`space did not arrive: ${JSON.stringify(spaced)}`);
await page.keyboard.press("Control+A");
await page.waitForTimeout(100);
await page.keyboard.type("Z", { delay: 30 });
const selected = await waitState((state) => String(state.text).startsWith("Z") && String(state.text).includes("ab"));
if (!(String(selected.text).startsWith("Z") && String(selected.text).includes("ab"))) {
  throw new Error(`Ctrl+A did not move to the start of the field: ${JSON.stringify(selected)}`);
}

await page.getByTestId("session-handle").click();
await page.getByTestId("clipboard-text").fill("CLIP");
await page.getByTestId("clipboard-send").click();
const pasted = await waitState((state) => String(state.text).includes("CLIP"));
if (!String(pasted.text).includes("CLIP")) throw new Error(`clipboard paste missed: ${JSON.stringify(pasted)}`);

await page.getByTestId("session-handle").click();
await page.getByTestId("quality-low").waitFor({ state: "visible" });
await page.screenshot({ path: resolve(outDir, "desktop-menu-open.png") });
await page.keyboard.press("Escape");

await page.setViewportSize({ width: 390, height: 844 });
await page.waitForTimeout(300);
const mobile = await metrics(page);
assertFull("mobile live stage", mobile.stage, 390, 844);
if (mobile.canvasMargin !== "0px") throw new Error(`mobile canvas margin ${mobile.canvasMargin}`);
await page.getByTestId("session-handle").click();
await page.getByRole("button", { name: "Connection" }).click();
const mobileConnect = await metrics(page);
assertFull("mobile connection backdrop", mobileConnect.backdrop, 390, 844);
await page.keyboard.press("Escape");
await page.getByTestId("connection-panel").waitFor({ state: "hidden" });
await page.screenshot({ path: resolve(outDir, "mobile-connected.png") });

console.log(JSON.stringify({
  desktop: live.stage,
  mobile: mobile.stage,
  remote: readState(),
  cursorHidden: live.cursorHidden,
}, null, 2));
} finally {
  await browser.close();
}
