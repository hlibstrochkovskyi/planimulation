import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import type { World } from '../src/core/world';
import { FrameReader, NativeController, NativeSession, decodeWorld } from '../src/native/client';
import type { Packet } from '../src/native/client';
import { decodeSoilMoisture } from '../src/native/soil-moisture';
import { SoilMoistureSession } from '../src/native/soil-session';
import { MAX_SEASONAL_CHECKPOINT_BYTES } from '../src/shared/seasonal-checkpoint';
import { SOIL_STOCKS, SOIL_TRANSFERS } from '../src/shared/soil-moisture';
import { regionalFlowRecipe } from './helpers/regional-water';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...DEFAULT_RECIPE, subdivision: 2 };
interface Mass { high: number; low: number }
interface FundingCheckpoint {
  elapsedSeconds: number; referenceBodies: Mass[]; liquid: Mass[];
  localTransfers: Array<{ liquidEvaporation: Mass; rain: Mass }>;
  atmosphericTransfers: Mass[];
}
function credit(m: Mass, amount: number): void {
  const sum = m.high + amount, v = sum - m.high;
  const tail = m.low + ((m.high - (sum - v)) + (amount - v));
  const high = sum + tail, w = high - sum;
  m.high = high; m.low = (sum - (high - w)) + (tail - w);
}
/** Synthetic directed stress input, not naturally reached rainfall. One finite
 * body funds evaporation, a real adjacent atmospheric crossing and land rain. */
function fund(contents: string, world: World): string {
  const cp = JSON.parse(contents) as FundingCheckpoint, faces: number[][] = [];
  for (let a = 0; a < world.stats.regionCount; a++) {
    for (const b of world.surface.neighbors.subarray(world.surface.neighborOffsets[a], world.surface.neighborOffsets[a + 1])) {
      if (a < b) faces.push([a, b]);
    }
  }
  faces.sort(([a, b], [c, d]) => a - c || b - d);
  const face = faces.findIndex(([a, b]) => Number(world.water.bodyIds[a] > 0) !== Number(world.water.bodyIds[b] > 0));
  assert.ok(face >= 0);
  const [a, b] = faces[face], source = world.water.bodyIds[a] > 0 ? a : b, target = source === a ? b : a;
  const ids = [...new Set(world.water.bodyIds)].filter(id => id > 0).sort((a, b) => a - b);
  const body = ids.indexOf(world.water.bodyIds[source]), amount = world.surface.areasSquareMeters[target] * 50000;
  assert.ok(cp.referenceBodies[body].high > amount);
  credit(cp.referenceBodies[body], -amount);
  credit(cp.localTransfers[source].liquidEvaporation, amount);
  credit(cp.atmosphericTransfers[2 * face + Number(source > target)], amount);
  credit(cp.localTransfers[target].rain, amount); credit(cp.liquid[target], amount);
  cp.elapsedSeconds = 900;
  return JSON.stringify(cp);
}

test('unified typed session preserves paired owners, split clocks, signed zero and complete replay', async () => {
  const daily = await SoilMoistureSession.initialize(executable, recipe);
  const hourly = await SoilMoistureSession.initialize(executable, recipe);
  let restored: SoilMoistureSession | undefined;
  try {
    const initial = daily.frame, geography = daily.world;
    assert.equal(initial.modelVersion, 'regional-seasonal-water-1');
    assert.ok(SOIL_STOCKS.every(k => initial.stocks[k].high.every(v => v === 0)));
    assert.equal(initial.budget.initialKilograms, initial.budget.storedKilograms);
    const signed = (await daily.checkpoint()).replace('"low":0.0', '"low":-0.0');
    assert.ok(signed.includes('"low":-0.0'));
    restored = await SoilMoistureSession.restore(executable, signed);
    assert.equal(await restored.checkpoint(), signed);
    const day = await daily.advance(86400);
    for (let h = 0; h < 24; h++) await hourly.advance(3600);
    assert.equal(await daily.checkpoint(), await hourly.checkpoint());
    const saved = await daily.checkpoint(), read = await daily.advance(0);
    assert.deepEqual(read.stocks, day.stocks);
    assert.deepEqual(read.cumulativeLocalTransfers, day.cumulativeLocalTransfers);
    assert.ok(SOIL_TRANSFERS.every(k => read.intervalLocalTransfers[k].every(v => v === 0)));
    assert.equal(await daily.checkpoint(), saved);
    restored.close(); restored = await SoilMoistureSession.restore(executable, saved);
    assert.deepEqual(restored.frame.stocks, day.stocks);
    await daily.advance(3600); await restored.advance(3600);
    assert.equal(await daily.checkpoint(), await restored.checkpoint());
    assert.deepEqual(daily.world, geography);
    const exposed = daily.frame; exposed.stocks.vapor.high.fill(999); exposed.budget.storedKilograms = 0;
    const exposedWorld = daily.world; exposedWorld.terrain.elevation.fill(0);
    await daily.advance(0); assert.deepEqual(daily.world, geography);
    for (const seconds of [-1, 0.1, 86401, NaN]) await assert.rejects(daily.advance(seconds), /Invalid/);
    const pending = daily.advance(900);
    await assert.rejects(daily.advance(900), /busy/); await pending;
  } finally { daily.close(); hourly.close(); restored?.close(); }
});

test('funded unified owners drive real land flow, active soil and exact daily/hourly continuation', async () => {
  const initial = await SoilMoistureSession.initialize(executable, await regionalFlowRecipe());
  let direct: SoilMoistureSession | undefined, split: SoilMoistureSession | undefined;
  try {
    const world = initial.world, fixture = fund(await initial.checkpoint(), world);
    direct = await SoilMoistureSession.restore(executable, fixture);
    split = await SoilMoistureSession.restore(executable, fixture);
    const before = direct.frame, day = await direct.advance(86400);
    for (let h = 0; h < 24; h++) await split.advance(3600);
    assert.equal(await direct.checkpoint(), await split.checkpoint());
    assert.ok(day.stocks.soil.high.some(v => v > 0));
    assert.ok(day.cumulativeLocalTransfers.infiltration.high.some(v => v > 0));
    assert.ok(day.cumulativeLocalTransfers.soilDrainage.high.some(v => v > 0));
    assert.ok(day.cumulativeSurfaceOutgoingKilograms.some(v => v > 0));
    assert.ok(day.stocks.vapor.low.some(v => v !== 0));
    assert.ok(day.visibleWaterDepthMeters.some((v, r) => world.water.bodyIds[r] === 0
      && v > 0 && before.visibleWaterDepthMeters[r] === 0));
    assert.ok(day.budget.relativeGlobalResidual < 1e-12);
    assert.ok(day.budget.maximumRelativeLocalResidual <= 128 * Number.EPSILON);
    const saved = await direct.checkpoint(); split.close(); split = await SoilMoistureSession.restore(executable, saved);
    await direct.advance(3600); await split.advance(3600);
    assert.equal(await direct.checkpoint(), await split.checkpoint());
  } finally { initial.close(); direct?.close(); split?.close(); }
});

test('protocol 15 rejects corrupted metadata, ownership, precision, depth and observation flows', async () => {
  const raw = new NativeSession(executable);
  try {
    const world = decodeWorld(await raw.request({ command: 'generate', recipe }));
    const packet = await raw.request({ command: 'initializeSoilMoisture' });
    const initial = decodeSoilMoisture(packet, world, 1, 0, 0), n = world.stats.regionCount;
    const wet = world.water.bodyIds.findIndex(id => id > 0), land = world.water.bodyIds.findIndex(id => id === 0);
    const corrupt = (change: (p: Packet) => void) => {
      const p = { header: structuredClone(packet.header), bytes: Buffer.from(packet.bytes) };
      change(p); assert.throws(() => decodeSoilMoisture(p, world, 1, 0, 0));
    };
    for (const key of ['protocol', 'modelVersion', 'soilModelVersion', 'transportModelVersion', 'surfaceFlowModelVersion',
      'runoffModelVersion', 'observationModelVersion', 'temperatureModelVersion', 'windModelVersion', 'referenceBodyModelVersion',
      'orographicModelVersion', 'regionCount', 'referenceBodyCount', 'byteLength', 'elapsedSeconds', 'coupledSubsteps']) {
      corrupt(p => { p.header[key] = -1; });
    }
    corrupt(p => { (p.header.settings as { numericalPolicy: string }).numericalPolicy = 'rejectUnrepresentable'; });
    corrupt(p => { p.bytes = p.bytes.subarray(8); });
    corrupt(p => { p.bytes.writeDoubleLE(NaN, 0); });
    corrupt(p => { p.bytes.writeDoubleLE(1, n * 8); }); // Nonzero low of an empty owner.
    corrupt(p => { p.bytes.writeDoubleLE(1e20, (2 * n + land) * 8); }); // Soil above capacity.
    corrupt(p => { p.bytes.writeDoubleLE(1e12, wet * 8); }); // Duplicated body-owned water.
    corrupt(p => { p.bytes.writeDoubleLE(1, (31 * n + land) * 8); });
    corrupt(p => { p.bytes.writeDoubleLE(1, (24 * n + land) * 8); }); // Observation cannot report interval rain.
    corrupt(p => { p.bytes.writeDoubleLE(1, (36 * n + land) * 8); });
    corrupt(p => { (p.header.budget as { storedKilograms: number }).storedKilograms *= 2; });
    corrupt(p => { (p.header.resolution as { deferredRequests: number }).deferredRequests = 1; });
    const saved = await raw.request({ command: 'exportMoisture' });
    const dayPacket = await raw.request({ command: 'seasonalMoisture', seconds: 86400 });
    const day = decodeSoilMoisture(dayPacket, world, 1, 86400, 86400, initial);
    const read = await raw.request({ command: 'seasonalMoisture', seconds: 0 });
    decodeSoilMoisture(read, world, 1, 86400, 0, day);
    const changed = { header: structuredClone(read.header), bytes: Buffer.from(read.bytes) };
    // Budget is unchanged but a fabricated cumulative local history is not an observation.
    changed.bytes.writeDoubleLE(1, (20 * n + land) * 8);
    assert.throws(() => decodeSoilMoisture(changed, world, 1, 86400, 0, day));
    assert.ok(saved.bytes.length > 0);
  } finally { raw.close(); }
});

test('selected seasonal families and manual water cannot coexist; failed restores preserve active sessions', async () => {
  const active = await SoilMoistureSession.initialize(executable, recipe), raw = new NativeSession(executable);
  const legacy = new NativeController(executable);
  try {
    const saved = await active.checkpoint(), cp = JSON.parse(saved);
    for (const change of [(c: typeof cp) => { c.settings.numericalPolicy = 'rejectUnrepresentable'; },
      (c: typeof cp) => { c.settings.maxCoupledStepSeconds = 1800; },
      (c: typeof cp) => { c.schemaVersion = 15; },
      (c: typeof cp) => { c.atmosphericTransfers.pop(); },
      (c: typeof cp) => { c.resolution = null; }]) {
      const invalid = structuredClone(cp); change(invalid);
      await assert.rejects(SoilMoistureSession.restore(executable, JSON.stringify(invalid)));
      assert.equal(await active.checkpoint(), saved);
    }
    const prepared = await legacy.generate(recipe); legacy.accept(prepared.epoch);
    await legacy.initializeRegionalMoisture(prepared.epoch);
    const old = await legacy.exportSeasonalCheckpoint(prepared.epoch);
    await assert.rejects(SoilMoistureSession.restore(executable, old), /Unsupported/);
    await assert.rejects(legacy.loadSeasonalCheckpoint(saved), /Unsupported/);
    assert.equal(await legacy.exportSeasonalCheckpoint(prepared.epoch), old);
    await raw.request({ command: 'generate', recipe });
    await assert.rejects(raw.request({ command: 'restoreSoilMoisture', checkpointJson: old }));
    await raw.request({ command: 'initializeSoilMoisture' });
    for (const command of [{ command: 'initializeRegionalMoisture' }, { command: 'initializePreciseMoisture' },
      { command: 'restoreMoisture', checkpointJson: old }, { command: 'restoreSoilMoisture', checkpointJson: saved },
      { command: 'prescribeWater', region: 0, mode: 'oneCubicKilometer' }]) await assert.rejects(raw.request(command));
    assert.equal((await raw.request({ command: 'exportMoisture' })).bytes.toString('utf8') + '\n', saved);
    await raw.request({ command: 'generate', recipe });
    await raw.request({ command: 'initializeRegionalMoisture' });
    await assert.rejects(raw.request({ command: 'initializeSoilMoisture' }));
    await raw.request({ command: 'generate', recipe });
    await raw.request({ command: 'prescribeWater', region: 0, mode: 'oneCubicKilometer' });
    await assert.rejects(raw.request({ command: 'initializeSoilMoisture' }));
  } finally { active.close(); raw.close(); legacy.close(); }
});

test('protocol 15 framing admits only its two kinds and retains finite display/checkpoint bounds', () => {
  const header = (kind: string, protocol: number, byteLength: number) => {
    const h = Buffer.from(JSON.stringify({ kind, protocol, byteLength })), prefix = Buffer.alloc(4);
    prefix.writeUInt32LE(h.length); return Buffer.concat([prefix, h]);
  };
  for (const kind of ['soilMoisture', 'soilMoistureCheckpoint']) {
    const received: Packet[] = [], reader = new FrameReader(p => received.push(p));
    for (const byte of header(kind, 15, 0)) reader.push(Buffer.from([byte]));
    assert.equal(received.length, 1);
    assert.throws(() => new FrameReader(() => {}).push(header(kind, 14, 0)));
  }
  for (const kind of ['world', 'regionalMoisture', 'moisture', 'error', 'checkpoint']) {
    assert.throws(() => new FrameReader(() => {}).push(header(kind, 15, 0)));
  }
  assert.throws(() => new FrameReader(() => {}).push(header('soilMoisture', 15, 32 * 2 ** 20 + 1)));
  assert.throws(() => new FrameReader(() => {}).push(header('soilMoistureCheckpoint', 15, MAX_SEASONAL_CHECKPOINT_BYTES + 1)));
});

test('unified transport supports empty reference-body and all-reference ownership partitions', async () => {
  for (const fraction of [0, 1]) {
    const core = await SoilMoistureSession.initialize(executable, { ...recipe, water: { mode: 'coverage', fraction } });
    let replay: SoilMoistureSession | undefined;
    try {
      assert.equal(core.frame.referenceBodyIds.length, fraction);
      const frame = await core.advance(86400);
      if (!fraction) {
        assert.equal(frame.budget.initialKilograms, 0);
        assert.equal(frame.budget.storedKilograms, 0);
        assert.ok(SOIL_STOCKS.every(k => frame.stocks[k].high.every(v => v === 0)));
      } else {
        assert.ok(frame.stocks.liquid.high.every(v => v === 0));
        assert.ok(frame.stocks.soil.high.every(v => v === 0));
        assert.ok(frame.stocks.drainage.high.every(v => v === 0));
      }
      const saved = await core.checkpoint();
      replay = await SoilMoistureSession.restore(executable, saved);
      assert.equal(await replay.checkpoint(), saved);
      await core.advance(900); await replay.advance(900);
      assert.equal(await replay.checkpoint(), await core.checkpoint());
    } finally { core.close(); replay?.close(); }
  }
});

test('dense numerical donor retention survives frames, read-only observation and checkpoint replay', async () => {
  const core = await SoilMoistureSession.initialize(executable, { ...await regionalFlowRecipe(), subdivision: 3, seed: 'first-light' });
  let replay: SoilMoistureSession | undefined;
  try {
    const day = await core.advance(86400);
    assert.ok(day.resolution.deferredRequests > 0);
    assert.ok(day.resolution.maximumDeferredRequestKilograms > 0);
    const saved = await core.checkpoint(), read = await core.advance(0);
    assert.deepEqual(read.resolution, day.resolution);
    assert.equal(await core.checkpoint(), saved);
    replay = await SoilMoistureSession.restore(executable, saved);
    assert.deepEqual(replay.frame.resolution, day.resolution);
    await core.advance(3600); await replay.advance(3600);
    assert.equal(await core.checkpoint(), await replay.checkpoint());
  } finally { core.close(); replay?.close(); }
});

test('finest supported unified frame and complete checkpoint fit bounds and restore exactly', async t => {
  const core = await SoilMoistureSession.initialize(executable, { ...recipe, subdivision: 6 });
  let replay: SoilMoistureSession | undefined;
  try {
    assert.equal(core.world.stats.regionCount, 40962);
    const frame = await core.advance(3600), saved = await core.checkpoint();
    t.diagnostic(`Finest-grid checkpoint bytes including newline: ${Buffer.byteLength(saved)}; display body bytes: ${(37 * 40962 + 2 * frame.referenceBodyIds.length) * 8}.`);
    assert.ok(Buffer.byteLength(saved) < MAX_SEASONAL_CHECKPOINT_BYTES);
    replay = await SoilMoistureSession.restore(executable, saved);
    assert.equal(await replay.checkpoint(), saved);
    assert.deepEqual(replay.frame.stocks, frame.stocks);
    await core.advance(900); await replay.advance(900);
    assert.equal(await replay.checkpoint(), await core.checkpoint());
    assert.ok((37 * 40962 + 2 * frame.referenceBodyIds.length) * 8 < 32 * 2 ** 20);
  } finally { core.close(); replay?.close(); }
});
