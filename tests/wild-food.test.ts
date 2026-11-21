import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { WILD_FOOD_MODEL_VERSION } from '../src/core/wild-food';
import { NativeSession, decodeForage, decodeVegetation, decodeWildFood, decodeWildFoodSeries, decodeWorld } from '../src/native/client';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...NEW_WORLD_RECIPE, subdivision: 4 };

test('wild food advances with vegetation, replays regions and depletes game under foraging', async () => {
  const session = new NativeSession(executable);
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    const n = world.stats.regionCount;
    await assert.rejects(session.request({ command: 'wildFood' }), /Initialize vegetation/);
    decodeVegetation(await session.request({ command: 'initializeVegetation' }), n, 1);
    const initial = decodeWildFood(await session.request({ command: 'wildFood' }), n, 0, 1);
    assert.equal(initial.modelVersion, WILD_FOOD_MODEL_VERSION);
    assert.equal(initial.summaries.length, 1);
    for (let i = 0; i < n; i++) {
      if (world.water.bodyIds[i] !== 0) assert.equal(initial.capacity[i], 0);
      // The game stock starts at its capacity.
      else assert.ok(Math.abs(initial.game[i] - initial.gameCapacity[i]) <= 1e-12 * initial.gameCapacity[i]);
    }
    decodeVegetation(await session.request({ command: 'advanceVegetation', years: 30 }), n, 1);
    const frame = decodeWildFood(await session.request({ command: 'wildFood' }), n, 30, 1);
    assert.equal(frame.summaries.length, 31);
    const people = frame.summaries[30].capacityPeople;
    assert.ok(people > 1e5 && people < 1e9, `forager capacity ${people}`);
    assert.ok(frame.summaries.some((s, y) => y > 0 && s.gameRatio < 0.98), 'dry years leave game below capacity');
    const region = [...frame.capacity].reduce((best, c, i) => c * world.surface.areasSquareMeters[i] > frame.capacity[best] * world.surface.areasSquareMeters[best] ? i : best, 0);
    const series = decodeWildFoodSeries(await session.request({ command: 'wildFoodSeries', region }), region, 30, 1);
    const last = series.years[30];
    assert.deepEqual([last.capacity, last.game, last.plantKcal, last.leanMonth],
      [frame.capacity[region], frame.game[region], frame.plantKcal[region], frame.leanMonth[region]]);
    assert.ok(Math.abs(last.season.reduce((a, b) => a + b, 0) - 1) < 1e-9);
    const sustainable = Math.round(frame.capacity[region] * world.surface.areasSquareMeters[region] / 1e6);
    const light = decodeForage(await session.request({ command: 'forage', region, people: Math.max(1, Math.round(sustainable / 2)), years: 20 }),
      region, Math.max(1, Math.round(sustainable / 2)), 20, 30, 1);
    const heavy = decodeForage(await session.request({ command: 'forage', region, people: sustainable * 5, years: 20 }), region, sustainable * 5, 20, 30, 1);
    assert.equal(heavy.game.length, 31);
    assert.ok(heavy.game[20] < 0.5 * light.game[20], 'heavy foraging depletes game');
    assert.ok(heavy.shortfall[20] > 0 && heavy.shortfall.slice(21).every(s => s === 0));
    assert.ok(heavy.game[30] > heavy.game[20], 'game recovers after the group leaves');
    await assert.rejects(session.request({ command: 'forage', region, people: 0, years: 5 }), /1 to 10,000,000/);
    const water = world.water.bodyIds.findIndex(b => b !== 0);
    await assert.rejects(session.request({ command: 'forage', region: water, people: 10, years: 5 }), /land region/);
    // Regenerating discards the run, and the same recipe repeats it exactly.
    decodeWorld(await session.request({ command: 'generate', recipe }));
    await assert.rejects(session.request({ command: 'wildFoodSeries', region }), /Initialize vegetation/);
    decodeVegetation(await session.request({ command: 'initializeVegetation' }), n, 1);
    decodeVegetation(await session.request({ command: 'advanceVegetation', years: 30 }), n, 1);
    assert.deepEqual(decodeWildFood(await session.request({ command: 'wildFood' }), n, 30, 1), frame);
    const packet = await session.request({ command: 'wildFood' });
    assert.throws(() => decodeWildFood(packet, n, 29, 1), /wild-food metadata/);
  } finally { session.close(); }
});
