import { expect, test } from "@playwright/test";
import { pathToFileURL } from "node:url";

test("发送页初始化 WASM 并渲染二进制 QRBeam 帧", async ({ page }) => {
  await page.goto("send/");
  await page.locator("#file").setInputFiles({
    name: "sample.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.from([1, 2, 3, 4]),
  });
  await expect(page.locator("#status")).toContainText("发送文件信息", { timeout: 8_000 });
  const canvasWidth = await page.locator("#qr").evaluate(canvas => (canvas as HTMLCanvasElement).width);
  expect(canvasWidth).toBeGreaterThan(0);
  expect(canvasWidth).not.toBe(1080);
});

test("可扫窄窗口自动选择低档位并显示模块物理像素", async ({ page }) => {
  await page.setViewportSize({ width: 540, height: 720 });
  await page.goto("send/");
  await page.locator("#file").setInputFiles({
    name: "scan-safe.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.alloc(1024, 7),
  });
  await expect(page.locator("#qr-player")).toHaveClass(/active/, { timeout: 8_000 });
  await expect(page.locator("#profile-status")).toContainText("模块", { timeout: 8_000 });
  await expect(page.locator("#profile-status")).toContainText("符号/帧");
});

test("高 DPR 屏幕保持二维码完整同屏", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, "devicePixelRatio", { configurable: true, value: 2 });
  });
  await page.setViewportSize({ width: 1800, height: 1000 });
  await page.goto("send/");
  await page.locator("#file").setInputFiles({
    name: "retina.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.alloc(1024, 7),
  });
  await expect(page.locator("#qr-player")).toHaveClass(/active/, { timeout: 8_000 });
  const dimensions = await page.locator("#qr").evaluate(canvas => ({
    bitmapWidth: (canvas as HTMLCanvasElement).width,
    cssWidth: canvas.getBoundingClientRect().width,
    dpr: window.devicePixelRatio,
    viewportHeight: window.innerHeight,
  }));
  expect(dimensions.cssWidth).toBe(dimensions.bitmapWidth / dimensions.dpr);
  expect(dimensions.cssWidth).toBeLessThanOrEqual(dimensions.viewportHeight - 146);
});

test("过窄窗口拒绝开始并提示全屏", async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 720 });
  await page.goto("send/");
  await page.locator("#file").setInputFiles({
    name: "too-small.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.alloc(1024, 7),
  });
  await expect(page.locator("#profile-status")).toContainText("至少 6 px", { timeout: 8_000 });
  await expect(page.locator("#qr-player")).not.toHaveClass(/active/);
});

test("停止发送时清除上一会话的二维码画布", async ({ page }) => {
  await page.goto("send/");
  await page.locator("#file").setInputFiles({
    name: "clear-frame.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.alloc(1024, 7),
  });
  await expect(page.locator("#qr-player")).toHaveClass(/active/, { timeout: 8_000 });
  await page.locator("#stop").click();
  const canvasWidth = await page.locator("#qr").evaluate(canvas => (canvas as HTMLCanvasElement).width);
  expect(canvasWidth).toBe(0);
  await expect(page.locator("#filename")).toHaveText("等待文件");
});

test("独立发送页不依赖开发服务器", async ({ page }) => {
  await page.goto(pathToFileURL(`${process.cwd()}/qrbeam-send.html`).href);
  await page.locator("#file").setInputFiles({
    name: "offline.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.from([5, 6, 7, 8]),
  });
  await expect(page.locator("#status")).toContainText("发送文件信息", { timeout: 8_000 });
});

test("接收页加载 WASM 与本地恢复存储", async ({ page }) => {
  await page.goto("receive/");
  await expect(page.locator("h1")).toHaveText("接收文件");
  await expect(page.locator("#status")).toHaveText("等待文件信息二维码");
  await expect(page.locator("#save")).toBeDisabled();
});
