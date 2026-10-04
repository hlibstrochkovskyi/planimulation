import type { World } from '../core/world';
import { DEFAULT_TEMPERATURE_SETTINGS, TEMPERATURE_MODEL_VERSION } from '../core/seasonal-temperature';
import { DEFAULT_WIND_SETTINGS, WIND_MODEL_VERSION } from '../core/seasonal-wind';
import { DEFAULT_MOISTURE_SETTINGS, DEFAULT_OROGRAPHIC_MOISTURE_SETTINGS,
  OROGRAPHIC_MOISTURE_MODEL_VERSION, OROGRAPHIC_RESPONSE_MODEL_VERSION, MOISTURE_MAX_SECONDS, MOISTURE_MODEL_VERSION,
  MOISTURE_STOCK_FIELDS, SURFACE_TRANSFER_FIELDS, RUNOFF_TRANSFER_FIELDS } from '../shared/seasonal-moisture';
import type { MoistureBudget, MoistureFrame } from '../shared/seasonal-moisture';
import type { Packet } from './client';

function sum(values: Iterable<number>): number {
  let total = 0, correction = 0;
  for (const value of values) {
    const adjusted = value - correction, next = total + adjusted;
    correction = (next - total) - adjusted; total = next;
  }
  return total;
}
function matches(value: unknown, expected: unknown): boolean {
  if (typeof expected !== 'object' || expected === null) return value === expected;
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const object = value as Record<string, unknown>, reference = expected as Record<string, unknown>;
  return Object.keys(object).length === Object.keys(reference).length
    && Object.entries(reference).every(([key, item]) => matches(object[key], item));
}
function numericRecord(value: unknown, keys: readonly string[], signed: readonly string[] = []): Record<string, number> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid seasonal-water budget.');
  const object = value as Record<string, unknown>;
  if (Object.keys(object).length !== keys.length || keys.some((key) => typeof object[key] !== 'number'
    || !Number.isFinite(object[key]) || (!signed.includes(key) && (object[key] as number) < 0))) {
    throw new Error('Invalid seasonal-water budget quantities.');
  }
  return object as Record<string, number>;
}
function close(a: number, b: number, scale = Math.max(Math.abs(a), Math.abs(b), 1)): void {
  if (!Number.isFinite(a) || !Number.isFinite(b) || Math.abs(a - b) > 1e-12 * scale) {
    throw new Error('Seasonal-water display budget does not reconcile.');
  }
}

/** Validate a display snapshot independently of the authoritative Rust budget.
 * This does not replace checkpoint validation or reconstruct hidden accumulators. */
export function decodeSeasonalMoisture(packet: Packet, world: World, epoch: number,
  elapsedSeconds: number, intervalSeconds: number, previous?: MoistureBudget, orographic = false): MoistureFrame {
  const { header: h, bytes } = packet, count = world.stats.regionCount;
  const integer = (value: unknown, low: number, high: number): boolean =>
    Number.isSafeInteger(value) && (value as number) >= low && (value as number) <= high;
  const version = orographic ? OROGRAPHIC_MOISTURE_MODEL_VERSION : MOISTURE_MODEL_VERSION;
  if (h.kind !== (orographic ? 'orographicMoisture' : 'moisture') || h.protocol !== (orographic ? 12 : 11)
    || (orographic ? h.orographicModelVersion !== OROGRAPHIC_RESPONSE_MODEL_VERSION : h.orographicModelVersion !== undefined)
    || h.regionCount !== count || h.byteLength !== bytes.length || bytes.length !== count * 18 * 8
    || h.modelVersion !== version || h.surfaceModelVersion !== 'surface-water-1'
    || h.runoffModelVersion !== 'runoff-transport-1' || h.transportModelVersion !== 'moisture-transport-1'
    || h.temperatureModelVersion !== TEMPERATURE_MODEL_VERSION || h.windModelVersion !== WIND_MODEL_VERSION
    || !matches(h.settings, orographic ? DEFAULT_OROGRAPHIC_MOISTURE_SETTINGS : DEFAULT_MOISTURE_SETTINGS) || !matches(h.temperatureSettings, DEFAULT_TEMPERATURE_SETTINGS)
    || !matches(h.windSettings, DEFAULT_WIND_SETTINGS) || h.elapsedSeconds !== elapsedSeconds
    || !integer(h.elapsedSeconds, 0, MOISTURE_MAX_SECONDS) || h.intervalSeconds !== intervalSeconds
    || !integer(h.intervalSeconds, 0, 86400) || intervalSeconds > elapsedSeconds
    || !integer(h.coupledSubsteps, intervalSeconds ? 1 : 0, intervalSeconds)
    || !integer(h.transportSubsteps, intervalSeconds ? 1 : 0, intervalSeconds * DEFAULT_MOISTURE_SETTINGS.transport.maxSubsteps)) {
    throw new Error('Invalid seasonal-water frame metadata.');
  }
  for (const key of ['maximumAbsoluteLocalExchangeResidualKilograms', 'maximumAbsoluteRoutingResidualKilograms']) {
    if (typeof h[key] !== 'number' || !Number.isFinite(h[key]) || (h[key] as number) < 0
      || (!intervalSeconds && h[key] !== 0)) throw new Error('Invalid seasonal-water residual.');
  }
  let offset = 0;
  const fields = <K extends string>(keys: readonly K[]): Record<K, Float64Array> => {
    return Object.fromEntries(keys.map((key) => {
      const field = new Float64Array(count);
      for (let i = 0; i < count; i++, offset += 8) {
        field[i] = bytes.readDoubleLE(offset);
        if (!Number.isFinite(field[i]) || field[i] < 0) throw new Error('Invalid seasonal-water field.');
      }
      return [key, field];
    })) as Record<K, Float64Array>;
  };
  const stocks = fields(MOISTURE_STOCK_FIELDS), surfaceTransfers = fields(SURFACE_TRANSFER_FIELDS),
    runoffTransfers = fields(RUNOFF_TRANSFER_FIELDS);
  const raw = h.budget as Record<string, unknown>;
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Invalid seasonal-water budget.');
  const { cumulativeSurfaceTransfers, cumulativeRunoffTransfers, ...quantities } = raw;
  const values = numericRecord(quantities, [...MOISTURE_STOCK_FIELDS, 'initialMobileWaterKilograms',
    'residualKilograms', 'cumulativeEvaporationKilograms', 'cumulativePrecipitationKilograms',
    'maximumRelativeLocalSurfaceLedgerResidual', 'vaporLedgerResidualKilograms'],
  ['residualKilograms', 'vaporLedgerResidualKilograms']);
  const surface = numericRecord(cumulativeSurfaceTransfers, SURFACE_TRANSFER_FIELDS);
  const runoff = numericRecord(cumulativeRunoffTransfers, RUNOFF_TRANSFER_FIELDS);
  const budget = { ...values, cumulativeSurfaceTransfers: surface, cumulativeRunoffTransfers: runoff } as unknown as MoistureBudget;
  const initial = sum(world.surface.areasSquareMeters.map((area, i) =>
    area * Math.min(world.water.depthMeters[i], DEFAULT_MOISTURE_SETTINGS.initialActiveSurfaceDepthMeters) * 1000));
  const scale = Math.max(initial, 1);
  close(budget.initialMobileWaterKilograms, initial, scale);
  for (const key of MOISTURE_STOCK_FIELDS) close(sum(stocks[key]), budget[key], scale);
  const total = sum(MOISTURE_STOCK_FIELDS.map((key) => budget[key]));
  close(total, initial, scale); close(budget.residualKilograms, total - initial, scale);
  if (budget.maximumRelativeLocalSurfaceLedgerResidual > 1e-12) throw new Error('Invalid seasonal-water regional ledger residual.');
  close(budget.cumulativeEvaporationKilograms, surface.liquidEvaporation + surface.soilEvaporation + runoff.terminalEvaporation);
  close(budget.cumulativePrecipitationKilograms, surface.rain + surface.snowfall);
  close(runoff.sent, runoff.receivedTransit + runoff.terminalDelivery);
  const vaporScale = Math.max(budget.vaporKilograms, budget.cumulativeEvaporationKilograms, budget.cumulativePrecipitationKilograms, 1);
  close(budget.vaporKilograms, budget.cumulativeEvaporationKilograms - budget.cumulativePrecipitationKilograms, vaporScale);
  close(budget.vaporLedgerResidualKilograms,
    budget.vaporKilograms - budget.cumulativeEvaporationKilograms + budget.cumulativePrecipitationKilograms, vaporScale);
  for (let i = 0; i < count; i++) {
    const terminal = world.drainage.receivers[i] === i, wet = world.water.depthMeters[i] > 0;
    if ((!terminal && (stocks.terminalWaterKilograms[i] !== 0 || runoffTransfers.terminalDelivery[i] !== 0
      || runoffTransfers.terminalEvaporation[i] !== 0))
      || (terminal && (stocks.pendingRunoffKilograms[i] !== 0 || runoffTransfers.receivedTransit[i] !== 0))
      || stocks.soilKilograms[i] > world.surface.areasSquareMeters[i] * DEFAULT_MOISTURE_SETTINGS.surface.soilCapacityKilogramsPerSquareMeter
      || (wet && (stocks.soilKilograms[i] !== 0 || ['infiltration', 'soilEvaporation', 'liquidRunoff', 'soilDrainage']
        .some((key) => surfaceTransfers[key as keyof typeof surfaceTransfers][i] !== 0)))) {
      throw new Error('Invalid seasonal-water regional ownership.');
    }
    if (!elapsedSeconds) {
      close(stocks.surfaceKilograms[i], world.surface.areasSquareMeters[i] * Math.min(world.water.depthMeters[i], 1) * 1000);
      if (MOISTURE_STOCK_FIELDS.slice(1).some((key) => stocks[key][i] !== 0)) throw new Error('Invalid initial seasonal-water partition.');
    }
  }
  if (!intervalSeconds && [...Object.values(surfaceTransfers), ...Object.values(runoffTransfers)].some((field) => field.some((v) => v !== 0))) {
    throw new Error('A seasonal-water observation cannot contain interval transfers.');
  }
  if (!elapsedSeconds && [...Object.values(surface), ...Object.values(runoff)].some((v) => v !== 0)) {
    throw new Error('Invalid initial seasonal-water ledgers.');
  }
  if (previous) {
    for (const key of SURFACE_TRANSFER_FIELDS) close(surface[key], previous.cumulativeSurfaceTransfers[key] + sum(surfaceTransfers[key]));
    for (const key of RUNOFF_TRANSFER_FIELDS) close(runoff[key], previous.cumulativeRunoffTransfers[key] + sum(runoffTransfers[key]));
  }
  return { epoch, modelVersion: version, settings: h.settings as MoistureFrame['settings'],
    temperatureSettings: h.temperatureSettings as MoistureFrame['temperatureSettings'], windSettings: h.windSettings as MoistureFrame['windSettings'],
    elapsedSeconds, intervalSeconds, coupledSubsteps: h.coupledSubsteps as number, transportSubsteps: h.transportSubsteps as number,
    maximumAbsoluteLocalExchangeResidualKilograms: h.maximumAbsoluteLocalExchangeResidualKilograms as number,
    maximumAbsoluteRoutingResidualKilograms: h.maximumAbsoluteRoutingResidualKilograms as number,
    stocks, surfaceTransfers, runoffTransfers, budget };
}
