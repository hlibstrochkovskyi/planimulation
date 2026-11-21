import type { Surface } from './surface';

export const SEDIMENT_MODEL_VERSION = 'sediment-1';
export const MAX_SEDIMENT_METERS = 12_000;

/** All fields are meters relative to a reference sphere, not sea level. */
export interface Terrain {
  baseline: Float64Array;
  convergence: Float64Array;
  divergence: Float64Array;
  detail: Float64Array;
  preparation: Float64Array;
  /** Coastal-lowland compression (zero or negative); present only for multiscale-geography-4. */
  lowland?: Float64Array;
  /** Ocean-floor contribution; present only for multiscale-geography-5. */
  seafloor?: Float64Array;
  /** Signed fluvial relief adjustment; present only for multiscale-geography-5. */
  erosion?: Float64Array;
  /** Routed sediment thickness; present only for multiscale-geography-5. */
  sediment?: Float64Array;
  sedimentModelVersion?: typeof SEDIMENT_MODEL_VERSION;
  elevation: Float64Array;
  transportedCubicMeters: number;
  appliedPasses: number;
}
export function summarizeTerrain(surface: Surface, terrain: Terrain): { minimumMeters: number; maximumMeters: number; meanMeters: number } {
  let min = Infinity, max = -Infinity, weighted = 0, area = 0;
  for (let id = 0; id < terrain.elevation.length; id++) {
    const h = terrain.elevation[id]; min = Math.min(min, h); max = Math.max(max, h);
    weighted += h * surface.areasSquareMeters[id]; area += surface.areasSquareMeters[id];
  }
  return { minimumMeters: min, maximumMeters: max, meanMeters: weighted / area };
}
