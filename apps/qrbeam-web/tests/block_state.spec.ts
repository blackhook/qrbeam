import { expect, test } from "@playwright/test";

test("接收页将未完成但已有有效帧的区块标为 partial", async ({ page }) => {
  await page.goto("receive/");
  const result = await page.evaluate(async () => {
    const state = await import("../src/receive/block_state.ts");
    return state.blockClass(3, new Set([1]), new Set([3]));
  });

  expect(result).toBe("partial");
});
