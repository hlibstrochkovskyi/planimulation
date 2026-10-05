import type { World } from '../core/world';
import type { MoistureFrame } from '../shared/seasonal-moisture';

/** Presentation only. A wet mask is not a fabricated connected-body label. */
export interface RegionalWaterDisplay {
  wetMask: Uint8Array;
  depthMeters: Float64Array;
  surfaceLevelsMeters: Float64Array;
}
export function regionalWaterDisplay(world: World, frame: MoistureFrame): RegionalWaterDisplay {
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
