import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { VEGETATION_MODEL_VERSION } from '../src/core/vegetation';
import { NativeSession, decodeClimateNormals, decodeVegetation, decodeVegetationSeries, decodeWorld } from '../src/native/client';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...NEW_WORLD_RECIPE, subdivision: 4 };

test('vegetation starts near the climate classes, advances reproducibly and replays regions', async () => {
  const session = new NativeSession(executable);
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    const n = world.stats.regionCount;
    await assert.rejects(session.request({ command: 'advanceVegetation', years: 1 }), /Initialize vegetation/);
    const climate = decodeClimateNormals(await session.request({ command: 'climateNormals' }), n, 1);
    const initial = decodeVegetation(await session.request({ command: 'initializeVegetation' }), n, 1);
    assert.equal(initial.modelVersion, VEGETATION_MODEL_VERSION);
    assert.equal(initial.year, 0);
    // The spin-up moves only marginal regions; temperature-only classes never move.
    const agree = [...initial.label].filter((label, i) => label === climate.biome[i]).length;
    assert.ok(agree > 0.85 * n, `year-0 labels agree with climate classes in ${agree} of ${n} regions`);
    for (let i = 0; i < n; i++) if (climate.biome[i] <= 2) assert.equal(initial.label[i], climate.biome[i]);
    assert.ok(initial.productivity.some((p) => p > 1000) && initial.woodyChange.some((c) => c !== 0));
    const first = decodeVegetation(await session.request({ command: 'advanceVegetation', years: 25 }), n, 1);
    assert.equal(first.year, 25);
    assert.equal(first.summaries.length, 25);
    assert.ok(first.precipitationFactor.some((f) => Math.abs(f - 1) > 0.05));
    const total = first.summaries[24].productivityPetagrams;
    assert.ok(total > 10 && total < 1000, `land productivity ${total} Pg`);
    const region = [...first.label].findIndex((label, i) => label !== 0 && first.woody[i] > 0.3);
    const series = decodeVegetationSeries(await session.request({ command: 'vegetationSeries', region }), region, 25, 1);
    const last = series.years[25];
    assert.equal(series.years.length, 26);
    assert.deepEqual([last.woody, last.herb, last.label, last.productivity],
      [first.woody[region], first.herb[region], first.label[region], first.productivity[region]]);
    // The same recipe reproduces the same years; regenerating discards the run.
    decodeWorld(await session.request({ command: 'generate', recipe }));
    await assert.rejects(session.request({ command: 'vegetationSeries', region }), /Initialize vegetation/);
    decodeVegetation(await session.request({ command: 'initializeVegetation' }), n, 1);
    const again = decodeVegetation(await session.request({ command: 'advanceVegetation', years: 25 }), n, 1);
    assert.deepEqual(again, first);
    await assert.rejects(session.request({ command: 'advanceVegetation', years: 1001 }), /1–1000 years/);
    const packet = await session.request({ command: 'advanceVegetation', years: 1 });
    assert.throws(() => decodeVegetation(packet, n + 1, 1), /vegetation metadata/);
    assert.throws(() => decodeVegetation({ ...packet, bytes: packet.bytes.subarray(8) }, n, 1), /vegetation metadata/);
  } finally { session.close(); }
});
