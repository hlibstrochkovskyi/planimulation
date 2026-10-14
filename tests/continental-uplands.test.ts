import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE, MULTISCALE_UPLANDS_MODEL_VERSION, MULTISCALE_GEOGRAPHY_MODEL_VERSION, parseRecipe, serializeRecipe } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { continentalUplandsThickness, columnBuoyancyBaseline } from '../src/core/continental-uplands';
import { NativeSession, decodeWorld } from '../src/native/client';
import { resolvedWorldJson } from '../src/native/resolved-export';
import { SoilMoistureSession } from '../src/native/soil-session';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...DEFAULT_RECIPE, modelVersion: MULTISCALE_UPLANDS_MODEL_VERSION, subdivision: 3 };

test('uplands scalar reference and strict recipes keep bounded physical columns and older defaults', () => {
  assert.equal(continentalUplandsThickness(1, .25, 0), 42500);
  assert.equal(continentalUplandsThickness(.5, .25, 0), 24750);
  assert.equal(continentalUplandsThickness(0, 1, 0), 7000);
  assert.ok(Math.abs(columnBuoyancyBaseline(42500, 2800) - 1303.030303030303) < 1e-9);
  assert.deepEqual(parseRecipe(JSON.parse(serializeRecipe(recipe))), recipe);
  assert.equal(DEFAULT_RECIPE.modelVersion, 'terrain-prep-1');
  assert.throws(() => continentalUplandsThickness(1.01, 0, 0));
  assert.throws(() => parseRecipe({ ...recipe, terrainPreparationPasses: 17 }));
  assert.throws(() => parseRecipe({ ...recipe, modelVersion: 'multiscale-geography-3' }));
});

test('uplands native fields repeat, preserve structure and export inspectable material provenance', async () => {
  const session = new NativeSession(executable);
  try {
    const oldPacket = await session.request({ command: 'generate', recipe: { ...recipe, modelVersion: MULTISCALE_GEOGRAPHY_MODEL_VERSION } });
    const old = decodeWorld(oldPacket);
    assert.equal(old.checksum, '0727ddc8');
    const packet = await session.request({ command: 'generate', recipe });
    const w = decodeWorld(packet);
    assert.equal(w.checksum, 'd4d48032');
    assert.deepEqual(decodeWorld(await session.request({ command: 'generate', recipe })), w);
    assert.deepEqual(w.tectonics, old.tectonics);
    assert.deepEqual(w.crust.potential, old.crust.potential);
    assert.deepEqual(w.crust.continentality, old.crust.continentality);
    assert.deepEqual(w.terrain.detail, old.terrain.detail);
    assert.ok(w.terrain.baseline.some(v => v > 167));
    for (let i = 0; i < w.stats.regionCount; i++) {
      assert.ok(Math.abs(w.crust.thicknessMeters[i] - continentalUplandsThickness(w.crust.continentality[i], w.crust.potential[i], w.crust.threshold)) < 1e-8);
    }
    const report = JSON.parse([...resolvedWorldJson(w)].join(''));
    assert.equal(report.recipe.modelVersion, MULTISCALE_UPLANDS_MODEL_VERSION);
    assert.deepEqual(report.crust.thicknessMeters, Array.from(w.crust.thicknessMeters));
    assert.equal(report.formatVersion, 2);
    assert.deepEqual(await session.request({ command: 'generate', recipe: old.recipe }), oldPacket);
  } finally { session.close(); }
});

test('decoder cannot relabel old/new material or accept bounded but wrong thickness', async () => {
  const session = new NativeSession(executable);
  try {
    const packet = await session.request({ command: 'generate', recipe });
    const oldPacket = await session.request({ command: 'generate', recipe: { ...recipe, modelVersion: MULTISCALE_GEOGRAPHY_MODEL_VERSION } });
    assert.throws(() => decodeWorld({ ...packet, header: { ...packet.header, recipe: { ...recipe, modelVersion: MULTISCALE_GEOGRAPHY_MODEL_VERSION } } }), /elevation|crust/);
    assert.throws(() => decodeWorld({ ...oldPacket, header: { ...oldPacket.header, recipe } }), /uplands/);
    const n = 10 * 4 ** recipe.subdivision + 2, neighbors = 6 * n - 12, faces = 60 * 4 ** recipe.subdivision;
    const boundaryCount = packet.header.boundarySegmentCount as number;
    const thicknessStart = n * 24 + faces * 4 + (n + 1) * 8 + neighbors * 12 + n * 16 + neighbors * 48
      + n * 4 + recipe.plateCount * 28 + boundaryCount * 76 + 8 + n * 16;
    const bytes = Buffer.from(packet.bytes);
    bytes.writeDoubleLE(bytes.readDoubleLE(thicknessStart) + 1, thicknessStart);
    assert.throws(() => decodeWorld({ header: packet.header, bytes }), /uplands/);
  } finally { session.close(); }
});

test('complete seasonal checkpoint restores uplands geometry and continues exactly', async () => {
  const direct = await SoilMoistureSession.initialize(executable, { ...recipe, subdivision: 2 });
  let restored: SoilMoistureSession | undefined;
  try {
    await direct.advance(3600);
    const saved = await direct.checkpoint();
    restored = await SoilMoistureSession.restore(executable, saved);
    assert.deepEqual(restored.world, direct.world);
    await direct.advance(3600); await restored.advance(3600);
    assert.equal(await direct.checkpoint(), await restored.checkpoint());
  } finally { direct.close(); restored?.close(); }
});
