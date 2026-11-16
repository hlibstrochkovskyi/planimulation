export const VEGETATION_MODEL_VERSION = 'vegetation-2';
export const VEGETATION_MAX_YEARS = 2_000;
export const VEGETATION_MAX_YEARS_PER_REQUEST = 1_000;

/** Native vegetation-2 defaults; the core rejects any other desktop settings. */
export const DEFAULT_VEGETATION_SETTINGS = {
  anomalyCenters: 480,
  anomalyLengthMeters: 1_000_000,
  anomalyPersistence: 0.4,
  humidVariability: 0.12,
  aridVariability: 0.4,
  woodyGrowthPerYear: 0.15,
  herbGrowthPerYear: 1.5,
  woodyDiebackPerYear: 0.3,
  herbDiebackPerYear: 0.8,
  backgroundEstablishment: 0.002,
  seedEstablishment: 0.05,
  herbEstablishment: 0.02,
  rootMemoryYears: 3,
  labelMemoryYears: 8,
  spinUpYears: 200,
} as const;
export type VegetationSettings = typeof DEFAULT_VEGETATION_SETTINGS;
/** Aridity ratios (precipitation / Köppen threshold) where capacities ramp from 0 to 1. */
export const WOODY_ARIDITY = [1.5, 2.5] as const;
export const COVER_ARIDITY = [0.6, 1.4] as const;
/** Forest and open labels switch on at the first value and keep until below the second. */
export const FOREST_COVER = [0.5, 0.4] as const;
export const OPEN_COVER = [0.5, 0.4] as const;
/** Window of the woody-change field, years. */
export const CHANGE_YEARS = 10;
/** Normal annual precipitation (mm) where variability takes its humid and arid values. */
export const VARIABILITY_PRECIPITATION_MM = [2000, 200] as const;
/** Normal cover below which productivity is not scaled by its realized share. */
export const MINIMUM_REFERENCE_COVER = 0.05;

/** Miami model (Lieth 1975): temperature and precipitation limits in g dry matter / m² / year. */
export function miami(annualMeanCelsius: number, precipitationMm: number): [number, number] {
  return [3000 / (1 + Math.exp(1.315 - 0.119 * annualMeanCelsius)), 3000 * (1 - Math.exp(-0.000664 * precipitationMm))];
}

/** Land-area-weighted global record of one simulated year. */
export interface VegetationYear {
  precipitationFactor: number;
  woodyCover: number;
  totalCover: number;
  forestShare: number;
  labelChanges: number;
  /** Land net primary productivity, petagrams of dry matter per year. */
  productivityPetagrams: number;
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
  /** Woody cover now minus CHANGE_YEARS years ago. */
  woodyChange: Float64Array;
  /** Net primary productivity, g dry matter / m² / year. */
  productivity: Float64Array;
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
  productivity: number;
  label: number;
}

/** One region replayed by the native core from year 0 to the current year. */
export interface VegetationSeries { epoch: number; region: number; years: VegetationRegionYear[] }
