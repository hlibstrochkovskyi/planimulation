import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import path from 'node:path';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { NativeController, NativeSession, decodeWorld, FrameReader } from '../src/native/client';
import type { Packet } from '../src/native/client';
import { decodeSeasonalMoisture } from '../src/native/seasonal-moisture';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...DEFAULT_RECIPE, subdivision: 2 };

test('precise upslope initialization isolates modes and preserves exact hourly/daily state', async () => {
  const a = new NativeController(executable), b = new NativeController(executable);
  try {
    const x = await a.generate(recipe), y = await b.generate(recipe); a.accept(x.epoch); b.accept(y.epoch);
    await assert.rejects(a.initializePreciseMoisture(x.epoch + 1), /matching/);
    const pending = a.initializePreciseMoisture(x.epoch);
    await assert.rejects(a.initializePreciseMoisture(x.epoch), /busy/);
    const initial = await pending; await b.initializePreciseMoisture(y.epoch);
    assert.equal(initial.modelVersion, 'seasonal-moisture-7');
    await assert.rejects(a.initializeOrographicMoisture(x.epoch), /Regenerate/);
    await assert.rejects(a.prescribeWater(x.epoch, 0, 'oneCubicKilometer'), /separate manual/);
    const daily = await a.seasonalMoisture(x.epoch, 86400); let hourly = initial;
    for (let i = 0; i < 24; i++) hourly = await b.seasonalMoisture(y.epoch, 3600);
    assert.deepEqual(daily.stocks, hourly.stocks); assert.deepEqual(daily.budget, hourly.budget);
    assert.equal(await a.exportSeasonalCheckpoint(x.epoch), await b.exportSeasonalCheckpoint(y.epoch));
    const fresh = await a.generate(recipe); a.accept(fresh.epoch);
    assert.equal((await a.seasonalMoisture(fresh.epoch, 0)).modelVersion, 'seasonal-moisture-3');
    await assert.rejects(a.initializePreciseMoisture(fresh.epoch), /Regenerate/);
  } finally { a.close(); b.close(); }
});

test('schema-seven replay preserves four low components and original numeric tokens transactionally', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    const fresh = await core.generate(recipe); core.accept(fresh.epoch);
    await core.initializePreciseMoisture(fresh.epoch);
    for (let i = 0; i < 31; i++) await core.seasonalMoisture(fresh.epoch, 86400);
    const text = await core.exportSeasonalCheckpoint(fresh.epoch), cp = JSON.parse(text);
    assert.equal(cp.schemaVersion, 7); assert.equal(cp.terminalStockModelVersion, 'terminal-stock-compensated-1');
    for (const key of ['surfaceLowKilograms', 'snowLowKilograms', 'soilLowKilograms', 'terminalLowKilograms']) {
      assert.equal(cp[key].length, 162); assert.ok(cp[key].some((v: number) => v !== 0));
    }
    const loaded = await replay.loadSeasonalCheckpoint(text); replay.accept(loaded.epoch);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), text);
    for (const edit of [
      (value: typeof cp) => { delete value.terminalLowKilograms; },
      (value: typeof cp) => { value.terminalStockModelVersion = 'future'; },
      (value: typeof cp) => { value.surfaceLowKilograms = null; },
      (value: typeof cp) => { value.settings.terminalNumerics = null; },
      (value: typeof cp) => { value.schemaVersion = 6; value.modelVersion = 'seasonal-moisture-6'; },
    ]) {
      const invalid = structuredClone(cp); edit(invalid);
      await assert.rejects(replay.loadSeasonalCheckpoint(JSON.stringify(invalid)));
      assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), text);
    }
    await core.seasonalMoisture(fresh.epoch, 86400); await replay.seasonalMoisture(loaded.epoch, 86400);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), await core.exportSeasonalCheckpoint(fresh.epoch));
    // Edit only one zero token, without JS rewriting any physical decimal values.
    const prefix = '"terminalLowKilograms":[', start = text.indexOf(prefix) + prefix.length, end = text.indexOf(']', start);
    const tokens = text.slice(start, end).split(','), zero = tokens.indexOf('0.0'); assert.ok(zero >= 0);
    tokens[zero] = '-0.0'; const signed = text.slice(0, start) + tokens.join(',') + text.slice(end);
    const negative = await replay.loadSeasonalCheckpoint(signed); replay.accept(negative.epoch);
    assert.equal(await replay.exportSeasonalCheckpoint(negative.epoch), signed);
  } finally { core.close(); replay.close(); }
});

test('protocol thirteen rejects mismatched precision pins and preserves display size bounds', async () => {
  const session = new NativeSession(executable);
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    const packet = await session.request({ command: 'initializePreciseMoisture' });
    assert.equal(packet.bytes.length, 18 * 8 * 162);
    assert.equal(decodeSeasonalMoisture(packet, world, 1, 0, 0, undefined, 'precise').modelVersion, 'seasonal-moisture-7');
    assert.throws(() => decodeSeasonalMoisture(packet, world, 1, 0, 0, undefined, true));
    for (const edit of [
      (p: Packet) => { p.header.protocol = 12; }, (p: Packet) => { p.header.kind = 'orographicMoisture'; },
      (p: Packet) => { p.header.surfaceModelVersion = 'surface-water-1'; },
      (p: Packet) => { delete p.header.terminalStockModelVersion; },
      (p: Packet) => { (p.header.settings as { terminalNumerics: unknown }).terminalNumerics = null; },
      (p: Packet) => { p.bytes.writeDoubleLE(NaN, 0); },
    ]) {
      const corrupt = { header: structuredClone(packet.header), bytes: Buffer.from(packet.bytes) }; edit(corrupt);
      assert.throws(() => decodeSeasonalMoisture(corrupt, world, 1, 0, 0, undefined, 'precise'));
    }
    assert.equal((await session.request({ command: 'exportMoisture' })).header.kind, 'preciseMoistureCheckpoint');
  } finally { session.close(); }
  for (const [kind, protocol, byteLength] of [['preciseMoisture', 12, 0], ['preciseMoistureCheckpoint', 11, 0],
    ['preciseMoisture', 13, 32 * 2 ** 20 + 1]] as const) {
    const header = Buffer.from(JSON.stringify({ kind, protocol, byteLength }));
    const size = Buffer.alloc(4); size.writeUInt32LE(header.length);
    assert.throws(() => new FrameReader(() => assert.fail()).push(Buffer.concat([size, header])), /protocol/);
  }
});

test('largest-grid precise checkpoint retains complete components and short exact continuation', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    const world = await core.generate({ ...recipe, subdivision: 6 }); core.accept(world.epoch);
    await core.initializePreciseMoisture(world.epoch); await core.seasonalMoisture(world.epoch, 3600);
    const text = await core.exportSeasonalCheckpoint(world.epoch);
    const loaded = await replay.loadSeasonalCheckpoint(text); replay.accept(loaded.epoch);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), text);
    await core.seasonalMoisture(world.epoch, 3600); await replay.seasonalMoisture(loaded.epoch, 3600);
    assert.equal(await core.exportSeasonalCheckpoint(world.epoch), await replay.exportSeasonalCheckpoint(loaded.epoch));
    console.log(`Level-6 precise checkpoint: ${Buffer.byteLength(text)} bytes; exact one-hour continuation.`);
  } finally { core.close(); replay.close(); }
});
