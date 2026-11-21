import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE, MULTISCALE_ERODED_MODEL_VERSION, MULTISCALE_LOWLANDS_MODEL_VERSION, parseRecipe, serializeRecipe } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { NativeSession, decodeWorld } from '../src/native/client';
import { resolvedWorldRecord } from '../src/native/resolved-export';
import { MAX_SEDIMENT_METERS, SEDIMENT_MODEL_VERSION } from '../src/core/terrain';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe: Recipe = { ...DEFAULT_RECIPE, modelVersion: MULTISCALE_ERODED_MODEL_VERSION, subdivision: 4 };

test('version-5 worlds carry ocean-floor, fluvial relief and sediment contributions and allow subdivision 7', async () => {
  assert.deepEqual(parseRecipe(JSON.parse(serializeRecipe(recipe))), recipe);
  assert.equal(parseRecipe({ ...recipe, subdivision: 7 }).subdivision, 7);
  assert.throws(() => parseRecipe({ ...recipe, subdivision: 8 }), /0 to 7/);
  assert.throws(() => parseRecipe({ ...recipe, modelVersion: MULTISCALE_LOWLANDS_MODEL_VERSION, subdivision: 7 }), /0 to 6/);
  const session = new NativeSession(executable);
  try {
    const packet = await session.request({ command: 'generate', recipe });
    const world = decodeWorld(packet);
    assert.deepEqual(decodeWorld(await session.request({ command: 'generate', recipe })), world);
    assert.throws(() => decodeWorld({ ...packet, header: { ...packet.header, protocol: 10 } }), /does not match/);
    assert.throws(() => decodeWorld({ ...packet, header: { ...packet.header, sedimentModelVersion: 'unknown' } }), /does not match/);
    const { seafloor, erosion, lowland, sediment } = world.terrain;
    assert.ok(seafloor && erosion && lowland && sediment);
    // The suffix holds sediment, elevation, water, drainage and basin arrays.
    const sedimentOffset = packet.bytes.length - (52 * sediment.length + 20 + 44 * world.basins.parents.length);
    for (const badValue of [-1, MAX_SEDIMENT_METERS + 1, sediment[0] + 1]) {
      const bytes = Buffer.from(packet.bytes);
      bytes.writeDoubleLE(badValue, sedimentOffset);
      assert.throws(() => decodeWorld({ ...packet, bytes }), /Invalid native elevation contributions/);
    }
    assert.ok(seafloor.some(value => Math.abs(value) > 100), 'The native ocean floor changes the version-5 bed.');
    assert.ok(sediment.some(value => value > 0));
    const prism = (values: Float64Array) => values.reduce((sum, v, i) => sum + Math.max(0, v) * world.surface.areasSquareMeters[i], 0);
    assert.ok(prism(sediment) <= prism(erosion.map(value => -value)) * (1 + 1e-12));
    const exported = resolvedWorldRecord(world) as { terrain: { seafloor: Float64Array; erosion: Float64Array; sediment: Float64Array; sedimentModelVersion: string } };
    assert.deepEqual(exported.terrain.seafloor, seafloor);
    assert.deepEqual(exported.terrain.erosion, erosion);
    assert.deepEqual(exported.terrain.sediment, sediment);
    assert.equal(exported.terrain.sedimentModelVersion, SEDIMENT_MODEL_VERSION);
    const v4 = decodeWorld(await session.request({ command: 'generate', recipe: { ...recipe, modelVersion: MULTISCALE_LOWLANDS_MODEL_VERSION } }));
    assert.equal(v4.terrain.seafloor, undefined);
    assert.equal(v4.terrain.sediment, undefined);
  } finally { session.close(); }
});
