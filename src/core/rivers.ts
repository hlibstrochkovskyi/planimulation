import { DEFAULT_TEMPERATURE_SETTINGS } from './seasonal-temperature';

export const RIVER_MODEL_VERSION = 'rivers-1' as const;

export const DEFAULT_RIVER_SETTINGS = {
  soilCapacityMm: 100,
  soilRechargeExponent: 2,
  soilEtFullFraction: 0.7,
  soilStepsPerMonth: 4,
  snowMeltMmPerDegreeDay: 3,
  snowThresholdCelsius: 0,
  petTemperatureOffsetCelsius: 5,
  petTemperatureScaleCelsius: 100,
  latentHeatMjPerKg: 2.45,
  axialTiltDegrees: DEFAULT_TEMPERATURE_SETTINGS.axialTiltDegrees,
  solarIrradianceWattsPerSquareMeter: DEFAULT_TEMPERATURE_SETTINGS.solarIrradianceWattsPerSquareMeter,
  lakePetFactor: 1.15,
  dryLakeAreaFraction: 0.1,
  riverThresholdCubicMetersPerSecond: 1,
} as const;

export const RIVER_CLASSES = ['None', 'Seasonal / ephemeral', 'Perennial'] as const;
export const RIVER_TOTAL_KEYS = ['landArea', 'landPrecipitation', 'landAet', 'landRunoff', 'landStorage',
  'oceanDischarge', 'landRunoffToOcean', 'landRunoffToEndorheic', 'landRunoffEvaporatedInSpillingLakes', 'endorheicArea', 'lakePrecipitation', 'lakeEvaporation', 'terminalStorage'] as const;

/** Annual volumes in m³, with areas in m². */
export type RiverTotals = Record<typeof RIVER_TOTAL_KEYS[number], number>;

export interface LakeBalance {
  /** Canonical static terminal region ID. */
  region: number;
  outlet: readonly [number, number] | null;
  sillElevation: number;
  area: number;
  inflow: number;
  precipitation: number;
  potentialEvaporation: number;
  evaporation: number;
  spill: number;
  storage: number;
}

export interface SpillSegment { from: number; to: number; class: number; meanDischarge: number }

export interface Rivers {
  epoch: number;
  modelVersion: typeof RIVER_MODEL_VERSION;
  settings: typeof DEFAULT_RIVER_SETTINGS;
  monthDays: number[];
  oceanTerminal: number | null;
  pet: Float64Array;
  aet: Float64Array;
  runoff: Float64Array;
  storage: Float64Array;
  meanDischarge: Float64Array;
  /** Region-major monthly values, in mm/month and m³/s respectively. */
  monthlyRunoff: Float64Array;
  monthlyDischarge: Float64Array;
  class: Uint32Array;
  terminal: Uint32Array;
  lakes: Map<number, LakeBalance>;
  spillSegments: SpillSegment[];
  totals: RiverTotals;
}
