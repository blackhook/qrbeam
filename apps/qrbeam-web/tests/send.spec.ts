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
  await expect(page.locator("#qr")).toHaveJSProperty("width", 1080);
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
