import type { World } from '../core/world';
import type { SeasonalDisplayFrame } from '../shared/seasonal-display';
import { isSoilMoistureFrame } from '../shared/seasonal-display';

/** Presentation only. A wet mask is not a fabricated connected-body label. */
export interface RegionalWaterDisplay {
  wetMask: Uint8Array;
  depthMeters: Float64Array;
  surfaceLevelsMeters: Float64Array;
}
export function regionalWaterDisplay(world: World, frame: SeasonalDisplayFrame): RegionalWaterDisplay {
  if (isSoilMoistureFrame(frame)) {
    const n = world.stats.regionCount;
    if ([frame.visibleWaterDepthMeters, frame.visibleWaterLevelMeters, frame.stocks.liquid.high, frame.stocks.liquid.low]
      .some(f => f.length !== n)) throw new Error('Soil-water display requires matching native fields.');
    const depthMeters = frame.visibleWaterDepthMeters.slice(), surfaceLevelsMeters = frame.visibleWaterLevelMeters.slice();
    const wetMask = new Uint8Array(n);
    for (let r = 0; r < n; r++) {
      wetMask[r] = Number(world.water.bodyIds[r] > 0 || depthMeters[r] > 0);
      if (depthMeters[r] === 0) surfaceLevelsMeters[r] = world.terrain.elevation[r];
    }
    return { wetMask, depthMeters, surfaceLevelsMeters };
  }
  const regional = frame.regionalSurface;
  const n = world.stats.regionCount;
  if (!regional || [regional.depthMeters, regional.levelsMeters, frame.stocks.terminalWaterKilograms]
    .some((field) => field.length !== n)) {
    throw new Error('Regional water display requires matching native fields.');
  }
  const wetMask = new Uint8Array(n), depthMeters = new Float64Array(n), surfaceLevelsMeters = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const reference = world.water.bodyIds[i] !== 0;
    wetMask[i] = Number(reference || frame.stocks.terminalWaterKilograms[i] > 0);
    depthMeters[i] = reference ? world.water.depthMeters[i] : regional.depthMeters[i];
    surfaceLevelsMeters[i] = reference ? world.water.levelMeters
      : regional.depthMeters[i] > 0 ? regional.levelsMeters[i] : world.terrain.elevation[i];
  }
  return { wetMask, depthMeters, surfaceLevelsMeters };
}
