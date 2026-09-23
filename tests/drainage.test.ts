import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE, parseRecipe } from '../src/core/recipe';
import { NativeController, NativeSession, decodeWorld } from '../src/native/client';
import { summarizeDrainage, validateDrainage } from '../src/core/drainage';
import { buildViewGeometry } from '../src/renderer/view-geometry';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');

test('drainage summaries account for all dry land and display preparation cannot alter routes', async () => {
  const core = new NativeController(executable);
  try {
    const { world } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 3 });
    const d = world.drainage, original = structuredClone(d), summary = summarizeDrainage(world.surface, world.water, d);
    const land = world.surface.areasSquareMeters.reduce((sum, a, i) => sum + (world.water.bodyIds[i] ? 0 : a), 0);
    const terminal = d.contributingArea.reduce((sum, a, i) => sum + (d.receivers[i] === i ? a : 0), 0);
    assert.ok(Math.abs(terminal / land - 1) < 1e-12);
    assert.ok(Math.abs(summary.catchments.reduce((sum, c) => sum + c.landAreaSquareMeters, 0) / land - 1) < 1e-12);
    assert.ok(summary.closedDrainageLandFraction >= 0 && summary.closedDrainageLandFraction <= 1);
    buildViewGeometry(world.surface, world.tectonics, world.terrain.elevation, world.water);
    assert.deepEqual(d, original);
    assert.deepEqual((await core.generate(world.recipe)).world.drainage, d);
  } finally { core.close(); }
});

test('uniform dry worlds terminate at one closed-flat sink; full water has no contributing land', async () => {
  const core = new NativeController(executable);
  try {
    for (const fraction of [0, 1]) {
      const { world } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 2, continentalFraction: 0,
        reliefScale: 0, detailAmplitudeMeters: 0, water: { mode: 'coverage', fraction } });
      const summary = summarizeDrainage(world.surface, world.water, world.drainage);
      assert.equal(summary.catchmentCount, 1); assert.equal(summary.closedSinkCount, 1 - fraction);
      assert.equal(summary.closedDrainageLandFraction, 1 - fraction);
      assert.ok(world.drainage.outlets.every((id) => id === 0));
      if (fraction) assert.ok(world.drainage.contributingArea.every((a) => a === 0));
      else assert.ok(Math.abs(world.drainage.contributingArea[0] / world.stats.totalAreaSquareMeters - 1) < 1e-12);
    }
  } finally { core.close(); }
});

test('protocol rejects drainage corruption and does not silently migrate water recipes', async () => {
  assert.throws(() => parseRecipe({ ...DEFAULT_RECIPE, modelVersion: 'water-1' }), /Unsupported/);
  const session = new NativeSession(executable);
  try {
    await assert.rejects(session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, modelVersion: 'water-1' } }), /Unsupported/);
    const packet = await session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, subdivision: 2 } });
    const world = decodeWorld(packet), n = world.stats.regionCount, start = packet.bytes.length - n * 20;
    for (const offset of [0, n * 4, n * 8]) {
      const bytes = Buffer.from(packet.bytes); bytes.writeUInt32LE(n + 1, start + offset);
      assert.throws(() => decodeWorld({ ...packet, bytes }), /drainage/);
    }
    for (const value of [NaN, -1, 1e30]) {
      const bytes = Buffer.from(packet.bytes); bytes.writeDoubleLE(value, start + n * 12);
      assert.throws(() => decodeWorld({ ...packet, bytes }), /drainage|finite/);
    }
    const routed = world.drainage.receivers.findIndex((r, i) => r !== i);
    assert.ok(routed >= 0);
    const invalidOutlet = structuredClone(world.drainage);
    invalidOutlet.outlets[routed] = (invalidOutlet.outlets[routed] + 1) % n;
    assert.throws(() => validateDrainage(world.surface, world.terrain.elevation, world.water, invalidOutlet), /drainage/);
    const wet = world.water.bodyIds.findIndex((id) => id > 0);
    const invalidWet = structuredClone(world.drainage); invalidWet.receivers[wet] = world.surface.neighbors[world.surface.neighborOffsets[wet]];
    assert.throws(() => validateDrainage(world.surface, world.terrain.elevation, world.water, invalidWet), /drainage/);
  } finally { session.close(); }
});
