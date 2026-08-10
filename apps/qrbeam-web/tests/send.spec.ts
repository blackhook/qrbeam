import { expect, test } from "@playwright/test";

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
