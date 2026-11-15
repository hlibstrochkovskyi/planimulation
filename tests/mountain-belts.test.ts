import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { BELTS_MAX_FILL_METERS, DEFAULT_RECIPE, MULTISCALE_BELTS_MODEL_VERSION, MULTISCALE_UPLANDS_MODEL_VERSION, parseRecipe, serializeRecipe } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { NativeSession, decodeWorld } from '../src/native/client';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...DEFAULT_RECIPE, modelVersion: MULTISCALE_BELTS_MODEL_VERSION, subdivision: 4 };

function closedDryShare(world: ReturnType<typeof decodeWorld>): number {
  let land = 0, closed = 0;
  for (let i = 0; i < world.stats.regionCount; i++) {
    if (world.water.bodyIds[i]) continue;
    land += world.surface.areasSquareMeters[i];
    if (!world.water.bodyIds[world.drainage.outlets[i]]) closed += world.surface.areasSquareMeters[i];
  }
  return closed / land;
}

test('mountain-belt worlds reuse version-2 structure, stay bounded and drain more connectedly', async () => {
  assert.deepEqual(parseRecipe(JSON.parse(serializeRecipe(recipe))), recipe);
  const session = new NativeSession(executable);
  try {
    const belts = decodeWorld(await session.request({ command: 'generate', recipe }));
    assert.deepEqual(decodeWorld(await session.request({ command: 'generate', recipe })), belts);
    const uplands = decodeWorld(await session.request({ command: 'generate', recipe: { ...recipe, modelVersion: MULTISCALE_UPLANDS_MODEL_VERSION } }));
    assert.deepEqual(belts.tectonics, uplands.tectonics);
    assert.deepEqual(belts.crust, uplands.crust);
    assert.deepEqual(belts.terrain.baseline, uplands.terrain.baseline);
    assert.deepEqual(belts.terrain.detail, uplands.terrain.detail);
    assert.notDeepEqual(belts.terrain.convergence, uplands.terrain.convergence);
    assert.ok(belts.terrain.convergence.every(v => v >= 0 && v <= 6000));
    assert.ok(belts.terrain.preparation.every(v => Math.abs(v) <= 4 * 100 + BELTS_MAX_FILL_METERS));
    assert.ok(closedDryShare(belts) < closedDryShare(uplands), 'Filling shallow depressions connects more land to water.');
  } finally { session.close(); }
});
