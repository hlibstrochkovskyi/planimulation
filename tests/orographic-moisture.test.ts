import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { NativeController, NativeSession, decodeWorld, FrameReader } from '../src/native/client';
import type { Packet } from '../src/native/client';
import { decodeSeasonalMoisture } from '../src/native/seasonal-moisture';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...DEFAULT_RECIPE, subdivision: 2 };

test('opt-in upslope response keeps mode selection isolated and hourly/daily batching exact', async () => {
  const a = new NativeController(executable), b = new NativeController(executable);
  try {
    const x = await a.generate(recipe), y = await b.generate(recipe); a.accept(x.epoch); b.accept(y.epoch);
    await assert.rejects(a.initializeOrographicMoisture(x.epoch + 1), /matching/);
    const pending = a.initializeOrographicMoisture(x.epoch);
    await assert.rejects(a.initializeOrographicMoisture(x.epoch), /busy/);
    const initial = await pending; await b.initializeOrographicMoisture(y.epoch);
    assert.equal(initial.modelVersion, 'seasonal-moisture-4');
    assert.equal(initial.elapsedSeconds, 0); assert.equal(initial.budget.vaporKilograms, 0);
    await assert.rejects(a.initializeOrographicMoisture(x.epoch), /Regenerate/);
    await assert.rejects(a.prescribeWater(x.epoch, 0, 'oneCubicKilometer'), /separate manual/);
    const daily = await a.seasonalMoisture(x.epoch, 86400); let hourly = initial;
    for (let i = 0; i < 24; i++) hourly = await b.seasonalMoisture(y.epoch, 3600);
    assert.deepEqual(daily.stocks, hourly.stocks); assert.deepEqual(daily.budget, hourly.budget);
    assert.deepEqual(a.resolvedInitialWorld(x.epoch), x.world);
    const fresh = await a.generate(recipe); a.accept(fresh.epoch);
    assert.equal((await a.seasonalMoisture(fresh.epoch, 0)).modelVersion, 'seasonal-moisture-3');
    await assert.rejects(a.initializeOrographicMoisture(fresh.epoch), /Regenerate/);
  } finally { a.close(); b.close(); }
});

test('schema-4 replay retains complete state and rejects altered pins transactionally', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    const fresh = await core.generate(recipe); core.accept(fresh.epoch);
    await core.initializeOrographicMoisture(fresh.epoch);
    let current = await core.seasonalMoisture(fresh.epoch, 86400);
    for (let i = 1; i < 40; i++) current = await core.seasonalMoisture(fresh.epoch, 86400);
    const text = await core.exportSeasonalCheckpoint(fresh.epoch); const checkpoint = JSON.parse(text);
    assert.equal(checkpoint.schemaVersion, 4); assert.equal(checkpoint.orographicModelVersion, 'orographic-response-1');
    assert.ok(current.budget.cumulativePrecipitationKilograms > 0);
    const loaded = await replay.loadSeasonalCheckpoint(text); replay.accept(loaded.epoch);
    assert.deepEqual(loaded.moistureFrame.stocks, current.stocks);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), text);
    for (const edit of [
      (cp: typeof checkpoint) => { cp.schemaVersion = 3; },
      (cp: typeof checkpoint) => { cp.modelVersion = 'seasonal-moisture-3'; },
      (cp: typeof checkpoint) => { cp.orographicModelVersion = 'future'; },
      (cp: typeof checkpoint) => { delete cp.orographicModelVersion; },
      (cp: typeof checkpoint) => { cp.settings.orography = null; },
      (cp: typeof checkpoint) => { cp.settings.orography.strength = 0.5; },
    ]) {
      const invalid = structuredClone(checkpoint); edit(invalid);
      await assert.rejects(replay.loadSeasonalCheckpoint(JSON.stringify(invalid)));
      assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), text);
    }
    await core.seasonalMoisture(fresh.epoch, 86400); await replay.seasonalMoisture(loaded.epoch, 86400);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), await core.exportSeasonalCheckpoint(fresh.epoch));
    await assert.rejects(replay.initializeOrographicMoisture(loaded.epoch), /Regenerate/);
  } finally { core.close(); replay.close(); }
});

test('protocol-12 frames pin the new mode and reject cross-version metadata', async () => {
  const session = new NativeSession(executable);
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    const packet = await session.request({ command: 'initializeOrographicMoisture' });
    assert.equal(packet.header.protocol, 12); assert.equal(packet.bytes.length, world.stats.regionCount * 18 * 8);
    assert.equal(decodeSeasonalMoisture(packet, world, 1, 0, 0, undefined, true).modelVersion, 'seasonal-moisture-4');
    assert.throws(() => decodeSeasonalMoisture(packet, world, 1, 0, 0));
    for (const change of [
      (p: Packet) => { p.header.protocol = 11; }, (p: Packet) => { p.header.kind = 'moisture'; },
      (p: Packet) => { p.header.orographicModelVersion = 'future'; },
      (p: Packet) => { delete p.header.orographicModelVersion; },
      (p: Packet) => { (p.header.settings as { orography: { strength: number } }).orography.strength = 0; },
      (p: Packet) => { p.bytes.writeDoubleLE(NaN, 0); },
    ]) {
      const corrupt = { header: structuredClone(packet.header), bytes: Buffer.from(packet.bytes) }; change(corrupt);
      assert.throws(() => decodeSeasonalMoisture(corrupt, world, 1, 0, 0, undefined, true));
    }
    const checkpoint = await session.request({ command: 'exportMoisture' });
    assert.equal(checkpoint.header.kind, 'orographicMoistureCheckpoint'); assert.equal(checkpoint.header.protocol, 12);
    await assert.rejects(session.request({ command: 'initializeOrographicMoisture' }), /Regenerate/);
    assert.equal((await session.request({ command: 'seasonalMoisture', seconds: 0 })).header.modelVersion, 'seasonal-moisture-4');
  } finally { session.close(); }
  for (const kind of ['orographicMoisture', 'orographicMoistureCheckpoint']) {
    const header = Buffer.from(JSON.stringify({ protocol: 11, kind, byteLength: 0 }));
    const length = Buffer.alloc(4); length.writeUInt32LE(header.length);
    assert.throws(() => new FrameReader(() => assert.fail()).push(Buffer.concat([length, header])), /protocol/);
  }
});

test('largest-grid upslope checkpoint supports exact short continuation', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    const world = await core.generate({ ...recipe, subdivision: 6 }); core.accept(world.epoch);
    await core.initializeOrographicMoisture(world.epoch);
    await core.seasonalMoisture(world.epoch, 3600);
    const text = await core.exportSeasonalCheckpoint(world.epoch);
    const loaded = await replay.loadSeasonalCheckpoint(text); replay.accept(loaded.epoch);
    assert.equal(loaded.world.stats.regionCount, 40962);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), text);
    await core.seasonalMoisture(world.epoch, 3600); await replay.seasonalMoisture(loaded.epoch, 3600);
    assert.equal(await core.exportSeasonalCheckpoint(world.epoch), await replay.exportSeasonalCheckpoint(loaded.epoch));
    console.log(`Level-6 upslope checkpoint: ${Buffer.byteLength(text)} bytes; exact one-hour continuation.`);
  } finally { core.close(); replay.close(); }
});
