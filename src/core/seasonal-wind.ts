export const WIND_MODEL_VERSION = 'seasonal-wind-1';

export interface WindSettings {
  itczShiftFraction: number;
  tradeEasterlyMetersPerSecond: number;
  midlatitudeWesterlyMetersPerSecond: number;
  polarEasterlyMetersPerSecond: number;
  tropicalConvergenceMetersPerSecond: number;
  midlatitudePolewardMetersPerSecond: number;
  polarEquatorwardMetersPerSecond: number;
}

export const DEFAULT_WIND_SETTINGS: Readonly<WindSettings> = Object.freeze({
  itczShiftFraction: 0.5,
  tradeEasterlyMetersPerSecond: 7,
  midlatitudeWesterlyMetersPerSecond: 10,
  polarEasterlyMetersPerSecond: 5,
  tropicalConvergenceMetersPerSecond: 2,
  midlatitudePolewardMetersPerSecond: 1,
  polarEquatorwardMetersPerSecond: 0.5,
});

/** East/north components of a prescribed surface-wind normal; no atmospheric state is evolved. */
export interface WindNormals {
  epoch: number;
  modelVersion: typeof WIND_MODEL_VERSION;
  settings: WindSettings;
  axialTiltDegrees: number;
  monthlyDayCounts: number[];
  monthlyEastMetersPerSecond: Float64Array[];
  monthlyNorthMetersPerSecond: Float64Array[];
}
