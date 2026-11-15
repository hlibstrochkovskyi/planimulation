import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE, MULTISCALE_BELTS_MODEL_VERSION, MULTISCALE_LOWLANDS_MODEL_VERSION, parseRecipe, serializeRecipe } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { NativeSession, decodeWorld } from '../src/native/client';
import { resolvedWorldRecord } from '../src/native/resolved-export';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...DEFAULT_RECIPE, modelVersion: MULTISCALE_LOWLANDS_MODEL_VERSION, subdivision: 4 };

function medianLandHeight(world: ReturnType<typeof decodeWorld>): number {
  const land: number[] = [];
  for (let i = 0; i < world.stats.regionCount; i++) if (!world.water.bodyIds[i]) land.push(world.terrain.elevation[i] - world.water.levelMeters);
  return land.sort((a, b) => a - b)[Math.floor(land.length / 2)];
}

test('coastal-lowland worlds keep version-3 shorelines and belts and lower only dry land', async () => {
  assert.deepEqual(parseRecipe(JSON.parse(serializeRecipe(recipe))), recipe);
  const session = new NativeSession(executable);
  try {
    const low = decodeWorld(await session.request({ command: 'generate', recipe }));
    assert.deepEqual(decodeWorld(await session.request({ command: 'generate', recipe })), low);
    const belts = decodeWorld(await session.request({ command: 'generate', recipe: { ...recipe, modelVersion: MULTISCALE_BELTS_MODEL_VERSION } }));
    assert.equal(belts.terrain.lowland, undefined);
    assert.ok(low.terrain.lowland);
    assert.deepEqual(low.crust, belts.crust);
    assert.deepEqual(low.terrain.baseline, belts.terrain.baseline);
    assert.deepEqual(low.terrain.convergence, belts.terrain.convergence);
    assert.deepEqual(low.water.bodyIds, belts.water.bodyIds);
    for (let i = 0; i < low.stats.regionCount; i++) {
      assert.ok(low.terrain.lowland[i] <= 0);
      if (low.water.bodyIds[i]) assert.equal(low.terrain.lowland[i], 0);
    }
    assert.ok(medianLandHeight(low) < medianLandHeight(belts) - 100, 'Coastal lowlands lower the median land height.');
    assert.deepEqual((resolvedWorldRecord(low) as { terrain: { lowland: Float64Array } }).terrain.lowland, low.terrain.lowland);
  } finally { session.close(); }
});
