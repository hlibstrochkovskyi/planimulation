export const MODEL_VERSION = 'surface-rust-1';
export const RANDOM_VERSION = 'fnv1a-utf8-mulberry32-1';

export interface Recipe {
  schemaVersion: 1;
  modelVersion: typeof MODEL_VERSION;
  randomVersion: typeof RANDOM_VERSION;
  seed: string;
  subdivision: number;
  radiusMeters: number;
}

export const DEFAULT_RECIPE: Readonly<Recipe> = Object.freeze({
  schemaVersion: 1,
  modelVersion: MODEL_VERSION,
  randomVersion: RANDOM_VERSION,
  seed: 'first-light',
  subdivision: 5,
  radiusMeters: 6_371_000,
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
  for (const key of keys) {
    if (!(key in input)) throw new Error(`Missing recipe field: ${key}.`);
  }
  if (input.schemaVersion !== 1 || input.modelVersion !== MODEL_VERSION || input.randomVersion !== RANDOM_VERSION) {
    throw new Error('Unsupported recipe version. This build supports surface-rust-1 recipes only; legacy recipes are not silently migrated.');
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
  return {
    schemaVersion: 1,
    modelVersion: MODEL_VERSION,
    randomVersion: RANDOM_VERSION,
    seed: input.seed,
    subdivision: Number(input.subdivision),
    radiusMeters: input.radiusMeters,
  };
}

export function serializeRecipe(recipe: Recipe): string {
  return `${JSON.stringify(parseRecipe(recipe), null, 2)}\n`;
}
