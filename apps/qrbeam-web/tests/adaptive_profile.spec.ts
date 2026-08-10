import { expect, test } from "@playwright/test";

test("优先选择模块物理像素达标的最高吞吐档位", async ({ page }) => {
  await page.goto("send/");
  const result = await page.evaluate(async () => {
    const adaptive = await import("../src/send/adaptive_profile.ts");
    return adaptive.choosePlaybackProfile({
      canvasCssPixels: 480,
      devicePixelRatio: 2,
      refreshRate: 60,
      profiles: [
        { id: 3, symbolsPerFrame: 11, targetFps: 60 },
        { id: 1, symbolsPerFrame: 5, targetFps: 24 },
        { id: 0, symbolsPerFrame: 1, targetFps: 8 },
      ],
      moduleCounts: [[3, 177], [1, 121], [0, 77]],
    });
  });

  expect(result).toMatchObject({ kind: "selected", profileId: 1, fps: 24 });
  expect((result as { modulePhysicalPixels: number }).modulePhysicalPixels).toBeGreaterThanOrEqual(6);
});

test("没有任何档位满足模块阈值时拒绝开始发送", async ({ page }) => {
  await page.goto("send/");
  const result = await page.evaluate(async () => {
    const adaptive = await import("../src/send/adaptive_profile.ts");
    return adaptive.choosePlaybackProfile({
      canvasCssPixels: 200,
      devicePixelRatio: 1,
      refreshRate: 60,
      profiles: [{ id: 0, symbolsPerFrame: 1, targetFps: 8 }],
      moduleCounts: [[0, 77]],
    });
  });

  expect(result).toMatchObject({ kind: "unavailable" });
});

test("动态二维码至少保持两个显示刷新周期", async ({ page }) => {
  await page.goto("send/");
  const result = await page.evaluate(async () => {
    const adaptive = await import("../src/send/adaptive_profile.ts");
    return adaptive.choosePlaybackProfile({
      canvasCssPixels: 900,
      devicePixelRatio: 2,
      refreshRate: 60,
      profiles: [{ id: 3, symbolsPerFrame: 11, targetFps: 60 }],
      moduleCounts: [[3, 121]],
    });
  });

  expect(result).toMatchObject({ kind: "selected", profileId: 3, fps: 30 });
});

test("即使屏幕足够大，也不选择超过手机可稳定位数量的二维码", async ({ page }) => {
  await page.goto("send/");
  const result = await page.evaluate(async () => {
    const adaptive = await import("../src/send/adaptive_profile.ts");
    return adaptive.choosePlaybackProfile({
      canvasCssPixels: 1800,
      devicePixelRatio: 2,
      refreshRate: 60,
      profiles: [
        { id: 3, symbolsPerFrame: 11, targetFps: 60 },
        { id: 1, symbolsPerFrame: 5, targetFps: 24 },
      ],
      moduleCounts: [[3, 177], [1, 121]],
    });
  });

  expect(result).toMatchObject({ kind: "selected", profileId: 1, modules: 121, fps: 24 });
});
