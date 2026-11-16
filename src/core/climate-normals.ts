export const CLIMATE_MODEL_VERSION = 'climate-normals-1';

/** Climate classes in native order; index 0 marks initial water regions. */
export const BIOMES = [
  { name: 'Water', color: [0, 0, 0] },
  { name: 'Ice', color: [236, 241, 245] },
  { name: 'Tundra', color: [166, 172, 150] },
  { name: 'Boreal forest', color: [62, 104, 82] },
  { name: 'Temperate forest', color: [76, 132, 62] },
  { name: 'Temperate grassland', color: [168, 180, 100] },
  { name: 'Cold desert', color: [190, 178, 146] },
  { name: 'Hot desert', color: [234, 208, 152] },
  { name: 'Savanna', color: [198, 160, 74] },
  { name: 'Tropical seasonal forest', color: [120, 150, 54] },
  { name: 'Tropical rainforest', color: [36, 112, 52] },
] as const;

/**
 * Prescribed annual climate normals on the static initial world. Precipitation is a
 * diagnostic estimate (zonal potential × upwind moisture supply × windward lift), not
 * the seasonal-water budget; biomes classify climate, not simulated vegetation.
 */
export interface ClimateNormals {
  epoch: number;
  modelVersion: typeof CLIMATE_MODEL_VERSION;
  annualMeanCelsius: Float64Array;
  warmestMonthCelsius: Float64Array;
  coldestMonthCelsius: Float64Array;
  annualPrecipitationMm: Float64Array;
  driestMonthMm: Float64Array;
  zonalPrecipitationMm: Float64Array;
  moistureSupply: Float64Array;
  lift: Float64Array;
  biome: Uint32Array;
}

/** Köppen-style aridity threshold (mm/year) used by the native classifier. */
export function aridityThreshold(meanCelsius: number): number { return 20 * meanCelsius + 140; }
