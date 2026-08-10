export const MIN_MODULE_PHYSICAL_PIXELS = 6;
const QUIET_ZONE_MODULES = 4;

export type PlaybackProfile = {
  id: number;
  symbolsPerFrame: number;
  targetFps: number;
};

export type AdaptiveInput = {
  canvasCssPixels: number;
  devicePixelRatio: number;
  refreshRate: number;
  profiles: PlaybackProfile[];
  moduleCounts: Array<[number, number]>;
};

export type AdaptiveSelection = {
  kind: "selected";
  profileId: number;
  fps: number;
  modules: number;
  modulePhysicalPixels: number;
};

export type AdaptiveFailure = {
  kind: "unavailable";
  modulePhysicalPixels: number;
};

export function choosePlaybackProfile(input: AdaptiveInput): AdaptiveSelection | AdaptiveFailure {
  const moduleCounts = new Map(input.moduleCounts);
  const physicalPixels = Math.floor(input.canvasCssPixels * input.devicePixelRatio);
  let bestAvailable = 0;
  for (const profile of [...input.profiles].sort((left, right) => right.symbolsPerFrame - left.symbolsPerFrame)) {
    const modules = moduleCounts.get(profile.id);
    if (!modules) continue;
    const modulePhysicalPixels = physicalPixels / (modules + 2 * QUIET_ZONE_MODULES);
    bestAvailable = Math.max(bestAvailable, modulePhysicalPixels);
    if (modulePhysicalPixels >= MIN_MODULE_PHYSICAL_PIXELS) {
      return {
        kind: "selected",
        profileId: profile.id,
        fps: Math.min(profile.targetFps, Math.max(1, Math.floor(input.refreshRate))),
        modules,
        modulePhysicalPixels,
      };
    }
  }
  return { kind: "unavailable", modulePhysicalPixels: bestAvailable };
}
