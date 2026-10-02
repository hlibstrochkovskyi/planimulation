import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { generateWorld } from '../src/core/world';
import { FrameReader, NativeController, NativeSession, decodeWorld } from '../src/native/client';
import type { Packet } from '../src/native/client';
import { buildViewGeometry } from '../src/renderer/view-geometry';
import { buildSurface } from '../src/core/surface';
import { speedCmPerYear, summarizeTectonics } from '../src/core/tectonics';
import { summarizeCrust } from '../src/core/crust';
import { basinTree } from '../src/core/basins';
import { runoffDestination } from '../src/renderer/water-budget';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');

test('binary framing handles fragmented headers/bodies, multiple frames, and rejects oversized packets', () => {
  const received: Packet[] = [], reader = new FrameReader((packet) => received.push(packet));
  const body = Buffer.from([1, 2, 3]);
  const header = Buffer.from(JSON.stringify({ protocol: 9, kind: 'frame', byteLength: body.length }));
  const prefix = Buffer.alloc(4); prefix.writeUInt32LE(header.length);
  const packet = Buffer.concat([prefix, header, body]);
  for (const byte of packet) reader.push(Buffer.from([byte]));
  reader.push(Buffer.concat([packet, packet]));
  assert.equal(received.length, 3); assert.deepEqual(received[0].bytes, body);
  for (const size of [0, 1, 32769, 0xffffffff]) {
    const invalid = Buffer.alloc(4); invalid.writeUInt32LE(size);
    assert.throws(() => new FrameReader(() => {}).push(invalid));
  }
  const badHeader = Buffer.from(JSON.stringify({ protocol: 1, kind: 'world', byteLength: 0 }));
  prefix.writeUInt32LE(badHeader.length);
  assert.throws(() => new FrameReader(() => {}).push(Buffer.concat([prefix, badHeader])));
});

test('Rust topology and fields agree with the independent TS reference; repeatability is exact', async () => {
  const core = new NativeController(executable);
  try {
    for (const subdivision of [0, 1, 3, 5, 6]) {
      const recipe = { ...DEFAULT_RECIPE, subdivision, seed: 'bridge-🌍' };
      const { world: actual } = await core.generate(recipe);
      const expected = generateWorld(recipe);
      for (const [key, array] of Object.entries({ ...expected.surface, diagnosticField: expected.diagnosticField })) {
        if (!ArrayBuffer.isView(array)) continue;
        const values = array as Float64Array | Uint32Array;
        const got = key === 'diagnosticField' ? actual.diagnosticField : actual.surface[key as keyof typeof actual.surface] as typeof values;
        assert.equal(got.length, values.length, key);
        for (let i = 0; i < values.length; i++) {
          if (values instanceof Uint32Array) assert.equal(got[i], values[i], key);
          else assert.ok(Math.abs(got[i] - values[i]) / Math.max(1, Math.abs(values[i])) < 1e-11, `${key}[${i}]`);
        }
      }
      assert.deepEqual((await core.generate(recipe)).world, actual);
    }
  } finally { core.close(); }
});

test('cancellation, replacement, step bounds, and backpressure preserve the active world', async () => {
  const core = new NativeController(executable);
  try {
    const { epoch, world } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 3 });
    core.accept(epoch);
    await assert.rejects(core.advance(epoch, 0));
    await assert.rejects(core.advance(epoch + 1, 1));
    const obsolete = core.generate({ ...DEFAULT_RECIPE, subdivision: 6 });
    const rejected = assert.rejects(obsolete, /canceled/);
    core.cancel(); await rejected;
    assert.equal((await core.advance(epoch, 4)).tick, 4);
    const pending = core.advance(epoch, 100);
    await assert.rejects(core.advance(epoch, 1), /busy/);
    const frame = await pending;
    assert.equal(frame.tick, 104); assert.ok(frame.relativeMassError < 1e-12);
    assert.ok(frame.field.every((v) => v >= -1 && v <= 1));
    assert.notDeepEqual(frame.field, world.diagnosticField);
    const prepared = await core.generate({ ...DEFAULT_RECIPE, subdivision: 2 });
    core.cancel();
    assert.throws(() => core.accept(prepared.epoch));
    assert.equal((await core.advance(epoch, 1)).tick, 105);
  } finally { core.close(); }
});

test('prescribed-water frames display a generated basin spill without changing initial water', async () => {
  const core = new NativeController(executable);
  try {
    const recipe = { ...DEFAULT_RECIPE, subdivision: 2, water: { mode: 'coverage' as const, fraction: 0 } };
    const { world, epoch } = await core.generate(recipe);
    core.accept(epoch);
    const children = basinTree(world.basins).children;
    const terminal = world.drainage.receivers.findIndex((receiver, region) => {
      if (receiver !== region) return false;
      const branch = world.basins.regionNodes[region], parent = world.basins.parents[branch], siblings = children[parent];
      return siblings.length === 2 && siblings.every((child) => children[child].length === 0)
        && siblings.every((child) => world.basins.capacities[child] > 1e9);
    });
    assert.ok(terminal >= 0);
    const original = structuredClone(world.water);
    await assert.rejects(core.prescribeWater(epoch, -1, 'fillToSpill'), /Invalid/);
    const first = await core.prescribeWater(epoch, terminal, 'fillToSpill');
    assert.equal(first.step, 1);
    assert.equal(first.depthMeters.length, world.stats.regionCount);
    assert.equal(first.surfaceLevelsMeters.length, world.stats.regionCount);
    assert.equal(first.bodyIds.length, world.stats.regionCount);
    assert.ok(first.depthMeters[terminal] > 0);
    assert.ok(first.mainOceanId > 0);
    const second = await core.prescribeWater(epoch, terminal, 'oneCubicKilometer');
    assert.equal(second.step, 2);
    assert.ok(BigInt(second.acceptedInputUnits) > BigInt(first.acceptedInputUnits));
    assert.ok(second.depthMeters[terminal] >= first.depthMeters[terminal]);
    assert.deepEqual(world.water, original);
    const full = await core.generate({ ...DEFAULT_RECIPE, subdivision: 2, water: { mode: 'coverage', fraction: 1 } });
    core.accept(full.epoch);
    const fullInitial = structuredClone(full.world.water);
    await assert.rejects(core.prescribeWater(full.epoch, 0, 'fillToSpill'), /no spill threshold/);
    const ocean = await core.prescribeWater(full.epoch, 0, 'oneCubicKilometer');
    assert.equal(ocean.step, 1, 'A rejected command must not advance the native water inventory.');
    assert.ok(ocean.bodyIds.every((body) => body === ocean.mainOceanId));
    assert.deepEqual(full.world.water, fullInitial);
  } finally { core.close(); }
});

test('water checkpoint reload preserves exact continuation and rejects damaged state atomically', async () => {
  const original = new NativeController(executable);
  const resumed = new NativeController(executable);
  try {
    const recipe = { ...DEFAULT_RECIPE, subdivision: 2, water: { mode: 'coverage' as const, fraction: 0 } };
    const { world, epoch } = await original.generate(recipe);
    original.accept(epoch);
    const children = basinTree(world.basins).children;
    const source = world.drainage.receivers.findIndex((receiver, region) => {
      if (receiver !== region) return false;
      const branch = world.basins.regionNodes[region], parent = world.basins.parents[branch], siblings = children[parent];
      return siblings.length === 2 && siblings.every((child) => children[child].length === 0)
        && siblings.every((child) => world.basins.capacities[child] > 1e9);
    });
    assert.ok(source >= 0);
    const initial = JSON.parse(await original.exportWaterCheckpoint(epoch));
    assert.equal(initial.step, 0);
    assert.equal(initial.acceptedInputUnits, '0');
    const initialBudget = await original.inspectWaterBudget(epoch);
    assert.equal(initialBudget.storedTotalUnits, initialBudget.initialTotalUnits);
    assert.equal(initialBudget.acceptedInputUnits, '0');
    const before = await original.prescribeWater(epoch, source, 'fillToSpill');
    const budget = await original.inspectWaterBudget(epoch);
    assert.equal(budget.step, before.step);
    assert.equal(budget.acceptedInputUnits, before.acceptedInputUnits);
    assert.equal(BigInt(budget.storedTotalUnits), BigInt(budget.initialTotalUnits) + BigInt(budget.acceptedInputUnits));
    assert.ok(BigInt(runoffDestination(world, budget, source).volumeUnits) > 0n);
    const contents = await original.exportWaterCheckpoint(epoch);
    const checkpoint = JSON.parse(contents);
    assert.equal(checkpoint.inventoryVersion, 'prescribed-water-inventory-2');
    assert.equal(checkpoint.step, before.step);
    assert.equal(checkpoint.acceptedInputUnits, before.acceptedInputUnits);
    await assert.rejects(resumed.loadWaterCheckpoint({ ...checkpoint, acceptedInputUnits: '0' }), /balance/);
    const loaded = await resumed.loadWaterCheckpoint(checkpoint);
    assert.deepEqual(loaded.world, world);
    assert.equal(loaded.waterFrame.step, before.step);
    assert.deepEqual(loaded.waterFrame.depthMeters, before.depthMeters);
    assert.deepEqual(loaded.waterFrame.bodyIds, before.bodyIds);
    resumed.accept(loaded.epoch);
    assert.equal(await resumed.exportWaterCheckpoint(loaded.epoch), contents);
    assert.deepEqual({ ...await resumed.inspectWaterBudget(loaded.epoch), epoch }, budget);
    const a = await original.prescribeWater(epoch, source, 'oneCubicKilometer');
    const b = await resumed.prescribeWater(loaded.epoch, source, 'oneCubicKilometer');
    assert.deepEqual({ ...b, epoch: epoch }, a);
    assert.equal(await resumed.exportWaterCheckpoint(loaded.epoch), await original.exportWaterCheckpoint(epoch));
    await assert.rejects(resumed.loadWaterCheckpoint({ ...checkpoint, inventoryVersion: 'old' }), /Unsupported/);
    assert.equal((await resumed.prescribeWater(loaded.epoch, source, 'oneCubicKilometer')).step, 3,
      'A rejected candidate must preserve the active inventory.');
  } finally { original.close(); resumed.close(); }
});

test('native validation rejects legacy recipes and malformed output is not decoded', async () => {
  const session = new NativeSession(executable);
  try {
    await assert.rejects(session.request({ command: 'advance', steps: 1 }), /Generate/);
    await assert.rejects(session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, modelVersion: 'surface-1' } }), /Unsupported/);
    const packet = await session.request({ command: 'generate', recipe: { ...DEFAULT_RECIPE, subdivision: 0 } });
    assert.throws(() => decodeWorld({ ...packet, bytes: packet.bytes.subarray(1) }), /lengths/);
    const invalid = Buffer.from(packet.bytes); invalid.writeDoubleLE(NaN);
    assert.throws(() => decodeWorld({ ...packet, bytes: invalid }), /finite/);
    assert.throws(() => decodeWorld({ ...packet, header: { ...packet.header, boundarySegmentCount: 1e9 } }), /boundary count/);
    const world = decodeWorld(packet);
    const waterAndDrainageBytes = 20 + world.stats.regionCount * 36 + world.basins.parents.length * 44;
    const extraBytes = world.stats.regionCount * 4 + world.recipe.plateCount * 28 + Number(packet.header.boundarySegmentCount) * 76 + 8 + world.stats.regionCount * 72 + waterAndDrainageBytes;
    const corruptedOwner = Buffer.from(packet.bytes);
    corruptedOwner.writeUInt32LE(world.recipe.plateCount, packet.bytes.length - extraBytes);
    assert.throws(() => decodeWorld({ ...packet, bytes: corruptedOwner }), /plate metadata/);
    for (const [offset, value] of [[0, NaN], [8, 2], [8 + 12 * 8, -0.1], [8 + 24 * 8, 1], [8 + 36 * 8, 4000]]) {
      const invalidCrust = Buffer.from(packet.bytes);
      invalidCrust.writeDoubleLE(value, packet.bytes.length - (8 + 12 * 72) - waterAndDrainageBytes + offset);
      assert.throws(() => decodeWorld({ ...packet, bytes: invalidCrust }), /finite|crust/);
    }
    const invalidHeight = Buffer.from(packet.bytes); invalidHeight.writeDoubleLE(100000, packet.bytes.length - waterAndDrainageBytes - 8);
    assert.throws(() => decodeWorld({ ...packet, bytes: invalidHeight }), /elevation/);
  } finally { session.close(); }
});

test('native plate fields survive transport and produce both boundary overlays without changing the model', async () => {
  const core = new NativeController(executable);
  try {
    const { world, epoch } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 3 });
    core.accept(epoch);
    const original = structuredClone(world.tectonics);
    const summary = summarizeTectonics(world.surface, world.tectonics);
    assert.equal(summary.plates.reduce((sum, plate) => sum + plate.regionCount, 0), world.stats.regionCount);
    assert.ok(Math.abs(summary.plates.reduce((sum, plate) => sum + plate.areaSquareMeters, 0) / world.stats.totalAreaSquareMeters - 1) < 1e-12);
    assert.equal(Object.values(summary.boundarySegmentCounts).reduce((sum, v) => sum + v, 0), world.tectonics.boundaryTypes.length);
    assert.equal(new Set(world.tectonics.owners).size, world.recipe.plateCount);
    const view = buildViewGeometry(world.surface, world.tectonics, world.terrain.elevation);
    assert.equal(view.globe.tectonicOffsets.length, world.tectonics.boundaryTypes.length * 2);
    assert.ok(view.globe.tectonicOffsets.every(Number.isFinite));
    const count = world.tectonics.boundaryTypes.length;
    assert.equal(view.globe.tectonicLines.length, count * 6);
    assert.ok(view.flat.tectonicLines.length >= count * 6);
    for (const data of [view.flat, view.globe]) {
      assert.equal(data.tectonicLines.length, data.tectonicColors.length);
      assert.ok(data.tectonicLines.every(Number.isFinite));
      assert.ok(data.tectonicColors.every((v) => v >= 0 && v <= 1));
    }
    for (let i = 0; i < view.flat.tectonicLines.length; i += 6) {
      assert.ok(Math.abs(view.flat.tectonicLines[i]) <= 1);
      assert.ok(Math.abs(view.flat.tectonicLines[i + 3]) <= 1);
      assert.ok(Math.abs(view.flat.tectonicLines[i] - view.flat.tectonicLines[i + 3]) <= 1);
    }
    for (let id = 0; id < world.stats.regionCount; id++) {
      assert.ok(speedCmPerYear(world.surface, world.tectonics, id) <= world.recipe.maxPlateSpeedCmPerYear + 1e-12);
    }
    assert.deepEqual(world.tectonics, original);
    await core.advance(epoch, 10);
    const stopped = (await core.generate({ ...DEFAULT_RECIPE, subdivision: 3, maxPlateSpeedCmPerYear: 0 })).world;
    assert.deepEqual(stopped.tectonics.owners, world.tectonics.owners);
    assert.ok(stopped.tectonics.boundaryTypes.every((v) => v === 0));
    assert.ok(stopped.tectonics.boundaryMotion.every((v) => v === 0));
  } finally { core.close(); }
});

test('native crust fields and summaries are reproducible, bounded, and independent of tectonics', async () => {
  const core = new NativeController(executable);
  try {
    const recipe = { ...DEFAULT_RECIPE, subdivision: 3 };
    const { world } = await core.generate(recipe);
    const summary = summarizeCrust(world.surface, world.crust);
    assert.ok(Math.abs(summary.continentalAreaFraction - recipe.continentalFraction) < world.stats.maximumAreaSquareMeters / world.stats.totalAreaSquareMeters);
    assert.ok(summary.continentalPatchCount > 0);
    assert.ok(summary.largestContinentalPatchAreaSquareMeters <= summary.continentalAreaFraction * world.stats.totalAreaSquareMeters + 1);
    for (let i = 0; i < world.stats.regionCount; i++) {
      assert.equal(world.crust.thicknessMeters[i], 7000 + 28000 * world.crust.continentality[i]);
      assert.equal(world.crust.densityKgPerCubicMeter[i], 3000 - 200 * world.crust.continentality[i]);
    }
    assert.deepEqual((await core.generate({ ...recipe, plateCount: 2, maxPlateSpeedCmPerYear: 0 })).world.crust, world.crust);
    for (const fraction of [0, 1]) {
      const changed = (await core.generate({ ...recipe, continentalFraction: fraction })).world;
      assert.deepEqual(changed.tectonics, world.tectonics);
      assert.deepEqual(changed.crust.potential, world.crust.potential);
      const result = summarizeCrust(changed.surface, changed.crust);
      assert.equal(result.continentalAreaFraction, fraction);
      assert.equal(result.continentalPatchCount, fraction);
    }
  } finally { core.close(); }
});

test('finest world with maximum plate count fits the native transport budget', async () => {
  const core = new NativeController(executable);
  try {
    const { world, epoch } = await core.generate({ ...DEFAULT_RECIPE, subdivision: 6, plateCount: 32,
      maxPlateSpeedCmPerYear: 20, continentalScale: 0.5, radiusMeters: 100_000 });
    assert.equal(world.stats.regionCount, 40962);
    assert.equal(world.crust.continentality.length, 40962);
    assert.ok(world.stats.arrayBytes < 32 * 2 ** 20);
    core.accept(epoch);
    const checkpoint = await core.exportWaterCheckpoint(epoch);
    assert.ok(Buffer.byteLength(checkpoint) < 8 * 2 ** 20, 'The largest supported world must fit the water-checkpoint limit.');
  } finally { core.close(); }
});

for (const level of [0, 1, 3]) test(`GPU geometry level ${level}: flat atlas covers area without seam overflow; IDs survive both views`, () => {
  const s = buildSurface(level, 1000), views = buildViewGeometry(s), n = s.areasSquareMeters.length;
  let area = 0;
  for (let i = 0; i < views.flat.positions.length; i += 9) {
    const [ax, ay, , bx, by, , cx, cy] = views.flat.positions.subarray(i, i + 9);
    area += Math.abs((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)) / 2;
  }
  assert.ok(Math.abs(area - 2) < 1e-6, `Atlas area: ${area}`);
  for (const view of Object.values(views)) {
    assert.equal(view.positions.length, view.regions.length * 3);
    assert.equal(new Set(view.regions).size, n);
    assert.ok(view.positions.every(Number.isFinite));
    for (let i = 0; i < view.regions.length; i += 3) {
      assert.equal(view.regions[i], view.regions[i + 1]); assert.equal(view.regions[i], view.regions[i + 2]);
    }
  }
  for (let i = 0; i < views.flat.positions.length; i += 3) assert.ok(Math.abs(views.flat.positions[i]) <= 1);
});
