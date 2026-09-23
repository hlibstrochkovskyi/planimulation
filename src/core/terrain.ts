import type { Surface } from './surface';

/** All fields are meters relative to a reference sphere, not sea level. */
export interface Terrain {
  baseline: Float64Array;
  convergence: Float64Array;
  divergence: Float64Array;
  detail: Float64Array;
  elevation: Float64Array;
}
export function summarizeTerrain(surface: Surface, terrain: Terrain): { minimumMeters: number; maximumMeters: number; meanMeters: number } {
  let min = Infinity, max = -Infinity, weighted = 0, area = 0;
  for (let id = 0; id < terrain.elevation.length; id++) {
    const h = terrain.elevation[id]; min = Math.min(min, h); max = Math.max(max, h);
    weighted += h * surface.areasSquareMeters[id]; area += surface.areasSquareMeters[id];
  }
  return { minimumMeters: min, maximumMeters: max, meanMeters: weighted / area };
}
