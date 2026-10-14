import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE, MULTISCALE_GEOGRAPHY_MODEL_VERSION, parseRecipe, serializeRecipe } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { NativeSession, decodeWorld } from '../src/native/client';
import { RESOLVED_WORLD_FORMAT, resolvedWorldJson } from '../src/native/resolved-export';
import { SoilMoistureSession } from '../src/native/soil-session';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const candidate: Recipe = { ...DEFAULT_RECIPE, modelVersion: MULTISCALE_GEOGRAPHY_MODEL_VERSION, subdivision: 3 };

test('multiscale geography recipes are explicit, strict, and preserve existing defaults', () => {
  assert.equal(DEFAULT_RECIPE.modelVersion, 'terrain-prep-1');
  assert.deepEqual(parseRecipe(candidate), candidate);
  assert.deepEqual(parseRecipe(JSON.parse(serializeRecipe(candidate))), candidate);
  for (const passes of [0, 4, 16]) {
    const recipe = { ...candidate, terrainPreparationPasses: passes };
    assert.deepEqual(parseRecipe(recipe), recipe);
  }
  for (const passes of [-1, 17, 1.5, undefined, NaN, '4']) {
    assert.throws(() => parseRecipe({ ...candidate, terrainPreparationPasses: passes }), /preparation passes/);
  }
  assert.throws(() => parseRecipe({ ...candidate, geographyVersion: 'implicit' }), /Unknown recipe field/);
  assert.throws(() => parseRecipe({ ...candidate, modelVersion: 'multiscale-geography-3' }), /Unsupported/);
  assert.throws(() => parseRecipe({ ...candidate, subdivision: 7 }), /Subdivision/);
});

test('native multiscale geography preserves contribution identity, repeatability, and explicit export', async () => {
  const session = new NativeSession(executable);
  try {
    const originalPacket = await session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, subdivision: 3 } });
    for (const seed of ['first-light', '4836187e']) {
      const recipe = { ...candidate, seed };
      const packet = await session.request({ command: 'generate', recipe });
      const first = decodeWorld(packet);
      if (seed === 'first-light') assert.equal(first.checksum, '0727ddc8', 'Versioned candidate regression pin.');
      assert.equal(packet.header.protocol, 10);
      assert.deepEqual(first.recipe, recipe);
      assert.deepEqual(decodeWorld(await session.request({ command: 'generate', recipe })), first);
      const { terrain: t } = first;
      for (let id = 0; id < first.stats.regionCount; id++) {
        assert.ok(Math.abs(t.elevation[id] - (t.baseline[id] + t.convergence[id] + t.divergence[id]
          + t.detail[id] + t.preparation[id])) <= 1e-8);
        assert.ok(Math.abs(t.detail[id]) <= recipe.detailAmplitudeMeters + 1e-9);
        assert.ok(Math.abs(t.preparation[id]) <= recipe.terrainPreparationPasses! * 100 + 1e-8);
      }
      const serialized = [...resolvedWorldJson(first)].join('');
      const report = JSON.parse(serialized);
      assert.equal(report.format, RESOLVED_WORLD_FORMAT);
      assert.equal(report.formatVersion, 2);
      assert.equal(report.purpose, 'initial-generated-fields-not-a-checkpoint');
      assert.deepEqual(report.recipe, first.recipe);
      assert.equal(report.initialFingerprint, first.checksum);
      assert.deepEqual(report.crust.potential, Array.from(first.crust.potential));
      assert.equal(report.crust.threshold, first.crust.threshold);
      assert.deepEqual(report.terrain.elevation, Array.from(t.elevation));
      assert.deepEqual(report.terrain.detail, Array.from(t.detail));
      assert.equal([...resolvedWorldJson(first)].join(''), serialized);
      if (seed === DEFAULT_RECIPE.seed) assert.notEqual(first.checksum, originalPacket.header.checksum);
    }
    const restoredOriginal = await session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, subdivision: 3 } });
    assert.deepEqual(restoredOriginal, originalPacket, 'Selecting the candidate cannot change the original family.');
  } finally { session.close(); }
});

test('candidate crust uses the same fixed reference threshold and shared-point fields across sampling resolutions', async () => {
  const session = new NativeSession(executable);
  try {
    const coarse = decodeWorld(await session.request({ command: 'generate', recipe: { ...candidate, subdivision: 2 } }));
    const fine = decodeWorld(await session.request({ command: 'generate', recipe: candidate }));
    assert.equal(coarse.crust.threshold, fine.crust.threshold);
    const locations = new Map<string, number>();
    for (let id = 0; id < fine.stats.regionCount; id++) {
      locations.set(JSON.stringify(Array.from(fine.surface.centers.subarray(id * 3, id * 3 + 3))), id);
    }
    for (let id = 0; id < coarse.stats.regionCount; id++) {
      const other = locations.get(JSON.stringify(Array.from(coarse.surface.centers.subarray(id * 3, id * 3 + 3))));
      assert.notEqual(other, undefined);
      assert.equal(coarse.crust.potential[id], fine.crust.potential[other!]);
      assert.equal(coarse.crust.continentality[id], fine.crust.continentality[other!]);
    }
    // The sampled tectonic boundary envelopes and final terrain are not claimed
    // to be mesh-independent merely because the continuous crust field is.
  } finally { session.close(); }
});

test('a complete seasonal checkpoint regenerates candidate geography and resumes exactly', async () => {
  const recipe = { ...candidate, subdivision: 2 };
  const direct = await SoilMoistureSession.initialize(executable, recipe);
  let restored: SoilMoistureSession | undefined;
  try {
    const initialGeography = direct.world;
    assert.equal(initialGeography.recipe.modelVersion, MULTISCALE_GEOGRAPHY_MODEL_VERSION);
    await direct.advance(3600);
    const saved = await direct.checkpoint();
    restored = await SoilMoistureSession.restore(executable, saved);
    assert.deepEqual(restored.world, initialGeography);
    assert.equal(await restored.checkpoint(), saved);
    await direct.advance(3600);
    await restored.advance(3600);
    assert.equal(await direct.checkpoint(), await restored.checkpoint());
    assert.deepEqual(direct.world, initialGeography);
  } finally { direct.close(); restored?.close(); }
});
