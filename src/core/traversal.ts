export const TRAVERSAL_MODEL_VERSION = 'traversal-1';
export const TRAVERSAL_MAX_SOURCES = 64;

export type TravelSeason = 'summer' | 'winter';

/** Native traversal-1 defaults; the core rejects any other desktop settings. */
export const DEFAULT_TRAVERSAL_SETTINGS = {
  hoursPerDay: 8,
  reliefScaleMeters: 600,
  riverThresholdSquareMeters: 1e11,
  riverCrossingHoursPerDecade: 8,
  alongRiverFactor: 0.25,
  snowSlowdownMax: 1.5,
  snowMonthsSaturation: 6,
  snowPrecipSaturationMm: 300,
  freezeTempCelsius: -10,
  freezeMinMonths: 3,
  frozenLakeCoverFactor: 0.75,
} as const;
export type TraversalSettings = typeof DEFAULT_TRAVERSAL_SETTINGS;

export const TRAVERSAL_FLAG_PASSABLE_SUMMER = 1 << 0;
export const TRAVERSAL_FLAG_PASSABLE_WINTER = 1 << 1;
export const TRAVERSAL_FLAG_FROZEN_LAKE_WINTER = 1 << 2;
export const TRAVERSAL_FLAG_FROZEN_RIVER_WINTER = 1 << 3;
export const TRAVERSAL_FLAG_FREEZING_MONTHS_SHIFT = 4;

/** Stepped isochrone bands for the travel-time layer (days from origin). */
export const ISOCHRONE_BANDS = [
  { maxDays: 1, label: '≤ 1 d', color: [250, 238, 150] as const },
  { maxDays: 3, label: '1–3 d', color: [242, 206, 102] as const },
  { maxDays: 7, label: '3–7 d', color: [236, 162, 68] as const },
  { maxDays: 14, label: '7–14 d', color: [222, 114, 62] as const },
  { maxDays: 30, label: '14–30 d', color: [190, 68, 86] as const },
  { maxDays: 60, label: '30–60 d', color: [140, 48, 108] as const },
  { maxDays: 120, label: '60–120 d', color: [88, 44, 122] as const },
  { maxDays: Infinity, label: '> 120 d', color: [46, 38, 92] as const },
] as const;
export const UNREACHABLE_COLOR = [88, 94, 102] as const;

/** Per-region foot-traversal costs and factor breakdown for summer and winter. */
export interface TraversalCosts {
  epoch: number;
  modelVersion: typeof TRAVERSAL_MODEL_VERSION;
  usesVegetation: boolean;
  vegetationYear: number | null;
  settings: TraversalSettings;
  /** Representative hours per 100 km (`-1` when impassable on foot). */
  hoursPer100KmSummer: Float64Array;
  hoursPer100KmWinter: Float64Array;
  /** Combined Tobler slope and sub-grid ruggedness multiplier (`-1` when impassable). */
  slopeFactorSummer: Float64Array;
  slopeFactorWinter: Float64Array;
  /** Land-cover multiplier (`-1` when impassable). */
  coverFactorSummer: Float64Array;
  coverFactorWinter: Float64Array;
  /** Winter snow speed multiplier in `(0, 1]`. */
  snowFactorWinter: Float64Array;
  /** River-crossing proxy delay in hours (0 when below threshold or frozen). */
  riverDelaySummerHours: Float64Array;
  riverDelayWinterHours: Float64Array;
  /** Mean elevation difference to neighbors, in meters. */
  localReliefMeters: Float64Array;
  /** Packed passability, deep-winter freeze, and freezing-month flags. */
  flags: Uint32Array;
}

/** Deterministic least-time Dijkstra field from one or more origin regions. */
export interface TravelField {
  epoch: number;
  modelVersion: typeof TRAVERSAL_MODEL_VERSION;
  season: TravelSeason;
  sources: number[];
  usesVegetation: boolean;
  vegetationYear: number | null;
  /** Travel time from the nearest origin in days (`-1` when unreachable). */
  days: Float64Array;
  /** Accumulated spherical route distance in meters (`-1` when unreachable). */
  distanceMeters: Float64Array;
  /** Predecessor region ID along the least-time route (`regionCount` when unreachable). */
  predecessor: Uint32Array;
}

export function isRegionPassable(costs: TraversalCosts, id: number, season: TravelSeason): boolean {
  const bit = season === 'summer' ? TRAVERSAL_FLAG_PASSABLE_SUMMER : TRAVERSAL_FLAG_PASSABLE_WINTER;
  return (costs.flags[id] & bit) !== 0;
}

export function isFrozenLakeWinter(costs: TraversalCosts, id: number): boolean {
  return (costs.flags[id] & TRAVERSAL_FLAG_FROZEN_LAKE_WINTER) !== 0;
}

export function isFrozenRiverWinter(costs: TraversalCosts, id: number): boolean {
  return (costs.flags[id] & TRAVERSAL_FLAG_FROZEN_RIVER_WINTER) !== 0;
}

export function freezingMonths(costs: TraversalCosts, id: number): number {
  return (costs.flags[id] >>> TRAVERSAL_FLAG_FREEZING_MONTHS_SHIFT) & 0xf;
}

/** Reconstruct the least-time path from the nearest origin to `target`, or null if unreachable. */
export function traceTravelRoute(field: TravelField, target: number): number[] | null {
  const n = field.days.length;
  if (!Number.isInteger(target) || target < 0 || target >= n || field.days[target] < 0) return null;
  const path: number[] = [];
  let cur = target;
  for (let step = 0; step <= n; step++) {
    path.push(cur);
    const prev = field.predecessor[cur];
    if (prev >= n) return null;
    if (prev === cur) {
      path.reverse();
      return path;
    }
    cur = prev;
  }
  return null;
}
