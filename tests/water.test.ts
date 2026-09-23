import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE, parseRecipe, serializeRecipe } from '../src/core/recipe';
import { summarizeWater, validateWater } from '../src/core/water';
import { NativeController, NativeSession, decodeWorld } from '../src/native/client';
import { buildViewGeometry } from '../src/renderer/view-geometry';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');

test('water recipe modes are mutually exclusive, bounded, required, and round-trip exactly', () => {
  for (const water of [{ mode: 'coverage', fraction: 0 }, { mode: 'coverage', fraction: 1 },
    { mode: 'volume', volumeCubicMeters: 0 }, { mode: 'volume', volumeCubicMeters: 1e18 }]) {
    const recipe = parseRecipe({ ...DEFAULT_RECIPE, water });
    assert.deepEqual(JSON.parse(serializeRecipe(recipe)), recipe);
    assert.notEqual(recipe.water, water);
  }
  for (const water of [null, [], {}, { mode: 'coverage', fraction: -0.01 }, { mode: 'coverage', fraction: 1.01 },
    { mode: 'coverage', fraction: NaN }, { mode: 'coverage', fraction: '0.71' },
    { mode: 'coverage', fraction: 0.71, volumeCubicMeters: 1e18 },
    { mode: 'volume', volumeCubicMeters: -1 }, { mode: 'volume', volumeCubicMeters: Infinity },
    { mode: 'volume', volumeCubicMeters: 1e30 }, { mode: 'volume', fraction: 0.71 },
    { mode: 'volume', volumeCubicMeters: 1, extra: 2 }, { mode: 'seaLevel', level: 0 }]) {
    assert.throws(() => parseRecipe({ ...DEFAULT_RECIPE, water }));
  }
  const missing: Record<string, unknown> = { ...DEFAULT_RECIPE }; delete missing.water;
  assert.throws(() => parseRecipe(missing), /Missing/);
  assert.throws(() => parseRecipe({ ...DEFAULT_RECIPE, modelVersion: 'terrain-1' }), /Unsupported/);
});

test('water summaries partition physical area and volume; fixed-volume regeneration preserves upstream fields', async () => {
  const core = new NativeController(executable);
  try {
    const { world } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 3 });
    const original = structuredClone(world.water);
    const summary = summarizeWater(world.surface, world.water);
    assert.ok(Math.abs(summary.waterAreaFraction - 0.71) < world.stats.maximumAreaSquareMeters / world.stats.totalAreaSquareMeters);
    assert.ok(Math.abs(summary.mainOceanAreaFraction + summary.inlandWaterAreaFraction - summary.waterAreaFraction) < 1e-12);
    assert.ok(Math.abs(summary.bodies.reduce((sum, b) => sum + b.volumeCubicMeters, 0) / summary.resolvedVolumeCubicMeters - 1) < 1e-12);
    const volumeWorld = (await core.generate({ ...world.recipe, water: { mode: 'volume', volumeCubicMeters: summary.resolvedVolumeCubicMeters } })).world;
    assert.deepEqual(volumeWorld.water.bodyIds, world.water.bodyIds);
    assert.ok(Math.abs(volumeWorld.water.levelMeters - world.water.levelMeters) < 1e-8);
    assert.deepEqual(volumeWorld.terrain, world.terrain);
    assert.deepEqual(volumeWorld.crust, world.crust);
    assert.deepEqual(volumeWorld.tectonics, world.tectonics);
    buildViewGeometry(world.surface, world.tectonics, world.terrain.elevation, world.water);
    assert.deepEqual(world.water, original, 'Bed and water display geometry cannot alter initial water.');
    for (const fraction of [0, 1]) {
      const changed = (await core.generate({ ...world.recipe, water: { mode: 'coverage', fraction } })).world;
      const s = summarizeWater(changed.surface, changed.water);
      assert.equal(s.waterAreaFraction, fraction); assert.equal(s.bodyCount, fraction);
      assert.equal(s.mainOceanAreaFraction, fraction); assert.equal(s.inlandWaterAreaFraction, 0);
    }
  } finally { core.close(); }
});

test('native water decoding rejects inconsistent levels, stocks, masks, and component identities', async () => {
  const session = new NativeSession(executable);
  try {
    for (const water of [{ mode: 'coverage', fraction: 0.71, volumeCubicMeters: 1 }, { mode: 'volume', volumeCubicMeters: -1 }]) {
      await assert.rejects(session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, water } }));
    }
    const packet = await session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, subdivision: 2 } });
    const world = decodeWorld(packet), n = world.stats.regionCount, start = packet.bytes.length - 20 - n * 12;
    for (const [offset, value] of [[0, NaN], [0, world.water.levelMeters + 1], [8, -1], [8, world.water.resolvedVolumeCubicMeters * 2], [16, -1]]) {
      const bytes = Buffer.from(packet.bytes); bytes.writeDoubleLE(value, start + offset);
      assert.throws(() => decodeWorld({ ...packet, bytes }), /water|finite/);
    }
    const wet = world.water.bodyIds.findIndex((id) => id > 0);
    const dry = world.water.bodyIds.findIndex((id) => id === 0);
    for (const [index, value] of [[wet, 0], [wet, n + 1], [dry, 1], [n, n + 1]]) {
      const bytes = Buffer.from(packet.bytes); bytes.writeUInt32LE(value, start + 16 + 8 * n + index * 4);
      assert.throws(() => decodeWorld({ ...packet, bytes }), /water/);
    }
    // Reusing a body ID across disconnected regions must also be rejected.
    const s = { ...world.surface, areasSquareMeters: new Float64Array([1, 1, 1]),
      neighborOffsets: new Uint32Array([0, 1, 3, 4]), neighbors: new Uint32Array([1, 0, 2, 1]) };
    const t = { ...world.terrain, elevation: new Float64Array([0, 10, 0]) };
    const w = { levelMeters: 1, resolvedVolumeCubicMeters: 2, depthMeters: new Float64Array([1, 0, 1]),
      bodyIds: new Uint32Array([1, 0, 2]), mainOceanId: 1 };
    validateWater(s, t, w, { mode: 'volume', volumeCubicMeters: 2 });
    assert.throws(() => validateWater(s, t, { ...w, bodyIds: new Uint32Array([1, 0, 1]) }, { mode: 'volume', volumeCubicMeters: 2 }), /water/);
    assert.throws(() => validateWater(s, t, { ...w, mainOceanId: 2 }, { mode: 'volume', volumeCubicMeters: 2 }), /water/);
  } finally { session.close(); }
});
