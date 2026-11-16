import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { BIOMES, CLIMATE_MODEL_VERSION } from '../src/core/climate-normals';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { NativeSession, decodeClimateNormals, decodeWorld } from '../src/native/client';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...NEW_WORLD_RECIPE, subdivision: 4 };

test('climate normals decode, stay read-only and classify only dry land', async () => {
  const session = new NativeSession(executable);
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    const packet = await session.request({ command: 'climateNormals' });
    const climate = decodeClimateNormals(packet, world.stats.regionCount, 1);
    assert.equal(climate.modelVersion, CLIMATE_MODEL_VERSION);
    assert.deepEqual(decodeClimateNormals(await session.request({ command: 'climateNormals' }), world.stats.regionCount, 1), climate);
    const seen = new Set<number>();
    for (let i = 0; i < world.stats.regionCount; i++) {
      assert.equal(climate.biome[i] === 0, world.water.bodyIds[i] !== 0);
      if (!world.water.bodyIds[i]) seen.add(climate.biome[i]);
    }
    assert.ok(seen.size >= 5 && [...seen].every(b => b > 0 && b < BIOMES.length));
    // Climate is derived on demand and never changes the generated world.
    const again = decodeWorld(await session.request({ command: 'generate', recipe }));
    assert.equal(again.checksum, world.checksum);
    assert.throws(() => decodeClimateNormals(packet, world.stats.regionCount + 1, 1), /climate-normal metadata/);
    assert.throws(() => decodeClimateNormals({ ...packet, bytes: packet.bytes.subarray(1) }, world.stats.regionCount, 1), /climate-normal metadata/);
  } finally { session.close(); }
});
