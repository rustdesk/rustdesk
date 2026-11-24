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

const beforeShot = resolve(outDir, "desktop-connected.png");
await page.screenshot({ path: beforeShot });

const canvas = page.getByTestId("canvas");
const box = await canvas.boundingBox();
if (!box) throw new Error("canvas has no box");
const remote = await page.evaluate(() => ({ width: window.__rd.width, height: window.__rd.height }));

function localPoint(remoteX, remoteY) {
  const boxAspect = box.width / box.height;
  const remoteAspect = remote.width / remote.height;
  let rect;
  if (boxAspect > remoteAspect) {
    const height = box.height;
    const width = height * remoteAspect;
    rect = { x: (box.width - width) / 2, y: 0, width, height };
  } else {
    const width = box.width;
    const height = width / remoteAspect;
    rect = { x: 0, y: (box.height - height) / 2, width, height };
  }
  return {
    x: box.x + rect.x + ((remoteX + 0.5) / remote.width) * rect.width,
    y: box.y + rect.y + ((remoteY + 0.5) / remote.height) * rect.height,
  };
}

const typed = "hello-e2e";
const entry = localPoint(640, 486);
await page.mouse.click(entry.x, entry.y);
await page.waitForTimeout(300);
await page.keyboard.type(typed, { delay: 40 });

const button = localPoint(640, 180);
await page.mouse.click(button.x, button.y);
await page.waitForTimeout(200);
await page.mouse.click(button.x, button.y);

await page.waitForFunction(() => (window.__rd?.decoded ?? 0) >= 8, null, { timeout: 20000 });
const deadline = Date.now() + 15000;
let state = readState();
while (Date.now() < deadline) {
  state = readState();
  if (state.clicks >= 1 && String(state.text).includes(typed)) break;
  await page.waitForTimeout(250);
  state = readState();
}
if (!(state.clicks >= 1 && String(state.text).includes(typed))) {
  throw new Error(`remote state was not updated: ${JSON.stringify(state)}`);
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

await page.setViewportSize({ width: 390, height: 844 });
await page.waitForTimeout(400);
const quickbar = await page.getByTestId("quickbar").isVisible();
const keyboard = await page.getByTestId("keyboard").isVisible();
if (!quickbar || !keyboard) {
  throw new Error(`mobile controls missing quickbar=${quickbar} keyboard=${keyboard}`);
}
const mobileShot = resolve(outDir, "mobile-connected.png");
await page.screenshot({ path: mobileShot });

const finalStats = await page.evaluate(() => window.__rd);
const report = {
  stats: finalStats,
  remote: state,
  nonBlank,
  screenshots: [beforeShot, afterShot, mobileShot],
  consoleErrors,
};
console.log(JSON.stringify(report, null, 2));
await browser.close();
