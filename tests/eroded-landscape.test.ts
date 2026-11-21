import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE, MULTISCALE_ERODED_MODEL_VERSION, MULTISCALE_LOWLANDS_MODEL_VERSION, parseRecipe, serializeRecipe } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { NativeSession, decodeWorld } from '../src/native/client';
import { resolvedWorldRecord } from '../src/native/resolved-export';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...DEFAULT_RECIPE, modelVersion: MULTISCALE_ERODED_MODEL_VERSION, subdivision: 4 };

test('version-5 worlds carry ocean-floor and erosion contributions and allow subdivision 7', async () => {
  assert.deepEqual(parseRecipe(JSON.parse(serializeRecipe(recipe))), recipe);
  assert.equal(parseRecipe({ ...recipe, subdivision: 7 }).subdivision, 7);
  assert.throws(() => parseRecipe({ ...recipe, subdivision: 8 }), /0 to 7/);
  assert.throws(() => parseRecipe({ ...recipe, modelVersion: MULTISCALE_LOWLANDS_MODEL_VERSION, subdivision: 7 }), /0 to 6/);
  const session = new NativeSession(executable);
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    assert.deepEqual(decodeWorld(await session.request({ command: 'generate', recipe })), world);
    const { seafloor, erosion, lowland } = world.terrain;
    assert.ok(seafloor && erosion && lowland);
    const exported = resolvedWorldRecord(world) as { terrain: { seafloor: Float64Array; erosion: Float64Array } };
    assert.deepEqual(exported.terrain.seafloor, seafloor);
    assert.deepEqual(exported.terrain.erosion, erosion);
    const v4 = decodeWorld(await session.request({ command: 'generate', recipe: { ...recipe, modelVersion: MULTISCALE_LOWLANDS_MODEL_VERSION } }));
    assert.equal(v4.terrain.seafloor, undefined);
  } finally { session.close(); }
});
