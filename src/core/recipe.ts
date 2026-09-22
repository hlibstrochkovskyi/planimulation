export const MODEL_VERSION = 'crust-1';
export const RANDOM_VERSION = 'fnv1a-utf8-mulberry32-1';

export interface Recipe {
  schemaVersion: 1;
  modelVersion: typeof MODEL_VERSION;
  randomVersion: typeof RANDOM_VERSION;
  seed: string;
  subdivision: number;
  radiusMeters: number;
  plateCount: number;
  maxPlateSpeedCmPerYear: number;
  continentalFraction: number;
  continentalScale: number;
}

export const DEFAULT_RECIPE: Readonly<Recipe> = Object.freeze({
  schemaVersion: 1,
  modelVersion: MODEL_VERSION,
  randomVersion: RANDOM_VERSION,
  seed: 'first-light',
  subdivision: 5,
  radiusMeters: 6_371_000,
  plateCount: 12,
  maxPlateSpeedCmPerYear: 8,
  continentalFraction: 0.38,
  continentalScale: 1,
});

/** Strict parsing prevents an old or misspelled parameter from being silently ignored. */
export function parseRecipe(value: unknown): Recipe {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('A recipe must be a JSON object.');
  }
  const input = value as Record<string, unknown>;
  const keys = Object.keys(DEFAULT_RECIPE);
  for (const key of Object.keys(input)) {
    if (!keys.includes(key)) throw new Error(`Unknown recipe field: ${key}.`);
  }
  if (input.schemaVersion !== 1 || input.modelVersion !== MODEL_VERSION || input.randomVersion !== RANDOM_VERSION) {
    throw new Error('Unsupported recipe version. This build supports crust-1 recipes only; legacy recipes are not silently migrated.');
  }
  for (const key of keys) {
    if (!(key in input)) throw new Error(`Missing recipe field: ${key}.`);
  }
  if (typeof input.seed !== 'string' || input.seed.trim().length === 0 || input.seed.length > 128) {
    throw new Error('Seed must contain 1–128 characters and cannot be blank.');
  }
  if (!Number.isInteger(input.subdivision) || Number(input.subdivision) < 0 || Number(input.subdivision) > 6) {
    throw new Error('Subdivision must be an integer from 0 to 6.');
  }
  if (typeof input.radiusMeters !== 'number' || !Number.isFinite(input.radiusMeters)
    || input.radiusMeters < 100_000 || input.radiusMeters > 20_000_000) {
    throw new Error('Radius must be between 100 and 20,000 km.');
  }
  if (!Number.isInteger(input.plateCount) || Number(input.plateCount) < 2 || Number(input.plateCount) > Math.min(32, 10 * 4 ** Number(input.subdivision) + 2)) {
    throw new Error('Plate count must be 2–32 and no larger than the region count.');
  }
  if (typeof input.maxPlateSpeedCmPerYear !== 'number' || !Number.isFinite(input.maxPlateSpeedCmPerYear)
    || input.maxPlateSpeedCmPerYear < 0 || input.maxPlateSpeedCmPerYear > 20) throw new Error('Maximum plate speed must be 0–20 cm/year.');
  if (typeof input.continentalFraction !== 'number' || !Number.isFinite(input.continentalFraction)
    || input.continentalFraction < 0 || input.continentalFraction > 1) throw new Error('Continental fraction must be 0–1, not a land-area target.');
  if (typeof input.continentalScale !== 'number' || !Number.isFinite(input.continentalScale)
    || input.continentalScale < 0.5 || input.continentalScale > 2) throw new Error('Continental scale must be 0.5–2.');
  return {
    schemaVersion: 1,
    modelVersion: MODEL_VERSION,
    randomVersion: RANDOM_VERSION,
    seed: input.seed,
    subdivision: Number(input.subdivision),
    radiusMeters: input.radiusMeters,
    plateCount: Number(input.plateCount),
    maxPlateSpeedCmPerYear: input.maxPlateSpeedCmPerYear,
    continentalFraction: input.continentalFraction,
    continentalScale: input.continentalScale,
  };
}

export function serializeRecipe(recipe: Recipe): string {
  return `${JSON.stringify(parseRecipe(recipe), null, 2)}\n`;
}
