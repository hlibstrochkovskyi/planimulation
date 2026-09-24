import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE, parseRecipe } from '../src/core/recipe';
import { NativeController, NativeSession, decodeWorld } from '../src/native/client';
import { basinTree, summarizeBasins, validateBasins } from '../src/core/basins';
import type { Basins } from '../src/core/basins';
import { buildSurface } from '../src/core/surface';
import { buildViewGeometry } from '../src/renderer/view-geometry';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');

test('weighted nested basin transport has independently known areas, capacities, and ancestry', () => {
  const s = { ...buildSurface(0, 100000), areasSquareMeters: Float64Array.of(2, 3, 5, 7, 11),
    neighborOffsets: Uint32Array.of(0, 1, 3, 5, 7, 8), neighbors: Uint32Array.of(1, 0, 2, 1, 3, 2, 4, 3) };
  const h = Float64Array.of(0, 2, 1, 5, -1);
  const b: Basins = { regionNodes: Uint32Array.of(1, 3, 2, 4, 0), parents: Uint32Array.of(4, 3, 3, 4, 4),
    birthLevels: Float64Array.of(-1, 0, 1, 2, 5), spillLevels: Float64Array.of(5, 2, 2, 5, 5),
    spillFrom: Uint32Array.of(3, 1, 1, 3, 5), spillTo: Uint32Array.of(4, 0, 2, 2, 5),
    supportAreas: Float64Array.of(11, 2, 5, 10, 28), capacities: Float64Array.of(66, 4, 5, 39, 0) };
  validateBasins(s, h, b);
  assert.deepEqual(basinTree(b).children, [[], [], [], [1, 2], [0, 3]]);
  assert.deepEqual(summarizeBasins(b), { analysisVersion: 'basin-analysis-1', nodeCount: 5, leafCount: 3, root: 4,
    minimumSpillMeters: 2, maximumSpillMeters: 5 });
  // Every field includes in-range semantic corruptions, not only out-of-bounds values.
  for (const [field, index, value] of [
    ['regionNodes', 0, 2], ['parents', 1, 4], ['parents', 1, 1], ['parents', 3, 2],
    ['birthLevels', 3, 2.1], ['spillLevels', 1, 3], ['spillFrom', 3, 1], ['spillTo', 3, 4],
    ['supportAreas', 3, 11], ['capacities', 3, 40], ['capacities', 4, 100], ['spillFrom', 4, 0],
  ] as const) {
    const bad = structuredClone(b); bad[field][index] = value;
    assert.throws(() => validateBasins(s, h, bad), /basin/, `${field}[${index}]`);
  }
});

test('native basins reproduce and remain independent of water, diagnostics, and view preparation', async () => {
  const core = new NativeController(executable);
  try {
    for (const subdivision of [0, 3, 5]) {
      const { world } = await core.generate({ ...DEFAULT_RECIPE, subdivision });
      const original = structuredClone(world.basins);
      buildViewGeometry(world.surface, world.tectonics, world.terrain.elevation, world.water);
      assert.deepEqual(world.basins, original);
      const other = (await core.generate(world.recipe)).world;
      assert.deepEqual(other.basins, original); assert.equal(other.checksum, world.checksum);
      const dry = (await core.generate({ ...world.recipe, water: { mode: 'coverage', fraction: 0 } })).world;
      assert.deepEqual(dry.basins, original);
      assert.deepEqual(dry.terrain, world.terrain);
      const summary = summarizeBasins(original);
      assert.ok(Math.abs(original.supportAreas[summary.root] / world.stats.totalAreaSquareMeters - 1) < 1e-10);
    }
    for (const fraction of [0, 1]) {
      const { world } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 2, continentalFraction: 0,
        reliefScale: 0, detailAmplitudeMeters: 0, water: { mode: 'coverage', fraction } });
      const b = world.basins, summary = summarizeBasins(b);
      assert.equal(summary.nodeCount, 1); assert.equal(summary.leafCount, 1);
      assert.equal(summary.minimumSpillMeters, null); assert.equal(summary.maximumSpillMeters, null);
      assert.ok(b.regionNodes.every((id) => id === 0)); assert.equal(b.spillFrom[0], world.stats.regionCount);
    }
  } finally { core.close(); }
});

test('basin decoding rejects malformed metadata, each binary field, and legacy versions', async () => {
  assert.throws(() => parseRecipe({ ...DEFAULT_RECIPE, modelVersion: 'drainage-1' }), /Unsupported/);
  const session = new NativeSession(executable);
  try {
    await assert.rejects(session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, modelVersion: 'drainage-1' } }), /Unsupported/);
    const packet = await session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, subdivision: 3 } });
    const world = decodeWorld(packet), n = world.stats.regionCount, k = world.basins.parents.length;
    assert.ok(k > 1);
    for (const bad of [0, -1, 2 * n, 1.5, undefined]) assert.throws(() => decodeWorld({ ...packet, header: { ...packet.header, basinNodeCount: bad } }), /basin/);
    assert.throws(() => decodeWorld({ ...packet, header: { ...packet.header, basinAnalysisVersion: 'unknown' } }), /basin/);
    const start = packet.bytes.length - n * 4 - k * 44;
    for (const [offset, value] of [[0, k], [n * 4, k], [n * 4 + k * 20, n + 1], [n * 4 + k * 24, n + 1]]) {
      const bytes = Buffer.from(packet.bytes); bytes.writeUInt32LE(value, start + offset);
      assert.throws(() => decodeWorld({ ...packet, bytes }), /basin/);
    }
    for (const offset of [n * 4 + k * 4, n * 4 + k * 12, n * 4 + k * 28, n * 4 + k * 36]) {
      for (const value of [NaN, 1e100]) {
        const bytes = Buffer.from(packet.bytes); bytes.writeDoubleLE(value, start + offset);
        assert.throws(() => decodeWorld({ ...packet, bytes }), /basin|finite/);
      }
    }
    const before = structuredClone(world.basins);
    await session.request({ command: 'advance', steps: 3 });
    const regenerated = decodeWorld(await session.request({ command: 'generate', recipe: world.recipe }));
    assert.deepEqual(regenerated.basins, before);
  } finally { session.close(); }
});
