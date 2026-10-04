import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { NativeController, NativeSession, decodeWorld, FrameReader } from '../src/native/client';
import type { Packet } from '../src/native/client';
import { decodeSeasonalMoisture } from '../src/native/seasonal-moisture';
import { MOISTURE_STOCK_FIELDS, SURFACE_TRANSFER_FIELDS, RUNOFF_TRANSFER_FIELDS } from '../src/shared/seasonal-moisture';
import { MAX_SEASONAL_CHECKPOINT_BYTES } from '../src/shared/seasonal-checkpoint';
import { MOISTURE_LAYERS, isMoistureLayer, moistureLayerValue, moistureLayerColor, moistureCalendar } from '../src/renderer/seasonal-moisture';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...DEFAULT_RECIPE, subdivision: 2 };
type EditableCheckpoint = Record<string, unknown> & {
  settings: Record<string, unknown>; temperatureSettings: { axialTiltDegrees: number };
  windSettings: Record<string, unknown>; surfaceKilograms: number[]; vaporKilograms: number[];
  soilKilograms: Array<number | null>; cumulativeSurfaceTransferRoundoff: number[][];
};

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
  const core = new NativeController(executable), restored = new NativeController(executable);
  let restoredEpoch = 0;
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
      if (day === 365) {
        const contents = await core.exportSeasonalCheckpoint(epoch);
        const saved = JSON.parse(contents);
        assert.ok(saved.cumulativeSurfaceTransferRoundoff.flat().some((value: number) => value !== 0));
        assert.ok(saved.cumulativeRunoffTransferRoundoff.flat().some((value: number) => value !== 0));
        const loaded = await restored.loadSeasonalCheckpoint(contents);
        assert.deepEqual(loaded.moistureFrame.stocks, frame.stocks);
        assert.deepEqual(loaded.moistureFrame.budget, frame.budget);
        assert.equal(loaded.moistureFrame.intervalSeconds, 0);
        restoredEpoch = loaded.epoch; restored.accept(restoredEpoch);
        assert.equal(await restored.exportSeasonalCheckpoint(restoredEpoch), contents);
      }
      if (day === 366) {
        assert.ok(previousRain > 0);
        assert.ok(frame.budget.cumulativeRunoffTransfers.terminalDelivery > 0);
        assert.ok(frame.budget.cumulativeRunoffTransfers.terminalEvaporation > 0);
        assert.equal(world.checksum, core.resolvedInitialWorld(epoch).checksum);
        const replay = await restored.seasonalMoisture(restoredEpoch, 86400);
        assert.deepEqual(replay.stocks, frame.stocks); assert.deepEqual(replay.budget, frame.budget);
        assert.deepEqual(replay.surfaceTransfers, frame.surfaceTransfers);
        assert.deepEqual(replay.runoffTransfers, frame.runoffTransfers);
        assert.equal(await restored.exportSeasonalCheckpoint(restoredEpoch), await core.exportSeasonalCheckpoint(epoch));
      }
    }
  } finally { core.close(); restored.close(); }
});

test('complete seasonal loading is transactional, mode-isolated, pinned, and preserves original numeric tokens', async () => {
  const core = new NativeController(executable), raw = new NativeSession(executable);
  try {
    const { world, epoch } = await core.generate(recipe); core.accept(epoch);
    await assert.rejects(core.exportSeasonalCheckpoint(epoch), /Initialize/);
    await core.seasonalMoisture(epoch, 0);
    const initial = await core.exportSeasonalCheckpoint(epoch);
    // A signed zero correction is valid, but JS JSON.parse/stringify loses its
    // sign. Preserve original checkpoint text all the way to Rust.
    const signed = initial.replace('"cumulativeSurfaceTransferRoundoff":[[0.0,', '"cumulativeSurfaceTransferRoundoff":[[-0.0,');
    assert.notEqual(signed, initial);
    const signedPrepared = await core.loadSeasonalCheckpoint(signed); core.accept(signedPrepared.epoch);
    assert.equal(await core.exportSeasonalCheckpoint(signedPrepared.epoch), signed);
    const firstDay = await core.seasonalMoisture(signedPrepared.epoch, 86400);
    const saved = await core.exportSeasonalCheckpoint(signedPrepared.epoch);
    const mutations: Array<(value: EditableCheckpoint) => void> = [
      (cp) => { cp.schemaVersion = 2; }, (cp) => { cp.modelVersion = 'future'; },
      ...['transportModelVersion', 'temperatureModelVersion', 'windModelVersion', 'surfaceModelVersion', 'runoffModelVersion']
        .map((key) => (cp: EditableCheckpoint) => { cp[key] = 'future'; }),
      (cp) => { cp.settings.routingEnabled = false; },
      (cp) => { cp.temperatureSettings.axialTiltDegrees += 1; },
      (cp) => { cp.windSettings.unexpected = 0; },
      (cp) => { cp.unexpected = 1; }, (cp) => { cp.elapsedSeconds = -1; },
      (cp) => { cp.elapsedSeconds = 3650 * 86400 + 1; },
      (cp) => { cp.surfaceKilograms[0] += 1e30; },
      (cp) => { cp.vaporKilograms.pop(); }, (cp) => { cp.soilKilograms[0] = null; },
      (cp) => { cp.cumulativeSurfaceTransferRoundoff[0][0] = 1e30; },
    ];
    for (const mutate of mutations) {
      const cp = JSON.parse(saved) as EditableCheckpoint; mutate(cp);
      await assert.rejects(core.loadSeasonalCheckpoint(JSON.stringify(cp)));
      assert.equal(await core.exportSeasonalCheckpoint(signedPrepared.epoch), saved);
      assert.equal(core.resolvedInitialWorld(signedPrepared.epoch).checksum, world.checksum);
    }
    await assert.rejects(core.loadSeasonalCheckpoint('{invalid'));
    await assert.rejects(core.loadSeasonalCheckpoint('{"inventoryVersion":"prescribed-water-inventory-2"}'), /version/);
    // Whitespace/newlines are safe inside the escaped restore envelope; no
    // numeric tokens need to be normalized to fit a single command line.
    const candidate = await core.loadSeasonalCheckpoint(saved.replace('{', '{\n  '));
    await assert.rejects(core.seasonalMoisture(signedPrepared.epoch, 0), /matching/);
    core.cancel(); assert.throws(() => core.accept(candidate.epoch), /matching/);
    assert.deepEqual((await core.seasonalMoisture(signedPrepared.epoch, 0)).stocks, firstDay.stocks);
    const advancing = core.seasonalMoisture(signedPrepared.epoch, 3600);
    const preparing = core.loadSeasonalCheckpoint(saved);
    const canceled = assert.rejects(preparing, /canceled|closed/);
    core.cancel(); await canceled;
    const advanced = await advancing;
    assert.equal(advanced.elapsedSeconds, firstDay.elapsedSeconds + 3600);
    assert.deepEqual((await core.seasonalMoisture(signedPrepared.epoch, 0)).stocks, advanced.stocks);
    const replacement = await core.generate({ ...recipe, seed: 'before-seasonal-restore' }); core.accept(replacement.epoch);
    const loaded = await core.loadSeasonalCheckpoint(saved); core.accept(loaded.epoch);
    assert.equal(core.resolvedInitialWorld(loaded.epoch).checksum, world.checksum);
    assert.equal(await core.exportSeasonalCheckpoint(loaded.epoch), saved);
    await assert.rejects(core.exportSeasonalCheckpoint(signedPrepared.epoch), /matching/);
    await assert.rejects(core.prescribeWater(loaded.epoch, 0, 'oneCubicKilometer'), /separate manual/);
    // Direct native commands must enforce the same provenance/default/mode
    // restrictions even when bypassing TypeScript preflight.
    await raw.request({ command: 'generate', recipe: { ...recipe, seed: 'wrong-seasonal-origin' } });
    await assert.rejects(raw.request({ command: 'restoreMoisture', checkpointJson: saved }), /recipe/);
    await raw.request({ command: 'generate', recipe });
    await raw.request({ command: 'restoreMoisture', checkpointJson: saved });
    await assert.rejects(raw.request({ command: 'restoreMoisture', checkpointJson: saved }), /separate generated/);
    assert.equal((await raw.request({ command: 'exportMoisture' })).bytes.toString('utf8') + '\n', saved);
  } finally { core.close(); raw.close(); }
});

test('seasonal checkpoints replay at the largest supported grid without the manual eight-MiB cap', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    const prepared = await core.generate({ ...DEFAULT_RECIPE, subdivision: 6 }); core.accept(prepared.epoch);
    await core.seasonalMoisture(prepared.epoch, 0);
    await core.seasonalMoisture(prepared.epoch, 86400);
    const current = await core.seasonalMoisture(prepared.epoch, 86400);
    const contents = await core.exportSeasonalCheckpoint(prepared.epoch);
    const bytes = Buffer.byteLength(contents);
    assert.ok(bytes > 8 * 2 ** 20 && bytes <= MAX_SEASONAL_CHECKPOINT_BYTES);
    const loaded = await replay.loadSeasonalCheckpoint(contents); replay.accept(loaded.epoch);
    assert.equal(loaded.world.stats.regionCount, 40962);
    assert.deepEqual(loaded.moistureFrame.stocks, current.stocks);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), contents);
    await core.seasonalMoisture(prepared.epoch, 3600); await replay.seasonalMoisture(loaded.epoch, 3600);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), await core.exportSeasonalCheckpoint(prepared.epoch));
    console.log(`Level-6 two-day seasonal checkpoint: ${bytes} bytes; exact one-hour continuation.`);
  } finally { core.close(); replay.close(); }
});

test('checkpoint protocol and command budgets stay kind-specific', async () => {
  const prefix = Buffer.alloc(4);
  for (const header of [
    { protocol: 11, kind: 'moistureCheckpoint', byteLength: MAX_SEASONAL_CHECKPOINT_BYTES + 1 },
    { protocol: 9, kind: 'moistureCheckpoint', byteLength: 1 },
    { protocol: 11, kind: 'moisture', byteLength: 32 * 2 ** 20 + 1 },
    { protocol: 9, kind: 'checkpoint', byteLength: 32 * 2 ** 20 + 1 },
  ]) {
    const encoded = Buffer.from(JSON.stringify(header)); prefix.writeUInt32LE(encoded.length);
    assert.throws(() => new FrameReader(() => {}).push(Buffer.concat([prefix, encoded])));
  }
  const session = new NativeSession(executable);
  try {
    await assert.rejects(session.request({ command: 'restoreMoisture', checkpointJson: ' '.repeat(MAX_SEASONAL_CHECKPOINT_BYTES + 1) }), /64 MiB/);
    await assert.rejects(session.request({ command: 'restoreMoisture', checkpointJson: {} }), /JSON text/);
    await assert.rejects(session.request({ command: 'generate', recipe: ' '.repeat(8 * 2 ** 20) }), /too large/);
    await assert.rejects(session.request({ command: 'exportMoisture' }), /Initialize/);
  } finally { session.close(); }
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
