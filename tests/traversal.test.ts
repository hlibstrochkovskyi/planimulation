import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import {
  DEFAULT_TRAVERSAL_SETTINGS,
  TRAVERSAL_MODEL_VERSION,
  isRegionPassable,
  traceTravelRoute,
} from '../src/core/traversal';
import {
  NativeSession,
  decodeClimateNormals,
  decodeTraversalCosts,
  decodeTraversalField,
  decodeVegetation,
  decodeWorld,
} from '../src/native/client';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...NEW_WORLD_RECIPE, subdivision: 4 };

test('traversal protocol computes biome fallback and vegetation-coupled costs, seasonal Dijkstra and routes', async () => {
  const session = new NativeSession(executable);
  try {
    await assert.rejects(session.request({ command: 'traversalCosts' }), /Generate a world first/);
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    const n = world.stats.regionCount;
    const climate = decodeClimateNormals(await session.request({ command: 'climateNormals' }), n, 1);

    // Before vegetation is initialized, traversal uses climate-normals biomes.
    const fallback = decodeTraversalCosts(await session.request({ command: 'traversalCosts' }), n, 1);
    assert.equal(fallback.modelVersion, TRAVERSAL_MODEL_VERSION);
    assert.equal(fallback.usesVegetation, false);
    assert.equal(fallback.vegetationYear, null);
    assert.deepEqual(fallback.settings, DEFAULT_TRAVERSAL_SETTINGS);

    // After vegetation starts, traversal automatically rebuilds using live woody cover.
    const veg = decodeVegetation(await session.request({ command: 'initializeVegetation' }), n, 1);
    const coupled = decodeTraversalCosts(await session.request({ command: 'traversalCosts' }), n, 1);
    assert.equal(coupled.usesVegetation, true);
    assert.equal(coupled.vegetationYear, veg.year);

    // Passable land regions have positive finite cost; main ocean is impassable in both seasons.
    let landSample = -1;
    let oceanSample = -1;
    for (let i = 0; i < n; i++) {
      const wet = world.water.bodyIds[i] !== 0;
      const ocean = world.water.bodyIds[i] === world.water.mainOceanId;
      if (!wet) {
        assert.equal(isRegionPassable(coupled, i, 'summer'), true);
        assert.equal(isRegionPassable(coupled, i, 'winter'), true);
        assert.ok(coupled.hoursPer100KmSummer[i] > 15 && coupled.hoursPer100KmSummer[i] < 500);
        if (landSample < 0 && climate.biome[i] >= 3) landSample = i;
      } else {
        assert.equal(isRegionPassable(coupled, i, 'summer'), false);
        assert.equal(coupled.hoursPer100KmSummer[i], -1);
        if (ocean) {
          assert.equal(isRegionPassable(coupled, i, 'winter'), false);
          assert.equal(coupled.hoursPer100KmWinter[i], -1);
          if (oceanSample < 0) oceanSample = i;
        }
      }
    }
    assert.ok(landSample >= 0 && oceanSample >= 0);

    // Deterministic summer & winter least-time Dijkstra and route tracing.
    const summer = decodeTraversalField(
      await session.request({ command: 'travelTime', season: 'summer', sources: [landSample] }), n, 1);
    const summerAgain = decodeTraversalField(
      await session.request({ command: 'travelTime', season: 'summer', sources: [landSample] }), n, 1);
    assert.deepEqual(summerAgain, summer);
    assert.equal(summer.days[landSample], 0);
    assert.equal(summer.distanceMeters[landSample], 0);
    assert.equal(summer.predecessor[landSample], landSample);
    assert.equal(summer.days[oceanSample], -1);
    assert.equal(summer.predecessor[oceanSample], n);

    const target = [...summer.days].findIndex((d, i) => i !== landSample && d > 10);
    assert.ok(target >= 0, 'should find a reachable target more than 10 days away');
    const route = traceTravelRoute(summer, target);
    assert.ok(route && route.length >= 2);
    assert.equal(route[0], landSample);
    assert.equal(route[route.length - 1], target);
    assert.equal(traceTravelRoute(summer, oceanSample), null);

    // Winter field succeeds and matches exact metadata; ocean remains unreachable.
    const winter = decodeTraversalField(
      await session.request({ command: 'travelTime', season: 'winter', sources: [landSample] }), n, 1);
    assert.equal(winter.season, 'winter');
    assert.equal(winter.days[oceanSample], -1);
    const fromWater = decodeTraversalField(
      await session.request({ command: 'travelTime', season: 'winter', sources: [oceanSample] }), n, 1);
    assert.ok(fromWater.days.every(days => days === -1));

    // Multi-source query returns elementwise minimum over single sources.
    const multi = decodeTraversalField(
      await session.request({ command: 'travelTime', season: 'summer', sources: [landSample, target] }), n, 1);
    assert.equal(multi.days[landSample], 0);
    assert.equal(multi.days[target], 0);

    // Advancing vegetation invalidates the cached traversal model and updates vegetationYear.
    decodeVegetation(await session.request({ command: 'advanceVegetation', years: 10 }), n, 1);
    const afterVeg = decodeTraversalCosts(await session.request({ command: 'traversalCosts' }), n, 1);
    assert.equal(afterVeg.vegetationYear, 10);

    // Reject malformed requests and corrupt packets.
    await assert.rejects(session.request({ command: 'travelTime', season: 'summer', sources: [] }), /origin/);
    await assert.rejects(session.request({ command: 'travelTime', season: 'summer', sources: [n] }), /outside/);
    const packet = await session.request({ command: 'traversalCosts' });
    const flags = Buffer.from(packet.bytes);
    flags.writeUInt32LE(0, n * 80 + landSample * 4);
    assert.throws(() => decodeTraversalCosts({ ...packet, bytes: flags }, n, 1), /traversal-cost values/);
    assert.throws(() => decodeTraversalCosts(packet, n + 1, 1), /traversal-cost metadata/);
    assert.throws(() => decodeTraversalCosts({ ...packet, bytes: packet.bytes.subarray(8) }, n, 1), /traversal-cost metadata/);
    const fieldPacket = await session.request({ command: 'travelTime', season: 'summer', sources: [landSample] });
    assert.throws(() => decodeTraversalField(fieldPacket, n + 1, 1), /traversal-field metadata/);
    const cyclic = Buffer.from(fieldPacket.bytes);
    cyclic.writeUInt32LE(target, n * 16 + target * 4);
    assert.throws(() => decodeTraversalField({ ...fieldPacket, bytes: cyclic }, n, 1), /traversal-field values/);
    assert.throws(() => decodeTraversalField({ ...fieldPacket, bytes: fieldPacket.bytes.subarray(8) }, n, 1), /traversal-field metadata/);
  } finally {
    session.close();
  }
});
