import { expect, test } from "@playwright/test";

test("从 ZXing 字节段读取 QRBeam 二进制载荷", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("send/");
  await page.locator("#file").setInputFiles({
    name: "binary-frame.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.alloc(1024, 7),
  });
  await expect(page.locator("#qr-player")).toHaveClass(/active/, { timeout: 15_000 });
  await page.locator("#pause").click();

  const payload = await page.evaluate(async () => {
    const { BrowserMultiFormatReader } = await import("/qrbeam/node_modules/.vite/deps/@zxing_browser.js");
    const { payloadFromResult } = await import("../src/receive/decoded_payload.ts");
    const result = new BrowserMultiFormatReader().decodeFromCanvas(
      document.querySelector<HTMLCanvasElement>("#qr")!,
    );
    return Array.from(payloadFromResult(result) ?? []);
  });

  expect(payload.slice(0, 4)).toEqual([0x51, 0x52, 0x42, 0x4d]);
});
