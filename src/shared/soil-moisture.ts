/** Separate unified-owner contract; never reinterpret legacy moisture fields. */
export const SOIL_MOISTURE_CONTRACT = {
  modelVersion: 'regional-seasonal-water-1', schema: 1, protocol: 15,
  displayKind: 'soilMoisture', checkpointKind: 'soilMoistureCheckpoint',
} as const;
export const SOIL_STOCKS = ['liquid', 'soil', 'snow', 'vapor', 'drainage'] as const;
export const SOIL_TRANSFERS = ['rain', 'snowfall', 'melt', 'liquidEvaporation',
  'soilEvaporation', 'infiltration', 'soilDrainage'] as const;
export const SOIL_MOISTURE_PINS = {
  soilModelVersion: 'ponded-soil-exchange-2', transportModelVersion: 'moisture-transport-paired-2',
  surfaceFlowModelVersion: 'regional-surface-flow-paired-2', runoffModelVersion: 'runoff-transport-paired-2',
  observationModelVersion: 'regional-soil-observation-1', orographicModelVersion: 'orographic-response-1',
  referenceBodyModelVersion: 'reference-water-pool-1',
} as const;
export const DEFAULT_SOIL_MOISTURE_SETTINGS = {
  numericalPolicy: 'retainDonor', initialActiveSurfaceDepthMeters: 10, effectiveVaporDepthMeters: 2000,
  evaporationResponseSeconds: 432000, precipitationResponseSeconds: 21600,
  evaporationEnabled: true, precipitationEnabled: true, routingEnabled: true,
  maxCoupledStepSeconds: 900, meltKilogramsPerSquareMeterDegreeDay: 3,
  soil: { soilCapacityKilogramsPerSquareMeter: 150, soilRetainedFraction: 0.6,
    infiltrationResponseSeconds: 21600, soilDrainageResponseSeconds: 2592000 },
  transport: { maxOutgoingFraction: 0.8, maxSubsteps: 4096 },
  runoff: { effectiveSpeedMetersPerSecond: 1, minimumResponseSeconds: 3600 },
  surfaceFlow: { roughness: 0.04, maximumDiffusivitySquareMetersPerSecond: 1e6 },
  orography: { upliftResponseHeightMeters: 1000, strength: 1 },
} as const;
export interface PairedMassField { high: Float64Array; low: Float64Array }
export interface SoilResolution {
  deferredRequests: number;
  summedDeferredRequestKilograms: number;
  maximumDeferredRequestKilograms: number;
}
export interface SoilBudget {
  initialKilograms: number;
  storedKilograms: number;
  relativeGlobalResidual: number;
  maximumRelativeLocalResidual: number;
}
export interface SoilMoistureFrame {
  epoch: number;
  modelVersion: typeof SOIL_MOISTURE_CONTRACT.modelVersion;
  settings: typeof DEFAULT_SOIL_MOISTURE_SETTINGS;
  elapsedSeconds: number;
  intervalSeconds: number;
  coupledSubsteps: number;
  atmosphericSubsteps: number;
  surfaceSubsteps: number;
  stocks: Record<typeof SOIL_STOCKS[number], PairedMassField>;
  cumulativeLocalTransfers: Record<typeof SOIL_TRANSFERS[number], PairedMassField>;
  intervalLocalTransfers: Record<typeof SOIL_TRANSFERS[number], Float64Array>;
  intervalDrainageSentKilograms: Float64Array;
  regionalLiquidDepthMeters: Float64Array;
  visibleWaterDepthMeters: Float64Array;
  visibleWaterLevelMeters: Float64Array;
  cumulativeSurfaceIncomingKilograms: Float64Array;
  cumulativeSurfaceOutgoingKilograms: Float64Array;
  referenceBodyIds: Uint32Array;
  referenceBodies: PairedMassField;
  budget: SoilBudget;
  resolution: SoilResolution;
}
