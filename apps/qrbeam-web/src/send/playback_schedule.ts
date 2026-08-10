const MANIFEST_HOLD_MS = 250;

export function manifestHoldTicks(fps: number) {
  return Math.max(1, Math.ceil((fps * MANIFEST_HOLD_MS) / 1_000));
}

export function manifestRoundTicks(fps: number, frameCount: number) {
  return manifestHoldTicks(fps) * frameCount;
}

export function manifestFrameIndex(displayTick: number, fps: number, frameCount: number) {
  if (frameCount < 1) throw new Error("manifest requires at least one frame");
  return Math.floor(displayTick / manifestHoldTicks(fps)) % frameCount;
}
