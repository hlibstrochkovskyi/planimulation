import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { NativeController } from '../src/native/client';
import { RESOLVED_WORLD_FORMAT, resolvedWorldJson, writeResolvedWorld } from '../src/native/resolved-export';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');

test('resolved initial-world report is complete, deterministic, and distinct from dynamic saves', async () => {
  const core = new NativeController(executable);
  const folder = await mkdtemp(path.join(os.tmpdir(), 'planimulation-resolved-'));
  try {
    const { world, epoch } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 2, water: { mode: 'coverage', fraction: 0 } });
    assert.throws(() => core.resolvedInitialWorld(epoch), /active world/);
    core.accept(epoch);
    assert.equal(core.resolvedInitialWorld(epoch), world);
    assert.throws(() => core.resolvedInitialWorld(epoch + 1), /active world/);
    const first = path.join(folder, 'first.json'), second = path.join(folder, 'second.json');
    await writeResolvedWorld(first, core.resolvedInitialWorld(epoch));
    const contents = await readFile(first, 'utf8');
    const report = JSON.parse(contents);
    assert.equal(report.format, RESOLVED_WORLD_FORMAT);
    assert.equal(report.formatVersion, 1);
    assert.equal(report.purpose, 'initial-generated-fields-not-a-checkpoint');
    assert.deepEqual(report.recipe, world.recipe);
    assert.equal(report.initialFingerprint, world.checksum);
    assert.deepEqual(report.stats, world.stats);
    for (const group of ['surface', 'tectonics', 'crust', 'terrain', 'water', 'drainage', 'basins'] as const) {
      assert.deepEqual(Object.keys(report[group]).sort(), Object.keys(world[group]).sort(), `${group} fields must be explicit in v1.`);
    }
    assert.deepEqual(report.surface.centers, Array.from(world.surface.centers));
    assert.deepEqual(report.tectonics.owners, Array.from(world.tectonics.owners));
    assert.deepEqual(report.crust.continentality, Array.from(world.crust.continentality));
    assert.deepEqual(report.terrain.elevation, Array.from(world.terrain.elevation));
    assert.deepEqual(report.water.depthMeters, Array.from(world.water.depthMeters));
    assert.deepEqual(report.drainage.receivers, Array.from(world.drainage.receivers));
    assert.deepEqual(report.basins.parents, Array.from(world.basins.parents));
    assert.equal(report.waterCheckpoint, undefined);
    const source = world.drainage.outlets.findIndex((outlet, region) => outlet !== region);
    assert.ok(source >= 0);
    await core.prescribeWater(epoch, source, 'oneCubicKilometer');
    await writeResolvedWorld(second, core.resolvedInitialWorld(epoch));
    assert.equal(await readFile(second, 'utf8'), contents, 'Manual water steps cannot alter the initial-world report.');
    await writeFile(first, 'existing destination');
    const bad = { ...world, terrain: { ...world.terrain, elevation: Float64Array.from(world.terrain.elevation) } };
    bad.terrain.elevation[0] = NaN;
    await assert.rejects(writeResolvedWorld(first, bad), /Non-finite/);
    assert.equal(await readFile(first, 'utf8'), 'existing destination');
    assert.throws(() => [...resolvedWorldJson(bad)], /Non-finite/);
  } finally { core.close(); await rm(folder, { recursive: true, force: true }); }
});

test('resolved export remains within its bound at the highest supported resolution', async () => {
  const core = new NativeController(executable);
  const folder = await mkdtemp(path.join(os.tmpdir(), 'planimulation-resolved-large-'));
  try {
    const { epoch } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 6 });
    core.accept(epoch);
    const destination = path.join(folder, 'large.json');
    await writeResolvedWorld(destination, core.resolvedInitialWorld(epoch));
    const size = (await stat(destination)).size;
    assert.ok(size > 1_000_000 && size < 256 * 2 ** 20);
  } finally { core.close(); await rm(folder, { recursive: true, force: true }); }
});
