import { parseRecipe, serializeRecipe } from './recipe';
import type { Recipe } from './recipe';
import { stream } from './random';
import { buildSurface } from './surface';
import type { Surface } from './surface';
import type { Tectonics } from './tectonics';
import type { Crust } from './crust';
import type { Terrain } from './terrain';

export interface SurfaceWorld {
  recipe: Recipe;
  surface: Surface;
  diagnosticField: Float64Array;
  checksum: string;
  stats: {
    regionCount: number;
    faceCount: number;
    edgeCount: number;
    totalAreaSquareMeters: number;
    relativeAreaError: number;
    minimumAreaSquareMeters: number;
    maximumAreaSquareMeters: number;
    arrayBytes: number;
  };
}
export interface World extends SurfaceWorld { tectonics: Tectonics; crust: Crust; terrain: Terrain }

/** A coherent test signal, deliberately not labeled terrain, climate, or biome. */
function diagnosticField(surface: Surface, seed: string): Float64Array {
  const random = stream(seed, 'diagnostic-field');
  const modes = Array.from({ length: 8 }, () => ({
    x: (random.next() * 2 - 1) * 9,
    y: (random.next() * 2 - 1) * 9,
    z: (random.next() * 2 - 1) * 9,
    phase: random.next() * Math.PI * 2,
  }));
  const result = new Float64Array(surface.areasSquareMeters.length);
  for (let id = 0; id < result.length; id++) {
    let value = 0;
    for (const mode of modes) {
      value += Math.sin(mode.x * surface.centers[id * 3] + mode.y * surface.centers[id * 3 + 1]
        + mode.z * surface.centers[id * 3 + 2] + mode.phase);
    }
    result[id] = value / modes.length;
  }
  return result;
}

/** Stable little-endian encoding; checksum is a regression identifier, not cryptography. */
export function checksumWorld(recipe: Recipe, surface: Surface, field: Float64Array): string {
  let hash = 0x811c9dc5;
  const byte = (value: number): void => { hash = Math.imul(hash ^ value, 0x01000193) >>> 0; };
  for (const value of new TextEncoder().encode(serializeRecipe(recipe))) byte(value);
  const buffer = new ArrayBuffer(8), view = new DataView(buffer);
  const arrays = [surface.centers, surface.faces, surface.neighborOffsets, surface.neighbors,
    surface.neighborDistancesMeters, surface.areasSquareMeters, surface.boundaryOffsets, surface.boundaryDirections, field];
  for (const array of arrays) {
    view.setUint32(0, array.length, true);
    for (let i = 0; i < 4; i++) byte(view.getUint8(i));
    for (const value of array) {
      if (array instanceof Uint32Array) {
        view.setUint32(0, value, true);
        for (let i = 0; i < 4; i++) byte(view.getUint8(i));
      } else {
        view.setFloat64(0, value, true);
        for (let i = 0; i < 8; i++) byte(view.getUint8(i));
      }
    }
  }
  return hash.toString(16).padStart(8, '0');
}

/** Independent TypeScript regression reference; desktop and headless use the native core. */
export function generateWorld(input: unknown, progress: (message: string) => void = () => {}): SurfaceWorld {
  const recipe = parseRecipe(input);
  progress('Building the spherical surface…');
  const surface = buildSurface(recipe.subdivision, recipe.radiusMeters);
  progress('Evaluating the seeded diagnostic field…');
  const field = diagnosticField(surface, recipe.seed);
  let total = 0, minimum = Infinity, maximum = 0;
  for (const area of surface.areasSquareMeters) {
    total += area;
    minimum = Math.min(minimum, area);
    maximum = Math.max(maximum, area);
  }
  const expected = 4 * Math.PI * recipe.radiusMeters ** 2;
  const relativeAreaError = Math.abs(total - expected) / expected;
  if (relativeAreaError > 1e-10 || !Number.isFinite(total)) throw new Error('Surface area validation failed.');
  const arrayBytes = Object.values(surface).reduce((sum, value) => sum + (ArrayBuffer.isView(value) ? value.byteLength : 0), field.byteLength);
  progress('Checking the surface fingerprint…');
  return {
    recipe, surface, diagnosticField: field,
    checksum: checksumWorld(recipe, surface, field),
    stats: {
      regionCount: field.length, faceCount: surface.faces.length / 3, edgeCount: surface.neighbors.length / 2,
      totalAreaSquareMeters: total, relativeAreaError, minimumAreaSquareMeters: minimum,
      maximumAreaSquareMeters: maximum, arrayBytes,
    },
  };
}
