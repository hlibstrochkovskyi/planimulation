import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { NativeController, NativeSession, decodeWorld } from '../src/native/client';
import type { Packet } from '../src/native/client';
import { decodeRegionalMoisture } from '../src/native/regional-moisture';
import { regionalWaterDisplay } from '../src/renderer/regional-water';
import { moistureLayerValue } from '../src/renderer/seasonal-moisture';
import { buildViewGeometry } from '../src/renderer/view-geometry';
import { buildWaterSurface } from '../src/renderer/water-surface';
import { MOISTURE_STOCK_FIELDS } from '../src/shared/seasonal-moisture';
import { MAX_SEASONAL_CHECKPOINT_BYTES } from '../src/shared/seasonal-checkpoint';
import { fundedRegionalCheckpoint, regionalFlowRecipe } from './helpers/regional-water';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...DEFAULT_RECIPE, subdivision: 2 };

test('regional desktop initialization, split clocks and complete checkpoint replay preserve ownership', async () => {
  const daily = new NativeController(executable), hourly = new NativeController(executable), replay = new NativeController(executable);
  try {
    const a = await daily.generate(recipe), b = await hourly.generate(recipe);
    daily.accept(a.epoch); hourly.accept(b.epoch);
    const geography = structuredClone(a.world);
    const initial = await daily.initializeRegionalMoisture(a.epoch);
    await hourly.initializeRegionalMoisture(b.epoch);
    assert.equal(initial.modelVersion, 'seasonal-moisture-15');
    assert.equal(initial.budget.referenceBodyWaterKilograms, initial.budget.initialMobileWaterKilograms);
    assert.ok(MOISTURE_STOCK_FIELDS.every((key) => initial.stocks[key].every((v) => v === 0)));
    const signed = (await daily.exportSeasonalCheckpoint(a.epoch))
      .replace('"cumulativeSurfaceTransferRoundoff":[[0.0,', '"cumulativeSurfaceTransferRoundoff":[[-0.0,');
    assert.ok(signed.includes('[[-0.0,'));
    const signedCandidate = await replay.loadSeasonalCheckpoint(signed); replay.accept(signedCandidate.epoch);
    assert.equal(await replay.exportSeasonalCheckpoint(signedCandidate.epoch), signed);
    const day = await daily.seasonalMoisture(a.epoch, 86400);
    for (let i = 0; i < 24; i++) await hourly.seasonalMoisture(b.epoch, 3600);
    assert.equal(await hourly.exportSeasonalCheckpoint(b.epoch), await daily.exportSeasonalCheckpoint(a.epoch));
    const read = await daily.seasonalMoisture(a.epoch, 0);
    assert.deepEqual(read.stocks, day.stocks);
    assert.deepEqual(read.budget, day.budget);
    assert.equal(read.regionalSurface!.cumulativeTransferredKilograms, day.regionalSurface!.cumulativeTransferredKilograms);
    assert.ok(Object.values(read.regionalSurface!.flow).every((v) => v === 0));
    const saved = await daily.exportSeasonalCheckpoint(a.epoch);
    const loaded = await replay.loadSeasonalCheckpoint(saved); replay.accept(loaded.epoch);
    assert.deepEqual(loaded.moistureFrame.regionalSurface, read.regionalSurface);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), saved);
    await daily.seasonalMoisture(a.epoch, 3600); await replay.seasonalMoisture(loaded.epoch, 3600);
    assert.equal(await replay.exportSeasonalCheckpoint(loaded.epoch), await daily.exportSeasonalCheckpoint(a.epoch));
    assert.deepEqual(a.world, geography);
    assert.deepEqual(daily.resolvedInitialWorld(a.epoch), geography);
    await assert.rejects(daily.initializePreciseMoisture(a.epoch));
    await assert.rejects(daily.prescribeWater(a.epoch, 0, 'oneCubicKilometer'), /separate manual/);
    const cp = JSON.parse(saved); cp.settings.maxCoupledStepSeconds = 1800;
    const before = await daily.exportSeasonalCheckpoint(a.epoch);
    await assert.rejects(daily.loadSeasonalCheckpoint(JSON.stringify(cp)));
    assert.equal(await daily.exportSeasonalCheckpoint(a.epoch), before);
    const candidate = await daily.loadSeasonalCheckpoint(saved);
    daily.cancel(); assert.throws(() => daily.accept(candidate.epoch));
    assert.equal(await daily.exportSeasonalCheckpoint(a.epoch), before);
  } finally { daily.close(); hourly.close(); replay.close(); }
});

test('funded neighboring columns produce actual desktop face flow and independent display caps', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    const prepared = await core.generate(await regionalFlowRecipe()); core.accept(prepared.epoch);
    await core.initializeRegionalMoisture(prepared.epoch);
    const fixture = fundedRegionalCheckpoint(await core.exportSeasonalCheckpoint(prepared.epoch), prepared.world);
    const loaded = await core.loadSeasonalCheckpoint(fixture); core.accept(loaded.epoch);
    const initial = loaded.moistureFrame, initialDisplay = regionalWaterDisplay(loaded.world, initial);
    const geography = structuredClone(loaded.world), initialFrame = structuredClone(initial);
    const frame = await core.seasonalMoisture(loaded.epoch, 900), regional = frame.regionalSurface!;
    assert.ok(regional.flow.transferredKilograms > 0);
    // The checkpointed compensated history and the interval diagnostic use
    // different summation paths; their rounded totals need not be bit-identical.
    assert.ok(Math.abs(regional.cumulativeTransferredKilograms - regional.flow.transferredKilograms)
      <= 2 * Number.EPSILON * regional.cumulativeTransferredKilograms);
    assert.ok(frame.stocks.terminalWaterKilograms.some((v, i) => v > 0 && i !== 8 && i !== 156));
    assert.ok(Math.abs(frame.budget.residualKilograms) / frame.budget.initialMobileWaterKilograms < 1e-12);
    const display = regionalWaterDisplay(loaded.world, frame);
    assert.ok(display.wetMask.reduce((s, v) => s + v, 0) > initialDisplay.wetMask.reduce((s, v) => s + v, 0));
    for (let i = 0; i < display.wetMask.length; i++) {
      const reference = loaded.world.water.bodyIds[i] !== 0;
      assert.equal(display.wetMask[i], Number(reference || frame.stocks.terminalWaterKilograms[i] > 0));
      if (reference) {
        assert.equal(display.surfaceLevelsMeters[i], loaded.world.water.levelMeters);
        assert.equal(display.depthMeters[i], loaded.world.water.depthMeters[i]);
        assert.equal(regional.cumulativeOutgoingKilograms[i], 0);
      }
      const area = loaded.world.surface.areasSquareMeters[i];
      assert.equal(moistureLayerValue(frame, 'regionalDepth', i, area), regional.depthMeters[i]);
      assert.equal(moistureLayerValue(frame, 'surfaceInflow', i, area), regional.cumulativeIncomingKilograms[i] / 1e12);
      assert.equal(moistureLayerValue(frame, 'surfaceOutflow', i, area), regional.cumulativeOutgoingKilograms[i] / 1e12);
    }
    const bed = buildViewGeometry(loaded.world.surface, undefined, loaded.world.terrain.elevation).globe;
    const before = structuredClone(bed);
    const caps = buildWaterSurface(bed.positions, bed.regions, display, loaded.world.recipe.radiusMeters);
    assert.deepEqual(new Set(caps.waterRegions), new Set([...display.wetMask].flatMap((v, i) => v ? [i] : [])));
    for (let i = 0; i < caps.waterRegions.length; i++) {
      const expected = display.surfaceLevelsMeters[caps.waterRegions[i]] / loaded.world.recipe.radiusMeters;
      assert.ok(Math.abs(caps.waterOffsets[i] - expected) <= Math.max(Math.abs(expected), 1) * 1e-7);
    }
    assert.deepEqual(bed, before);
    assert.deepEqual(initial, initialFrame);
    assert.deepEqual(loaded.world, geography);
    const saved = await core.exportSeasonalCheckpoint(loaded.epoch);
    const restored = await replay.loadSeasonalCheckpoint(saved); replay.accept(restored.epoch);
    await core.seasonalMoisture(loaded.epoch, 3600); await replay.seasonalMoisture(restored.epoch, 3600);
    assert.equal(await replay.exportSeasonalCheckpoint(restored.epoch), await core.exportSeasonalCheckpoint(loaded.epoch));
  } finally { core.close(); replay.close(); }
});

test('protocol 14 rejects wrong pins, body ownership, field shapes, flow diagnostics and depth', async () => {
  const raw = new NativeSession(executable);
  try {
    const world = decodeWorld(await raw.request({ command: 'generate', recipe }));
    const packet = await raw.request({ command: 'initializeRegionalMoisture' });
    const initial = decodeRegionalMoisture(packet, world, 1, 0, 0);
    assert.ok(initial.regionalSurface);
    const corrupt = (change: (p: Packet) => void) => {
      const altered = { header: structuredClone(packet.header), bytes: Buffer.from(packet.bytes) };
      change(altered); assert.throws(() => decodeRegionalMoisture(altered, world, 1, 0, 0));
    };
    for (const key of ['protocol', 'referenceBodyCount', 'regionCount', 'byteLength']) corrupt((p) => { p.header[key] = 0; });
    for (const key of ['modelVersion', 'referenceBodyModelVersion', 'closedLakeModelVersion',
      'regionalSurfaceObservationVersion', 'regionalTransferObservationVersion']) corrupt((p) => { p.header[key] = 'future'; });
    corrupt((p) => { (p.header.settings as Record<string, unknown>).initialActiveSurfaceDepthMeters = 1; });
    corrupt((p) => { p.header.explicitStabilityBoundSeconds = Infinity; });
    corrupt((p) => { (p.header.regionalSurfaceFlow as Record<string, unknown>).transferredKilograms = 1; });
    corrupt((p) => { p.header.cumulativeFaceTransferKilograms = 1e20; });
    const n = world.stats.regionCount, bodies = initial.regionalSurface!.referenceBodyIds.length;
    corrupt((p) => { p.bytes.writeDoubleLE(NaN, 18 * n * 8); });
    corrupt((p) => { p.bytes.writeDoubleLE(1, 19 * n * 8); });
    corrupt((p) => { p.bytes.writeDoubleLE(1e30, (22 * n + bodies) * 8); });
    corrupt((p) => { p.bytes = p.bytes.subarray(0, p.bytes.length - 8); p.header.byteLength = p.bytes.length; });
    const day = await raw.request({ command: 'seasonalMoisture', seconds: 86400 });
    decodeRegionalMoisture(day, world, 1, 86400, 86400, initial.budget, 0);
    assert.throws(() => decodeRegionalMoisture(day, world, 1, 86400, 86400, initial.budget, 1e25));
  } finally { raw.close(); }
});

test('regional ownership handles zero/all-water worlds and largest-grid checkpoint continuation', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    for (const fraction of [0, 1]) {
      const generated = await core.generate({ ...recipe, water: { mode: 'coverage', fraction } }); core.accept(generated.epoch);
      const initial = await core.initializeRegionalMoisture(generated.epoch);
      assert.equal(initial.regionalSurface!.referenceBodyIds.length, fraction);
      const frame = await core.seasonalMoisture(generated.epoch, 3600);
      assert.equal(frame.regionalSurface!.cumulativeTransferredKilograms, 0);
      if (!fraction) assert.equal(frame.budget.initialMobileWaterKilograms, 0);
      assert.deepEqual(generated.world, core.resolvedInitialWorld(generated.epoch));
    }
    const generated = await core.generate({ ...DEFAULT_RECIPE, subdivision: 6 }); core.accept(generated.epoch);
    await core.initializeRegionalMoisture(generated.epoch);
    await core.seasonalMoisture(generated.epoch, 3600);
    const saved = await core.exportSeasonalCheckpoint(generated.epoch), bytes = Buffer.byteLength(saved);
    assert.ok(bytes > 8 * 2 ** 20 && bytes <= MAX_SEASONAL_CHECKPOINT_BYTES);
    const candidate = await replay.loadSeasonalCheckpoint(saved); replay.accept(candidate.epoch);
    assert.equal(candidate.world.stats.regionCount, 40962);
    assert.equal(await replay.exportSeasonalCheckpoint(candidate.epoch), saved);
    await core.seasonalMoisture(generated.epoch, 3600); await replay.seasonalMoisture(candidate.epoch, 3600);
    assert.equal(await replay.exportSeasonalCheckpoint(candidate.epoch), await core.exportSeasonalCheckpoint(generated.epoch));
    console.log(`Level-6 one-hour regional checkpoint: ${bytes} bytes; exact one-hour continuation.`);
  } finally { core.close(); replay.close(); }
});

test('active regional display ledgers continue across a year boundary with complete replay', async () => {
  const core = new NativeController(executable), replay = new NativeController(executable);
  try {
    const generated = await core.generate(await regionalFlowRecipe()); core.accept(generated.epoch);
    await core.initializeRegionalMoisture(generated.epoch);
    const fixture = fundedRegionalCheckpoint(await core.exportSeasonalCheckpoint(generated.epoch), generated.world);
    const candidate = await core.loadSeasonalCheckpoint(fixture); core.accept(candidate.epoch);
    let previous = 0;
    for (let day = 1; day <= 365; day++) {
      const frame = await core.seasonalMoisture(candidate.epoch, 86400);
      assert.equal(frame.elapsedSeconds, 900 + day * 86400);
      assert.ok(frame.regionalSurface!.cumulativeTransferredKilograms >= previous);
      previous = frame.regionalSurface!.cumulativeTransferredKilograms;
      assert.ok(Math.abs(frame.budget.residualKilograms) / frame.budget.initialMobileWaterKilograms < 1e-12);
    }
    assert.ok(previous > 0);
    const saved = await core.exportSeasonalCheckpoint(candidate.epoch), cp = JSON.parse(saved);
    assert.ok(cp.regionalSurfaceFlow.directedTransfers.lowKilograms.some((v: number) => v !== 0));
    const restored = await replay.loadSeasonalCheckpoint(saved); replay.accept(restored.epoch);
    await core.seasonalMoisture(candidate.epoch, 86400); await replay.seasonalMoisture(restored.epoch, 86400);
    assert.equal(await replay.exportSeasonalCheckpoint(restored.epoch), await core.exportSeasonalCheckpoint(candidate.epoch));
  } finally { core.close(); replay.close(); }
});
