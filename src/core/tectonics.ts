import { cross, readVector } from './vector';
import type { Surface } from './surface';

export interface Tectonics {
  owners: Uint32Array;
  seeds: Uint32Array;
  angularVelocities: Float64Array;
  boundaryCells: Uint32Array;
  boundaryDirections: Float64Array;
  boundaryMotion: Float64Array;
  boundaryTypes: Uint32Array;
}
export const BOUNDARY_NAMES = ['Quiet', 'Convergent', 'Divergent', 'Transform-dominant'] as const;
export const BOUNDARY_COLORS = ['#82919a', '#ef927e', '#77c6e8', '#e5cd79'] as const;

/** Read-only derived display value. Angular velocity is authoritative native data. */
export function speedCmPerYear(surface: Surface, t: Tectonics, id: number): number {
  return Math.hypot(...cross(readVector(t.angularVelocities, t.owners[id]), readVector(surface.centers, id))) * surface.radiusMeters * 100;
}

export function summarizeTectonics(surface: Surface, t: Tectonics): {
  plates: { id: number; seedRegion: number; regionCount: number; areaSquareMeters: number }[];
  boundarySegmentCounts: Record<string, number>;
} {
  const plates = Array.from(t.seeds, (seedRegion, id) => ({ id, seedRegion, regionCount: 0, areaSquareMeters: 0 }));
  for (let id = 0; id < t.owners.length; id++) {
    const plate = plates[t.owners[id]]; plate.regionCount++; plate.areaSquareMeters += surface.areasSquareMeters[id];
  }
  const boundarySegmentCounts = Object.fromEntries(BOUNDARY_NAMES.map((name) => [name, 0]));
  for (const kind of t.boundaryTypes) boundarySegmentCounts[BOUNDARY_NAMES[kind]]++;
  return { plates, boundarySegmentCounts };
}
