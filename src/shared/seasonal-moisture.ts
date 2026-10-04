import type { TemperatureSettings } from '../core/seasonal-temperature';
import type { WindSettings } from '../core/seasonal-wind';

export const MOISTURE_MODEL_VERSION = 'seasonal-moisture-3';
export const OROGRAPHIC_MOISTURE_MODEL_VERSION = 'seasonal-moisture-4';
export const OROGRAPHIC_RESPONSE_MODEL_VERSION = 'orographic-response-1';
export const MOISTURE_MAX_SECONDS = 3650 * 86400;
export const MOISTURE_STOCK_FIELDS = ['surfaceKilograms', 'snowKilograms', 'soilKilograms',
  'pendingRunoffKilograms', 'terminalWaterKilograms', 'vaporKilograms'] as const;
export const SURFACE_TRANSFER_FIELDS = ['rain', 'snowfall', 'melt', 'liquidEvaporation',
  'soilEvaporation', 'infiltration', 'liquidRunoff', 'soilDrainage'] as const;
export const RUNOFF_TRANSFER_FIELDS = ['sent', 'receivedTransit', 'terminalDelivery', 'terminalEvaporation'] as const;
export type MoistureStock = typeof MOISTURE_STOCK_FIELDS[number];
export type SurfaceTransfer = typeof SURFACE_TRANSFER_FIELDS[number];
export type RunoffTransfer = typeof RUNOFF_TRANSFER_FIELDS[number];

/** The desktop currently exposes the same pinned defaults as the native report. */
export const DEFAULT_MOISTURE_SETTINGS = {
  initialActiveSurfaceDepthMeters: 1, effectiveVaporDepthMeters: 2000,
  evaporationResponseSeconds: 432000, precipitationResponseSeconds: 21600,
  evaporationEnabled: true, precipitationEnabled: true, maxCoupledStepSeconds: 3600,
  transport: { maxOutgoingFraction: 0.8, maxSubsteps: 4096 },
  surface: { meltKilogramsPerSquareMeterDegreeDay: 3, soilCapacityKilogramsPerSquareMeter: 150,
    soilRetainedFraction: 0.6, infiltrationResponseSeconds: 21600,
    liquidRunoffResponseSeconds: 86400, soilDrainageResponseSeconds: 2592000 },
  routingEnabled: true, runoff: { effectiveSpeedMetersPerSecond: 1, minimumResponseSeconds: 3600 },
} as const;
export const DEFAULT_OROGRAPHIC_MOISTURE_SETTINGS = {
  ...DEFAULT_MOISTURE_SETTINGS, orography: { upliftResponseHeightMeters: 1000, strength: 1 },
} as const;

export interface MoistureBudget extends Record<MoistureStock, number> {
  initialMobileWaterKilograms: number;
  residualKilograms: number;
  cumulativeEvaporationKilograms: number;
  cumulativePrecipitationKilograms: number;
  cumulativeSurfaceTransfers: Record<SurfaceTransfer, number>;
  cumulativeRunoffTransfers: Record<RunoffTransfer, number>;
  maximumRelativeLocalSurfaceLedgerResidual: number;
  vaporLedgerResidualKilograms: number;
}

/** Read-only presentation data. No geometry, cumulative regional corrections,
 * RNG state, or resumable simulation state is transferred here. */
export interface MoistureFrame {
  epoch: number;
  modelVersion: typeof MOISTURE_MODEL_VERSION | typeof OROGRAPHIC_MOISTURE_MODEL_VERSION;
  settings: typeof DEFAULT_MOISTURE_SETTINGS | typeof DEFAULT_OROGRAPHIC_MOISTURE_SETTINGS;
  temperatureSettings: TemperatureSettings;
  windSettings: WindSettings;
  elapsedSeconds: number;
  intervalSeconds: number;
  coupledSubsteps: number;
  transportSubsteps: number;
  maximumAbsoluteLocalExchangeResidualKilograms: number;
  maximumAbsoluteRoutingResidualKilograms: number;
  stocks: Record<MoistureStock, Float64Array>;
  surfaceTransfers: Record<SurfaceTransfer, Float64Array>;
  runoffTransfers: Record<RunoffTransfer, Float64Array>;
  budget: MoistureBudget;
}
