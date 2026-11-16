export const VEGETATION_MODEL_VERSION = 'vegetation-1';
export const VEGETATION_MAX_YEARS = 10_000;
export const VEGETATION_MAX_YEARS_PER_REQUEST = 1_000;

/** Native vegetation-1 defaults; the core rejects any other desktop settings. */
export const DEFAULT_VEGETATION_SETTINGS = {
  anomalyCenters: 240,
  anomalyLengthMeters: 1_500_000,
  anomalyPersistence: 0.4,
  precipitationVariability: 0.22,
  woodyGrowthPerYear: 0.15,
  herbGrowthPerYear: 1.5,
  woodyDiebackPerYear: 0.3,
  herbDiebackPerYear: 0.8,
  establishment: 0.02,
  rootMemoryYears: 3,
  labelMemoryYears: 8,
} as const;
export type VegetationSettings = typeof DEFAULT_VEGETATION_SETTINGS;
/** Aridity ratios (precipitation / Köppen threshold) where capacities ramp from 0 to 1. */
export const WOODY_ARIDITY = [1.5, 2.5] as const;
export const COVER_ARIDITY = [0.6, 1.4] as const;
/** Forest and open labels switch on at the first value and keep until below the second. */
export const FOREST_COVER = [0.5, 0.4] as const;
export const OPEN_COVER = [0.5, 0.4] as const;

/** Land-area-weighted global record of one simulated year. */
export interface VegetationYear {
  precipitationFactor: number;
  woodyCover: number;
  totalCover: number;
  forestShare: number;
  labelChanges: number;
}

/**
 * Yearly vegetation state on the static world. Labels reuse the climate-normals-1 class
 * indices but describe accumulated cover with hysteresis, not the climate class.
 */
export interface VegetationFrame {
  epoch: number;
  modelVersion: typeof VEGETATION_MODEL_VERSION;
  year: number;
  woody: Float64Array;
  herb: Float64Array;
  /** This year's precipitation as a fraction of the climate normal. */
  precipitationFactor: Float64Array;
  /** Exponential memory of the precipitation factor seen by woody roots. */
  rootMoisture: Float64Array;
  /** Exponential memory of total cover deciding open-vegetation labels. */
  establishedCover: Float64Array;
  woodyCapacity: Float64Array;
  coverCapacity: Float64Array;
  label: Uint32Array;
  summaries: VegetationYear[];
}

export interface VegetationRegionYear {
  precipitationFactor: number;
  rootMoisture: number;
  establishedCover: number;
  woody: number;
  herb: number;
  woodyCapacity: number;
  coverCapacity: number;
  label: number;
}

/** One region replayed by the native core from year 0 to the current year. */
export interface VegetationSeries { epoch: number; region: number; years: VegetationRegionYear[] }
