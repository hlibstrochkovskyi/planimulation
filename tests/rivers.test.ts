import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import { RIVER_MODEL_VERSION } from '../src/core/rivers';
import { NativeSession, decodeClimateNormals, decodeRivers, decodeWorld } from '../src/native/client';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...NEW_WORLD_RECIPE, subdivision: 4 };

function near(a: number, b: number): void {
  assert.ok(Math.abs(a - b) <= 1e-8 + Math.max(Math.abs(a), Math.abs(b)) * 1e-12, `${a} != ${b}`);
}

test('river protocol carries conserved runoff, spill records and reproducible discharge', async () => {
  const session = new NativeSession(executable);
  try {
    await assert.rejects(session.request({ command: 'rivers' }), /Generate a world first/);
    const initial = await session.request({ command: 'generate', recipe });
    const world = decodeWorld(initial), n = world.stats.regionCount;
    const packet = await session.request({ command: 'rivers' });
    const frame = decodeRivers(packet, n, 1);
    assert.equal(packet.header.protocol, 9);
    assert.equal(frame.modelVersion, RIVER_MODEL_VERSION);
    assert.equal(frame.monthDays.reduce((a, b) => a + b, 0), 365);
    assert.equal(frame.oceanTerminal, world.water.bodyIds.indexOf(world.water.mainOceanId));
    assert.ok(frame.spillSegments.length > 0 && frame.lakes.size > 0);
    assert.ok(frame.class.some(value => value === 2) && frame.class.some(value => value === 1));
    const climate = decodeClimateNormals(await session.request({ command: 'climateNormals' }), n, 1);
    let oceanVolume = 0;
    for (let i = 0; i < n; i++) {
      if (world.water.bodyIds[i] === 0) near(climate.annualPrecipitationMm[i], frame.aet[i] + frame.runoff[i] + frame.storage[i]);
      if (world.water.bodyIds[i] === world.water.mainOceanId) oceanVolume += frame.meanDischarge[i] * 365 * 86_400;
      assert.ok(frame.meanDischarge[i] <= frame.meanDischarge[world.drainage.receivers[i]]);
    }
    near(oceanVolume, frame.totals.oceanDischarge);
    for (const { from, to } of frame.spillSegments) {
      assert.ok(world.surface.neighbors.subarray(world.surface.neighborOffsets[from], world.surface.neighborOffsets[from + 1]).includes(to));
    }
    await session.request({ command: 'advance', steps: 1 });
    assert.deepEqual(await session.request({ command: 'rivers' }), packet);
    assert.deepEqual(await session.request({ command: 'generate', recipe }), initial);
    assert.deepEqual(await session.request({ command: 'rivers' }), packet);

    await session.request({ command: 'generate', recipe: { ...recipe, seed: 'river-protocol-regeneration' } });
    const next = decodeRivers(await session.request({ command: 'rivers' }), n, 2);
    assert.notDeepEqual(next.runoff, frame.runoff);
    assert.throws(() => decodeRivers({ ...packet, header: { ...packet.header, monthDays: Array(12).fill(30) } }, n, 1), /river metadata/);
    assert.throws(() => decodeRivers({ ...packet, bytes: packet.bytes.subarray(8) }, n, 1), /river metadata/);
    const bytes = Buffer.from(packet.bytes);
    bytes.writeUInt32LE(n, n * 236);
    assert.throws(() => decodeRivers({ ...packet, bytes }, n, 1), /river region/);
  } finally { session.close(); }
});

test('fully wet and oceanless worlds have explicit drainage outcomes', async () => {
  const session = new NativeSession(executable);
  try {
    for (const fraction of [0, 1]) {
      const world = decodeWorld(await session.request({ command: 'generate', recipe: { ...recipe, subdivision: 2, water: { mode: 'coverage', fraction } } }));
      const frame = decodeRivers(await session.request({ command: 'rivers' }), world.stats.regionCount, 1);
      assert.equal(frame.totals.oceanDischarge, 0);
      if (fraction === 0) {
        assert.equal(frame.oceanTerminal, null);
        near(frame.totals.endorheicArea, frame.totals.landArea);
      } else {
        assert.equal(frame.totals.landRunoff, 0);
        assert.equal(frame.lakes.size, 0);
      }
    }
  } finally { session.close(); }
});
