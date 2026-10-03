export const TEMPERATURE_MODEL_VERSION = 'seasonal-temperature-1';
export const TEMPERATURE_DAYS_PER_YEAR = 365;
export const TEMPERATURE_MONTHS_PER_YEAR = 12;

export interface TemperatureSettings {
  axialTiltDegrees: number;
  solarIrradianceWattsPerSquareMeter: number;
  referenceTemperatureCelsius: number;
  sensitivityCelsiusPerWattPerSquareMeter: number;
  lapseRateCelsiusPerMeter: number;
  landResponseDays: number;
  waterResponseDays: number;
}

export const DEFAULT_TEMPERATURE_SETTINGS: Readonly<TemperatureSettings> = Object.freeze({
  axialTiltDegrees: 23.44,
  solarIrradianceWattsPerSquareMeter: 1360,
  referenceTemperatureCelsius: 15,
  sensitivityCelsiusPerWattPerSquareMeter: 0.09,
  lapseRateCelsiusPerMeter: 0.0065,
  landResponseDays: 20,
  waterResponseDays: 60,
});

/** A repeatable annual normal on the generated initial geography, not mutable weather. */
export interface TemperatureNormals {
  epoch: number;
  modelVersion: typeof TEMPERATURE_MODEL_VERSION;
  settings: TemperatureSettings;
  monthlyDayCounts: number[];
  monthlyTemperatureCelsius: Float64Array[];
  annualMeanCelsius: Float64Array;
  annualMinimumCelsius: Float64Array;
  annualMaximumCelsius: Float64Array;
}
