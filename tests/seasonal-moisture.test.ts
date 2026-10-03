import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { NativeController, NativeSession, decodeWorld, FrameReader } from '../src/native/client';
import type { Packet } from '../src/native/client';
import { decodeSeasonalMoisture } from '../src/native/seasonal-moisture';
import { MOISTURE_STOCK_FIELDS, SURFACE_TRANSFER_FIELDS, RUNOFF_TRANSFER_FIELDS } from '../src/shared/seasonal-moisture';
import { MOISTURE_LAYERS, isMoistureLayer, moistureLayerValue, moistureLayerColor, moistureCalendar } from '../src/renderer/seasonal-moisture';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...DEFAULT_RECIPE, subdivision: 2 };

test('seasonal desktop snapshots match hourly/daily headless native state without changing geography', async () => {
  const daily = new NativeController(executable), hourly = new NativeController(executable);
  try {
    const a = await daily.generate(recipe), b = await hourly.generate(recipe);
    daily.accept(a.epoch); hourly.accept(b.epoch);
    const unchanged = structuredClone(a.world);
    const initial = await daily.seasonalMoisture(a.epoch, 0);
    assert.equal(initial.elapsedSeconds, 0); assert.equal(initial.intervalSeconds, 0);
    assert.equal(initial.budget.surfaceKilograms, initial.budget.initialMobileWaterKilograms);
    await hourly.seasonalMoisture(b.epoch, 0);
    const firstDay = await daily.seasonalMoisture(a.epoch, 86400);
    let firstHours = initial;
    for (let hour = 0; hour < 24; hour++) firstHours = await hourly.seasonalMoisture(b.epoch, 3600);
    assert.equal(firstDay.elapsedSeconds, 86400); assert.equal(firstHours.elapsedSeconds, 86400);
    assert.deepEqual(firstDay.stocks, firstHours.stocks);
    assert.deepEqual(firstDay.budget, firstHours.budget);
    assert.ok(firstDay.budget.vaporKilograms > 0);
    assert.notDeepEqual(firstDay.stocks.vaporKilograms, initial.stocks.vaporKilograms);
    const read = await daily.seasonalMoisture(a.epoch, 0);
    assert.deepEqual(read.stocks, firstDay.stocks); assert.deepEqual(read.budget, firstDay.budget);
    for (const field of [...Object.values(read.surfaceTransfers), ...Object.values(read.runoffTransfers)]) assert.ok(field.every((v) => v === 0));
    assert.deepEqual(daily.resolvedInitialWorld(a.epoch), unchanged);
    assert.deepEqual(a.world, unchanged);
    for (const stock of MOISTURE_STOCK_FIELDS) {
      assert.ok(firstDay.stocks[stock] instanceof Float64Array);
      assert.equal(firstDay.stocks[stock].length, a.world.stats.regionCount);
    }
  } finally { daily.close(); hourly.close(); }
});

test('seasonal commands reject invalid clocks, overlapping requests, stale epochs, and mixed inventories', async () => {
  const core = new NativeController(executable);
  try {
    const { world, epoch } = await core.generate(recipe); core.accept(epoch);
    await assert.rejects(core.seasonalMoisture(epoch, 1), /Initialize/);
    for (const seconds of [-1, 0.5, NaN, Infinity, 86401]) await assert.rejects(core.seasonalMoisture(epoch, seconds), /Invalid/);
    await assert.rejects(core.seasonalMoisture(epoch + 1, 0), /matching/);
    const initializing = core.seasonalMoisture(epoch, 0);
    await assert.rejects(core.seasonalMoisture(epoch, 0), /busy/);
    const initial = await initializing;
    await assert.rejects(core.prescribeWater(epoch, 0, 'oneCubicKilometer'), /separate manual/);
    await assert.rejects(core.exportWaterCheckpoint(epoch), /does not contain seasonal/);
    const advancing = core.seasonalMoisture(epoch, 86400);
    const candidate = await core.generate({ ...recipe, seed: 'seasonal-candidate' });
    await assert.rejects(core.seasonalMoisture(epoch, 0), /matching/);
    core.cancel();
    const advanced = await advancing;
    assert.equal(advanced.elapsedSeconds, 86400);
    assert.deepEqual((await core.seasonalMoisture(epoch, 0)).stocks, advanced.stocks);
    assert.deepEqual(core.resolvedInitialWorld(epoch), world);
    assert.throws(() => core.accept(candidate.epoch), /matching/);
    const fresh = await core.generate(recipe); core.accept(fresh.epoch);
    assert.deepEqual((await core.seasonalMoisture(fresh.epoch, 0)).stocks, initial.stocks);
    const manual = await core.generate({ ...recipe, water: { mode: 'coverage', fraction: 1 } }); core.accept(manual.epoch);
    await core.prescribeWater(manual.epoch, 0, 'oneCubicKilometer');
    await assert.rejects(core.seasonalMoisture(manual.epoch, 0), /manual water input/);
    assert.equal((await core.prescribeWater(manual.epoch, 0, 'oneCubicKilometer')).step, 2);
  } finally { core.close(); }
});

test('daily desktop accounting continues through monthly forcing and a full year boundary', async () => {
  const core = new NativeController(executable);
  try {
    const { world, epoch } = await core.generate(recipe); core.accept(epoch);
    await core.seasonalMoisture(epoch, 0);
    let previousRain = 0;
    for (let day = 1; day <= 366; day++) {
      const frame = await core.seasonalMoisture(epoch, 86400);
      assert.equal(frame.elapsedSeconds, day * 86400);
      assert.ok(frame.budget.cumulativePrecipitationKilograms >= previousRain);
      previousRain = frame.budget.cumulativePrecipitationKilograms;
      assert.ok(Math.abs(frame.budget.residualKilograms) / Math.max(frame.budget.initialMobileWaterKilograms, 1) < 1e-12);
      if (day === 366) {
        assert.ok(previousRain > 0);
        assert.ok(frame.budget.cumulativeRunoffTransfers.terminalDelivery > 0);
        assert.ok(frame.budget.cumulativeRunoffTransfers.terminalEvaporation > 0);
        assert.equal(world.checksum, core.resolvedInitialWorld(epoch).checksum);
      }
    }
  } finally { core.close(); }
});

test('protocol 11 validates finite ownership, pinned settings, exact frame shapes, and flow/budget reconciliation', async () => {
  const session = new NativeSession(executable);
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    await assert.rejects(session.request({ command: 'seasonalMoisture', seconds: 86401 }), /86400/);
    await assert.rejects(session.request({ command: 'seasonalMoisture', seconds: 1 }), /Initialize/);
    const packet = await session.request({ command: 'seasonalMoisture', seconds: 0 });
    const initial = decodeSeasonalMoisture(packet, world, 1, 0, 0);
    const corrupt = (change: (packet: Packet) => void): void => {
      const altered = { header: structuredClone(packet.header), bytes: Buffer.from(packet.bytes) };
      change(altered); assert.throws(() => decodeSeasonalMoisture(altered, world, 1, 0, 0));
    };
    corrupt((p) => { p.header.protocol = 10; });
    corrupt((p) => { p.header.byteLength = 0; });
    corrupt((p) => { p.header.modelVersion = 'future'; });
    corrupt((p) => { p.header.runoffModelVersion = 'future'; });
    corrupt((p) => { p.header.elapsedSeconds = 1; });
    corrupt((p) => { p.header.coupledSubsteps = 1; });
    corrupt((p) => { (p.header.settings as Record<string, unknown>).routingEnabled = false; });
    corrupt((p) => { p.bytes = p.bytes.subarray(8); });
    corrupt((p) => { p.bytes.writeDoubleLE(NaN, 0); });
    corrupt((p) => { p.bytes.writeDoubleLE(-1, 0); });
    corrupt((p) => { (p.header.budget as Record<string, unknown>).residualKilograms = 1e30; });
    corrupt((p) => { (p.header.budget as Record<string, unknown>).unexpected = 0; });
    corrupt((p) => { p.bytes.writeDoubleLE(1, world.stats.regionCount * 6 * 8); });
    // The standalone frame validator must also reject a fabricated pool away from a terminal.
    const dayPacket = await session.request({ command: 'seasonalMoisture', seconds: 86400 });
    const day = decodeSeasonalMoisture(dayPacket, world, 1, 86400, 86400, initial.budget);
    const wrongFlow = { header: structuredClone(dayPacket.header), bytes: Buffer.from(dayPacket.bytes) };
    wrongFlow.bytes.writeDoubleLE(1e30, world.stats.regionCount * 6 * 8);
    assert.throws(() => decodeSeasonalMoisture(wrongFlow, world, 1, 86400, 86400, initial.budget), /reconcile/);
    const wrongOwner = { header: structuredClone(dayPacket.header), bytes: Buffer.from(dayPacket.bytes) };
    const dry = world.drainage.receivers.findIndex((receiver, i) => receiver !== i);
    assert.ok(dry >= 0);
    wrongOwner.bytes.writeDoubleLE(1, (world.stats.regionCount * 4 + dry) * 8);
    assert.throws(() => decodeSeasonalMoisture(wrongOwner, world, 1, 86400, 86400), /ownership/);
    // Real framed output survives arbitrary pipe fragmentation and is confined to its new kind/version.
    const header = Buffer.from(JSON.stringify(dayPacket.header)), prefix = Buffer.alloc(4); prefix.writeUInt32LE(header.length);
    const encoded = Buffer.concat([prefix, header, dayPacket.bytes]), received: Packet[] = [];
    const reader = new FrameReader((value) => received.push(value));
    for (let offset = 0; offset < encoded.length; offset += 137) reader.push(encoded.subarray(offset, offset + 137));
    assert.equal(received.length, 1);
    assert.deepEqual(decodeSeasonalMoisture(received[0], world, 1, 86400, 86400), day);
    const wrongKind = Buffer.from(JSON.stringify({ ...dayPacket.header, kind: 'world' })); prefix.writeUInt32LE(wrongKind.length);
    assert.throws(() => new FrameReader(() => {}).push(Buffer.concat([prefix, wrongKind])));
    await assert.rejects(session.request({ command: 'prescribeWater', region: 0, mode: 'oneCubicKilometer' }), /separate manual/);
    assert.deepEqual(decodeSeasonalMoisture(await session.request({ command: 'seasonalMoisture', seconds: 0 }), world, 1, 86400, 0).stocks, day.stocks);
  } finally { session.close(); }
});

test('display helpers convert recorded masses to WE/rates without mutating stocks or rescaling colors', async () => {
  const core = new NativeController(executable);
  try {
    const { world, epoch } = await core.generate(recipe); core.accept(epoch);
    const frame = await core.seasonalMoisture(epoch, 0), saved = structuredClone(frame);
    const wet = world.water.depthMeters.findIndex((depth) => depth >= 1), area = world.surface.areasSquareMeters[wet];
    assert.ok(wet >= 0);
    assert.equal(moistureLayerValue(frame, 'liquidWater', wet, area), 1000);
    assert.equal(moistureLayerValue(frame, 'precipitation', wet, area), 0);
    assert.equal(moistureLayerColor(1000, 'liquidWater'), 1);
    assert.equal(moistureLayerColor(2000, 'liquidWater'), 1);
    assert.equal(moistureLayerColor(0, 'vaporWater'), 0);
    for (const [layer, info] of Object.entries(MOISTURE_LAYERS)) {
      assert.ok(isMoistureLayer(layer));
      assert.ok(moistureLayerColor(info.maximum / 2, layer) > 0);
      assert.equal(moistureLayerColor(info.maximum, layer), 1);
    }
    assert.equal(isMoistureLayer('surface'), false);
    assert.equal(isMoistureLayer('toString'), false);
    assert.equal(moistureCalendar(86400 + 3660), 'Year 1 · day 2 · 01:01 · elapsed 1.042 days');
    assert.equal(MOISTURE_STOCK_FIELDS.length + SURFACE_TRANSFER_FIELDS.length + RUNOFF_TRANSFER_FIELDS.length, 18);
    assert.deepEqual(frame, saved);
    const day = await core.seasonalMoisture(epoch, 86400), synthetic = structuredClone(day);
    synthetic.surfaceTransfers.rain[wet] = area * 2; synthetic.surfaceTransfers.snowfall[wet] = area * 3;
    synthetic.runoffTransfers.sent[wet] = 1000 * 86400 * 7;
    assert.equal(moistureLayerValue(synthetic, 'precipitation', wet, area), 5);
    assert.equal(moistureLayerValue(synthetic, 'runoffFlow', wet, area), 7);
    synthetic.intervalSeconds = 3600;
    assert.equal(moistureLayerValue(synthetic, 'precipitation', wet, area), 120);
    assert.equal(moistureLayerValue(synthetic, 'runoffFlow', wet, area), 168);
  } finally { core.close(); }
});
