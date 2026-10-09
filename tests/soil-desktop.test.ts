import { strict as assert } from 'node:assert';
import path from 'node:path';
import { test } from 'node:test';
import { DEFAULT_RECIPE } from '../src/core/recipe';
import { NativeController } from '../src/native/client';
import { SoilMoistureSession } from '../src/native/soil-session';
import { regionalWaterDisplay } from '../src/renderer/regional-water';
import { moistureLayerValue, moistureLayerTitle, supportsMoistureLayer } from '../src/renderer/seasonal-moisture';
import { soilWaterInspection } from '../src/renderer/soil-water-inspector';
import { buildViewGeometry } from '../src/renderer/view-geometry';
import { buildWaterSurface } from '../src/renderer/water-surface';
import { SOIL_STOCKS } from '../src/shared/soil-moisture';
import { regionalFlowRecipe } from './helpers/regional-water';
import { fundedSoilCheckpoint } from './helpers/soil-water';

const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const recipe = { ...DEFAULT_RECIPE, subdivision: 2 };

test('desktop soil selection, checkpoint candidates and exact continuation preserve old-world ownership', async () => {
  const core = new NativeController(executable), savedCore = new NativeController(executable);
  let reference: SoilMoistureSession | undefined;
  try {
    const generated = await core.generate(recipe); core.accept(generated.epoch);
    await assert.rejects(core.seasonalSoilMoisture(generated.epoch, 0));
    await core.initializeSoilMoisture(generated.epoch);
    const day = await core.seasonalSoilMoisture(generated.epoch, 86400);
    const saved = await core.exportSeasonalCheckpoint(generated.epoch);
    reference = await SoilMoistureSession.restore(executable, saved);
    await assert.rejects(core.seasonalMoisture(generated.epoch, 0), /separate typed/);
    await assert.rejects(core.initializeRegionalMoisture(generated.epoch));
    await assert.rejects(core.initializeSoilMoisture(generated.epoch));
    await assert.rejects(core.prescribeWater(generated.epoch, 0, 'oneCubicKilometer'));
    const oldWorld = core.resolvedInitialWorld(generated.epoch);
    const pending = core.seasonalSoilMoisture(generated.epoch, 3600);
    const pendingCandidate = core.loadSoilCheckpoint(saved), refusal = assert.rejects(pendingCandidate, /canceled/);
    core.cancel(); await refusal;
    const finished = await pending; await reference.advance(3600);
    assert.equal(await core.exportSeasonalCheckpoint(generated.epoch), await reference.checkpoint());
    assert.equal(core.resolvedInitialWorld(generated.epoch), oldWorld);
    const canceled = await core.loadSoilCheckpoint(saved); core.cancel(); assert.throws(() => core.accept(canceled.epoch));
    assert.equal(await core.exportSeasonalCheckpoint(generated.epoch), await reference.checkpoint());
    const corrupt = JSON.parse(saved); corrupt.soil[0].high = -1;
    await assert.rejects(core.loadSoilCheckpoint(JSON.stringify(corrupt)));
    assert.equal(await core.exportSeasonalCheckpoint(generated.epoch), await reference.checkpoint());
    const loaded = await savedCore.loadSoilCheckpoint(saved);
    loaded.soilMoistureFrame.stocks.vapor.high.fill(999); // Candidate baseline is private.
    savedCore.accept(loaded.epoch);
    assert.deepEqual((await savedCore.seasonalSoilMoisture(loaded.epoch, 0)).stocks, day.stocks);
    await savedCore.seasonalSoilMoisture(loaded.epoch, 3600);
    assert.equal(await savedCore.exportSeasonalCheckpoint(loaded.epoch), await reference.checkpoint());
    finished.stocks.vapor.high.fill(999); // Accepted baseline is private too.
    await core.seasonalSoilMoisture(generated.epoch, 0);
    const signed = saved.replace('"low":0.0', '"low":-0.0');
    const candidate = await core.loadSoilCheckpoint(signed); core.accept(candidate.epoch);
    assert.equal(await core.exportSeasonalCheckpoint(candidate.epoch), signed);
    await assert.rejects(core.exportSeasonalCheckpoint(generated.epoch), /No matching/);
    const fresh = await core.generate(recipe); core.accept(fresh.epoch);
    await core.initializeRegionalMoisture(fresh.epoch);
    await assert.rejects(core.initializeSoilMoisture(fresh.epoch));
  } finally { core.close(); savedCore.close(); reference?.close(); }
});

test('unified layer values, inspectors and both projection caps use real five-owner fields without mutation', async () => {
  const core = new NativeController(executable);
  try {
    const generated = await core.generate(await regionalFlowRecipe()); core.accept(generated.epoch);
    await core.initializeSoilMoisture(generated.epoch);
    const fixture = fundedSoilCheckpoint(await core.exportSeasonalCheckpoint(generated.epoch), generated.world);
    const loaded = await core.loadSoilCheckpoint(fixture); core.accept(loaded.epoch);
    const frame = await core.seasonalSoilMoisture(loaded.epoch, 3600), world = loaded.world;
    const originalFrame = structuredClone(frame), originalWorld = structuredClone(world);
    const display = regionalWaterDisplay(world, frame);
    const stockLayers = { liquidWater: 'liquid', snowWater: 'snow', soilWater: 'soil', runoffWater: 'drainage', vaporWater: 'vapor' } as const;
    assert.equal(supportsMoistureLayer(frame, 'terminalWater'), false);
    assert.throws(() => moistureLayerValue(frame, 'terminalWater', 0, world.surface.areasSquareMeters[0]));
    assert.match(moistureLayerTitle(frame, 'runoffWater'), /not surface runoff/);
    for (let r = 0; r < world.stats.regionCount; r++) {
      const area = world.surface.areasSquareMeters[r];
      for (const [layer, stock] of Object.entries(stockLayers) as Array<[keyof typeof stockLayers, typeof stockLayers[keyof typeof stockLayers]]>) {
        assert.equal(moistureLayerValue(frame, layer, r, area), frame.stocks[stock].high[r] / area);
      }
      assert.equal(moistureLayerValue(frame, 'regionalDepth', r, area), frame.regionalLiquidDepthMeters[r]);
      assert.equal(moistureLayerValue(frame, 'surfaceInflow', r, area), frame.cumulativeSurfaceIncomingKilograms[r] / 1e12);
      assert.equal(moistureLayerValue(frame, 'surfaceOutflow', r, area), frame.cumulativeSurfaceOutgoingKilograms[r] / 1e12);
      assert.equal(moistureLayerValue(frame, 'precipitation', r, area),
        (frame.intervalLocalTransfers.rain[r] + frame.intervalLocalTransfers.snowfall[r]) / area * 86400 / frame.intervalSeconds);
      assert.equal(moistureLayerValue(frame, 'runoffFlow', r, area), frame.intervalDrainageSentKilograms[r] / 1000 / frame.intervalSeconds);
      assert.equal(display.wetMask[r], Number(world.water.bodyIds[r] > 0 || frame.visibleWaterDepthMeters[r] > 0));
      assert.equal(display.depthMeters[r], frame.visibleWaterDepthMeters[r]);
    }
    const land = world.water.bodyIds.findIndex((id, r) => id === 0 && frame.stocks.soil.high[r] > 0);
    assert.ok(land >= 0);
    const inspection = soilWaterInspection(world, frame, land);
    assert.match(inspection.budgetNote, /counted once per body/);
    assert.equal(inspection.budgetRows.filter(([label]) => ['Unified terrestrial liquid', 'Terrestrial soil water',
      'Snow water equivalent', 'Atmospheric vapor', 'Delayed soil drainage'].includes(label)).length, SOIL_STOCKS.length);
    assert.ok(inspection.selectionRows.some(([label, value]) => label === 'Terrestrial soil water · exact high / signed low'
      && value === `${frame.stocks.soil.high[land]} kg / ${frame.stocks.soil.low[land]} kg`));
    assert.ok(inspection.budgetRows.some(([label]) => label.includes('Deferred numerical requests')));
    const signed = structuredClone(frame); signed.stocks.soil.low[land] = -0;
    assert.ok(soilWaterInspection(world, signed, land).selectionRows.some(([label, value]) =>
      label === 'Terrestrial soil water · exact high / signed low' && value.endsWith('/ -0 kg')));
    assert.ok(inspection.selectionRows.every(([label]) => !label.includes('Generated liquid runoff') && !label.includes('Terminal evaporation')));
    const wet = world.water.bodyIds.findIndex(id => id > 0), bodyInspection = soilWaterInspection(world, frame, wet);
    assert.ok(bodyInspection.selectionRows.some(([label, value]) => label === 'Reference body owner' && value.includes('not allocated')));
    const geometry = buildViewGeometry(world.surface, undefined, world.terrain.elevation);
    const bed = structuredClone(geometry.globe);
    const caps = buildWaterSurface(geometry.globe.positions, geometry.globe.regions, display, world.recipe.radiusMeters);
    assert.deepEqual(new Set(caps.waterRegions), new Set([...display.wetMask].flatMap((v, r) => v ? [r] : [])));
    assert.deepEqual(geometry.globe, bed);
    assert.deepEqual(frame, originalFrame); assert.deepEqual(world, originalWorld);
    const read = await core.seasonalSoilMoisture(loaded.epoch, 0);
    assert.equal(moistureLayerValue(read, 'precipitation', land, world.surface.areasSquareMeters[land]), 0);
    assert.equal(moistureLayerValue(read, 'runoffFlow', land, world.surface.areasSquareMeters[land]), 0);
  } finally { core.close(); }
});

test('dense desktop diagnostics remain visible and exact after candidate replacement', async () => {
  const core = new NativeController(executable);
  try {
    const generated = await core.generate({ ...await regionalFlowRecipe(), subdivision: 3, seed: 'first-light' }); core.accept(generated.epoch);
    await core.initializeSoilMoisture(generated.epoch);
    const day = await core.seasonalSoilMoisture(generated.epoch, 86400);
    assert.ok(day.resolution.deferredRequests > 0);
    const saved = await core.exportSeasonalCheckpoint(generated.epoch), candidate = await core.loadSoilCheckpoint(saved);
    core.accept(candidate.epoch);
    const read = await core.seasonalSoilMoisture(candidate.epoch, 0);
    assert.deepEqual(read.resolution, day.resolution);
    assert.equal(await core.exportSeasonalCheckpoint(candidate.epoch), saved);
    assert.match(soilWaterInspection(candidate.world, read, null).budgetNote, /unresolved transfers remain at their donors/);
  } finally { core.close(); }
});
