import type { World } from '../core/world';
import { DEFAULT_TEMPERATURE_SETTINGS, TEMPERATURE_MODEL_VERSION } from '../core/seasonal-temperature';
import { DEFAULT_WIND_SETTINGS, WIND_MODEL_VERSION } from '../core/seasonal-wind';
import { MOISTURE_MAX_SECONDS } from '../shared/seasonal-moisture';
import { DEFAULT_SOIL_MOISTURE_SETTINGS, SOIL_MOISTURE_CONTRACT, SOIL_MOISTURE_PINS,
  SOIL_STOCKS, SOIL_TRANSFERS } from '../shared/soil-moisture';
import type { PairedMassField, SoilBudget, SoilMoistureFrame, SoilResolution } from '../shared/soil-moisture';
import type { Packet } from './client';
import { close, matches, numericRecord, sum } from './water-validation';

function pairedTotal(pair: PairedMassField): number { return sum([...pair.high, ...pair.low]); }
function difference(pair: PairedMassField, old: PairedMassField, r: number): number {
  return sum([pair.high[r] - old.high[r], pair.low[r], -old.low[r]]);
}

/** Display transport validation is independent of native checkpoint ledgers.
 * Every real stock and cumulative local history retains both signed components. */
export function decodeSoilMoisture(packet: Packet, world: World, epoch: number,
  elapsedSeconds: number, intervalSeconds: number, previous?: SoilMoistureFrame): SoilMoistureFrame {
  const { header: h, bytes } = packet, n = world.stats.regionCount, contract = SOIL_MOISTURE_CONTRACT;
  const ids = [...new Set(world.water.bodyIds)].filter(id => id > 0).sort((a, b) => a - b);
  const integer = (v: unknown, low: number, high: number): boolean =>
    Number.isSafeInteger(v) && (v as number) >= low && (v as number) <= high;
  const steps = h.coupledSubsteps;
  if (h.kind !== contract.displayKind || h.protocol !== contract.protocol || h.modelVersion !== contract.modelVersion
    || Object.entries(SOIL_MOISTURE_PINS).some(([key, value]) => h[key] !== value)
    || h.temperatureModelVersion !== TEMPERATURE_MODEL_VERSION || h.windModelVersion !== WIND_MODEL_VERSION
    || !matches(h.settings, DEFAULT_SOIL_MOISTURE_SETTINGS)
    || !matches(h.temperatureSettings, DEFAULT_TEMPERATURE_SETTINGS) || !matches(h.windSettings, DEFAULT_WIND_SETTINGS)
    || h.regionCount !== n || h.referenceBodyCount !== ids.length || h.byteLength !== bytes.length
    || bytes.length !== (37 * n + 2 * ids.length) * 8
    || !integer(elapsedSeconds, 0, MOISTURE_MAX_SECONDS) || !integer(intervalSeconds, 0, 86400)
    || intervalSeconds > elapsedSeconds || h.elapsedSeconds !== elapsedSeconds || h.intervalSeconds !== intervalSeconds
    || !integer(steps, intervalSeconds ? 1 : 0, intervalSeconds ? Math.min(intervalSeconds, 4096) : 0)
    || !integer(h.atmosphericSubsteps, intervalSeconds ? steps as number : 0, (steps as number) * 4096)
    || !integer(h.surfaceSubsteps, 2 * (steps as number), 2 * (steps as number) * 16384)
    || (previous && (previous.epoch !== epoch || previous.elapsedSeconds + intervalSeconds !== elapsedSeconds))) {
    throw new Error('Invalid unified soil-water frame metadata.');
  }
  let cursor = 0;
  const field = (count = n, signed = false): Float64Array => {
    const values = new Float64Array(count);
    for (let i = 0; i < count; i++, cursor += 8) {
      values[i] = bytes.readDoubleLE(cursor);
      if (!Number.isFinite(values[i]) || (!signed && values[i] < 0)) throw new Error('Invalid soil-water field.');
    }
    return values;
  };
  const pair = (count = n): PairedMassField => {
    const high = field(count), low = field(count, true);
    for (let i = 0; i < count; i++) {
      const a = high[i], b = low[i], s = a + b, virtual = s - a;
      if (s !== a || (a - (s - virtual)) + (b - virtual) !== b || (a === 0 && b !== 0)) {
        throw new Error('Invalid normalized soil-water pair.');
      }
    }
    return { high, low };
  };
  const stocks = Object.fromEntries(SOIL_STOCKS.map(k => [k, pair()])) as SoilMoistureFrame['stocks'];
  const cumulativeLocalTransfers = Object.fromEntries(SOIL_TRANSFERS.map(k => [k, pair()])) as SoilMoistureFrame['cumulativeLocalTransfers'];
  const intervalLocalTransfers = Object.fromEntries(SOIL_TRANSFERS.map(k => [k, field()])) as SoilMoistureFrame['intervalLocalTransfers'];
  const regionalLiquidDepthMeters = field(), visibleWaterDepthMeters = field(), visibleWaterLevelMeters = field(n, true);
  const cumulativeSurfaceIncomingKilograms = field(), cumulativeSurfaceOutgoingKilograms = field();
  const intervalDrainageSentKilograms = field(), referenceBodies = pair(ids.length);
  const budget = numericRecord(h.budget, ['initialKilograms', 'storedKilograms', 'relativeGlobalResidual',
    'maximumRelativeLocalResidual']) as unknown as SoilBudget;
  const resolution = numericRecord(h.resolution, ['deferredRequests', 'summedDeferredRequestKilograms',
    'maximumDeferredRequestKilograms']) as unknown as SoilResolution;
  if (!Number.isSafeInteger(resolution.deferredRequests)
    || resolution.maximumDeferredRequestKilograms > resolution.summedDeferredRequestKilograms
    || (resolution.deferredRequests === 0 && (resolution.maximumDeferredRequestKilograms !== 0 || resolution.summedDeferredRequestKilograms !== 0))
    || (resolution.deferredRequests > 0 && resolution.maximumDeferredRequestKilograms === 0)
    || budget.relativeGlobalResidual > 1e-12 || budget.maximumRelativeLocalResidual > 128 * Number.EPSILON) {
    throw new Error('Invalid soil-water budget or resolution diagnostics.');
  }
  const initialColumns = world.surface.areasSquareMeters.map((area, r) =>
    area * Math.min(world.water.depthMeters[r], DEFAULT_SOIL_MOISTURE_SETTINGS.initialActiveSurfaceDepthMeters) * 1000);
  const initial = sum(initialColumns);
  close(budget.initialKilograms, initial);
  const total = sum([...SOIL_STOCKS.map(k => pairedTotal(stocks[k])), pairedTotal(referenceBodies)]);
  close(budget.storedKilograms, total, Math.max(initial, 1));
  close(total, initial, Math.max(initial, 1));
  close(sum(cumulativeSurfaceIncomingKilograms), sum(cumulativeSurfaceOutgoingKilograms));
  for (let r = 0; r < n; r++) {
    const wet = world.water.bodyIds[r] > 0;
    const capacity = world.surface.areasSquareMeters[r] * DEFAULT_SOIL_MOISTURE_SETTINGS.soil.soilCapacityKilogramsPerSquareMeter;
    if (stocks.soil.high[r] > capacity || (stocks.soil.high[r] === capacity && stocks.soil.low[r] > 0)
      || (wet && (['liquid', 'soil', 'drainage'] as const).some(k => stocks[k].high[r] !== 0))
      || (wet && (cumulativeSurfaceOutgoingKilograms[r] !== 0 || intervalDrainageSentKilograms[r] !== 0
        || (['infiltration', 'soilEvaporation', 'soilDrainage'] as const)
          .some(k => cumulativeLocalTransfers[k].high[r] !== 0)))) {
      throw new Error('Invalid soil-water regional ownership or capacity.');
    }
    close(regionalLiquidDepthMeters[r], stocks.liquid.high[r] / (1000 * world.surface.areasSquareMeters[r]));
    close(visibleWaterDepthMeters[r], wet ? Math.max(0, world.water.levelMeters - world.terrain.elevation[r]) : regionalLiquidDepthMeters[r]);
    if (visibleWaterDepthMeters[r] > 0) close(visibleWaterLevelMeters[r], wet ? world.water.levelMeters
      : world.terrain.elevation[r] + regionalLiquidDepthMeters[r]);
    else if (visibleWaterLevelMeters[r] !== 0) throw new Error('Dry soil-water levels require a zero placeholder.');
    for (const k of SOIL_TRANSFERS) {
      const transfer = intervalLocalTransfers[k][r];
      if (!intervalSeconds && transfer !== 0) throw new Error('Observation has an interval transfer.');
      if (previous) {
        const increment = difference(cumulativeLocalTransfers[k], previous.cumulativeLocalTransfers[k], r);
        close(increment, transfer, Math.max(Math.abs(increment), transfer, 1));
        if (increment < 0) throw new Error('Soil-water history decreased.');
      }
    }
    if (!intervalSeconds && intervalDrainageSentKilograms[r] !== 0) throw new Error('Observation has a drainage departure.');
    if (!elapsedSeconds && (SOIL_STOCKS.some(k => stocks[k].high[r] !== 0)
      || SOIL_TRANSFERS.some(k => cumulativeLocalTransfers[k].high[r] !== 0)
      || cumulativeSurfaceIncomingKilograms[r] !== 0 || cumulativeSurfaceOutgoingKilograms[r] !== 0)) {
      throw new Error('Invalid initial soil-water partition or history.');
    }
  }
  if (!elapsedSeconds) {
    if (resolution.deferredRequests !== 0) throw new Error('Initial soil-water diagnostics must be empty.');
    const groups = new Map(ids.map(id => [id, [] as number[]]));
    for (let r = 0; r < n; r++) groups.get(world.water.bodyIds[r])?.push(initialColumns[r]);
    for (let b = 0; b < ids.length; b++) close(referenceBodies.high[b] + referenceBodies.low[b], sum(groups.get(ids[b])!));
  }
  if (previous) {
    if (!intervalSeconds) {
      const unchanged = (a: Float64Array, b: Float64Array): boolean =>
        a.length === b.length && a.every((v, i) => Object.is(v, b[i]));
      const samePair = (a: PairedMassField, b: PairedMassField): boolean =>
        unchanged(a.high, b.high) && unchanged(a.low, b.low);
      if (SOIL_STOCKS.some(k => !samePair(stocks[k], previous.stocks[k]))
        || SOIL_TRANSFERS.some(k => !samePair(cumulativeLocalTransfers[k], previous.cumulativeLocalTransfers[k]))
        || !samePair(referenceBodies, previous.referenceBodies)
        || !unchanged(cumulativeSurfaceIncomingKilograms, previous.cumulativeSurfaceIncomingKilograms)
        || !unchanged(cumulativeSurfaceOutgoingKilograms, previous.cumulativeSurfaceOutgoingKilograms)
        || !matches(budget, previous.budget) || !matches(resolution, previous.resolution)) {
        throw new Error('Soil-water observation changed state.');
      }
    }
    if (resolution.deferredRequests < previous.resolution.deferredRequests
      || resolution.summedDeferredRequestKilograms < previous.resolution.summedDeferredRequestKilograms
      || resolution.maximumDeferredRequestKilograms < previous.resolution.maximumDeferredRequestKilograms) {
      throw new Error('Soil-water diagnostics decreased.');
    }
    for (let r = 0; r < n; r++) {
      if (cumulativeSurfaceIncomingKilograms[r] < previous.cumulativeSurfaceIncomingKilograms[r]
        || cumulativeSurfaceOutgoingKilograms[r] < previous.cumulativeSurfaceOutgoingKilograms[r]) {
        throw new Error('Soil-water cumulative surface flow decreased.');
      }
    }
  }
  return { epoch, modelVersion: contract.modelVersion, settings: DEFAULT_SOIL_MOISTURE_SETTINGS,
    elapsedSeconds, intervalSeconds, coupledSubsteps: steps as number,
    atmosphericSubsteps: h.atmosphericSubsteps as number, surfaceSubsteps: h.surfaceSubsteps as number,
    stocks, cumulativeLocalTransfers, intervalLocalTransfers, intervalDrainageSentKilograms,
    regionalLiquidDepthMeters, visibleWaterDepthMeters, visibleWaterLevelMeters,
    cumulativeSurfaceIncomingKilograms, cumulativeSurfaceOutgoingKilograms,
    referenceBodyIds: Uint32Array.from(ids), referenceBodies, budget, resolution };
}
