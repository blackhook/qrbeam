import { BarcodeFormat, DecodeHintType } from "@zxing/library";
import { expect, test } from "@playwright/test";
import { createQrReader, receiverVideoConstraints } from "../src/receive/web_scanner";

test("网页接收器请求后置高清相机并启用高强度 QR 定位", () => {
  const reader = createQrReader() as unknown as { hints: Map<DecodeHintType, unknown> };
  const video = receiverVideoConstraints.video as MediaTrackConstraints;

  expect(video.facingMode).toEqual({ ideal: "environment" });
  expect(video.width).toEqual({ ideal: 1920 });
  expect(video.height).toEqual({ ideal: 1080 });
  expect(reader.hints.get(DecodeHintType.TRY_HARDER)).toBe(true);
  expect(reader.hints.get(DecodeHintType.POSSIBLE_FORMATS)).toEqual([BarcodeFormat.QR_CODE]);
});
