import { expect, test } from "@playwright/test";
import { manifestFrameIndex, manifestHoldTicks } from "../src/send/playback_schedule";

test("每个文件清单二维码至少保持 250 毫秒", () => {
  expect(manifestHoldTicks(30)).toBe(8);
  expect(manifestHoldTicks(60)).toBe(15);
  expect(Array.from({ length: 8 }, (_, tick) => manifestFrameIndex(tick, 30, 3))).toEqual(
    Array(8).fill(0),
  );
  expect(manifestFrameIndex(8, 30, 3)).toBe(1);
});
