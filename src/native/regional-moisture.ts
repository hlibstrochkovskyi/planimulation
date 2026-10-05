import type { World } from '../core/world';
import { DEFAULT_TEMPERATURE_SETTINGS, TEMPERATURE_MODEL_VERSION } from '../core/seasonal-temperature';
import { DEFAULT_WIND_SETTINGS, WIND_MODEL_VERSION } from '../core/seasonal-wind';
import { MOISTURE_MODES, MOISTURE_MAX_SECONDS, MOISTURE_STOCK_FIELDS,
  SURFACE_TRANSFER_FIELDS, RUNOFF_TRANSFER_FIELDS } from '../shared/seasonal-moisture';
import type { MoistureBudget, MoistureFrame, RegionalFlowDiagnostics } from '../shared/seasonal-moisture';
import type { Packet } from './client';
import { close, matches, numericRecord, sum } from './water-validation';

/** Protocol 14 never reinterprets body-owned liquid as a regional stock.
 * Rounded display reconciliation is not authoritative compensated accounting;
 * complete per-owner/per-face ledgers remain validated in the native core. */
export function decodeRegionalMoisture(packet: Packet, world: World, epoch: number, elapsedSeconds: number,
  intervalSeconds: number, previous?: MoistureBudget, previousFaceTransfer?: number): MoistureFrame {
  const { header: h, bytes } = packet, n = world.stats.regionCount, contract = MOISTURE_MODES.regional;
  const integer = (v: unknown, low: number, high: number): boolean => Number.isSafeInteger(v) && (v as number) >= low && (v as number) <= high;
  const ids = [...new Set(world.water.bodyIds)].filter((id) => id > 0).sort((a, b) => a - b);
  if (h.kind !== contract.displayKind || h.protocol !== contract.protocol || h.regionCount !== n
    || h.referenceBodyCount !== ids.length || h.byteLength !== bytes.length || bytes.length !== (22 * n + 2 * ids.length) * 8
    || h.modelVersion !== contract.modelVersion || h.surfaceModelVersion !== contract.surfaceVersion
    || h.runoffModelVersion !== 'runoff-transport-1' || h.transportModelVersion !== 'moisture-transport-1'
    || h.temperatureModelVersion !== TEMPERATURE_MODEL_VERSION || h.windModelVersion !== WIND_MODEL_VERSION
    || h.orographicModelVersion !== 'orographic-response-1' || h.terminalStockModelVersion !== 'terminal-stock-compensated-1'
    || h.referenceBodyModelVersion !== 'reference-water-pool-1' || h.closedLakeModelVersion !== 'regional-surface-flow-1'
    || h.regionalSurfaceObservationVersion !== 'regional-surface-observation-1' || h.regionalTransferObservationVersion !== 'regional-surface-transfers-1'
    || !matches(h.settings, contract.settings) || !matches(h.temperatureSettings, DEFAULT_TEMPERATURE_SETTINGS)
    || !matches(h.windSettings, DEFAULT_WIND_SETTINGS) || h.elapsedSeconds !== elapsedSeconds || h.intervalSeconds !== intervalSeconds
    || !integer(elapsedSeconds, 0, MOISTURE_MAX_SECONDS) || !integer(intervalSeconds, 0, 86400) || intervalSeconds > elapsedSeconds
    || !integer(h.coupledSubsteps, intervalSeconds ? 1 : 0, intervalSeconds)
    || !integer(h.transportSubsteps, intervalSeconds ? 1 : 0, intervalSeconds * 4096)
    || typeof h.explicitStabilityBoundSeconds !== 'number' || !Number.isFinite(h.explicitStabilityBoundSeconds) || h.explicitStabilityBoundSeconds <= 0) {
    throw new Error('Invalid regional-water frame metadata.');
  }
  for (const key of ['maximumAbsoluteLocalExchangeResidualKilograms', 'maximumAbsoluteRoutingResidualKilograms']) {
    if (typeof h[key] !== 'number' || !Number.isFinite(h[key]) || (h[key] as number) < 0
      || (!intervalSeconds && h[key] !== 0)) throw new Error('Invalid regional-water residual.');
  }
  let offset = 0;
  const field = (count = n, signed = false): Float64Array => {
    const values = new Float64Array(count);
    for (let i = 0; i < count; i++, offset += 8) {
      values[i] = bytes.readDoubleLE(offset);
      if (!Number.isFinite(values[i]) || (!signed && values[i] < 0)) throw new Error('Invalid regional-water field.');
    }
    return values;
  };
  const fields = <K extends string>(keys: readonly K[]): Record<K, Float64Array> =>
    Object.fromEntries(keys.map((key) => [key, field()])) as Record<K, Float64Array>;
  const stocks = fields(MOISTURE_STOCK_FIELDS), surfaceTransfers = fields(SURFACE_TRANSFER_FIELDS);
  const runoffTransfers = fields(RUNOFF_TRANSFER_FIELDS);
  const depthMeters = field(), levelsMeters = field(n, true);
  const cumulativeIncomingKilograms = field(), cumulativeOutgoingKilograms = field();
  const referenceBodyHighKilograms = field(ids.length), referenceBodyLowKilograms = field(ids.length, true);
  const flow = numericRecord(h.regionalSurfaceFlow, ['substeps', 'transferredKilograms', 'deferredRequestKilograms', 'deferredRequests',
    'maximumDeferredRequestKilograms', 'deferredEvaporationRequests', 'deferredEvaporationRequestKilograms', 'maximumDeferredEvaporationKilograms']) as unknown as RegionalFlowDiagnostics;
  for (const key of ['substeps', 'deferredRequests', 'deferredEvaporationRequests'] as const) {
    if (!Number.isSafeInteger(flow[key]) || (!intervalSeconds && flow[key] !== 0)) throw new Error('Invalid regional-flow count.');
  }
  if ((intervalSeconds && (!flow.substeps || flow.substeps > 2 * (h.coupledSubsteps as number) * 16384))
    || (!intervalSeconds && Object.values(flow).some((value) => value !== 0))
    || flow.maximumDeferredRequestKilograms > flow.deferredRequestKilograms
    || flow.maximumDeferredEvaporationKilograms > flow.deferredEvaporationRequestKilograms
    || (!flow.deferredRequests && (flow.deferredRequestKilograms || flow.maximumDeferredRequestKilograms))
    || (!flow.deferredEvaporationRequests && (flow.deferredEvaporationRequestKilograms || flow.maximumDeferredEvaporationKilograms))) {
    throw new Error('Invalid regional-flow diagnostics.');
  }
  if (typeof h.cumulativeFaceTransferKilograms !== 'number' || !Number.isFinite(h.cumulativeFaceTransferKilograms)
    || h.cumulativeFaceTransferKilograms < 0) throw new Error('Invalid cumulative face flow.');
  close(sum(cumulativeIncomingKilograms), h.cumulativeFaceTransferKilograms);
  close(sum(cumulativeOutgoingKilograms), h.cumulativeFaceTransferKilograms);
  if (previousFaceTransfer !== undefined) close(h.cumulativeFaceTransferKilograms, previousFaceTransfer + flow.transferredKilograms);
  const raw = h.budget as Record<string, unknown>;
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Invalid regional-water budget.');
  const { cumulativeSurfaceTransfers, cumulativeRunoffTransfers, ...quantities } = raw;
  const values = numericRecord(quantities, [...MOISTURE_STOCK_FIELDS, 'initialMobileWaterKilograms', 'residualKilograms',
    'cumulativeEvaporationKilograms', 'cumulativePrecipitationKilograms', 'maximumRelativeLocalSurfaceLedgerResidual', 'vaporLedgerResidualKilograms',
    'referenceBodyWaterKilograms', 'cumulativeLakeCaptureKilograms'], ['residualKilograms', 'vaporLedgerResidualKilograms']);
  const surface = numericRecord(cumulativeSurfaceTransfers, SURFACE_TRANSFER_FIELDS), runoff = numericRecord(cumulativeRunoffTransfers, RUNOFF_TRANSFER_FIELDS);
  const budget = { ...values, cumulativeSurfaceTransfers: surface, cumulativeRunoffTransfers: runoff } as unknown as MoistureBudget;
  const initialColumns = world.surface.areasSquareMeters.map((area, i) =>
    area * Math.min(world.water.depthMeters[i], contract.settings.initialActiveSurfaceDepthMeters) * 1000);
  const initial = sum(initialColumns), scale = Math.max(initial, 1);
  close(initial, budget.initialMobileWaterKilograms, scale);
  for (const key of MOISTURE_STOCK_FIELDS) close(sum(stocks[key]), budget[key], scale);
  // Group once, rather than scanning the globe for each fragmented body.
  const initialBodyColumns = new Map(ids.map((id) => [id, [] as number[]]));
  if (!elapsedSeconds) for (let i = 0; i < n; i++) initialBodyColumns.get(world.water.bodyIds[i])?.push(initialColumns[i]);
  for (let i = 0; i < ids.length; i++) {
    const a = referenceBodyHighKilograms[i], b = referenceBodyLowKilograms[i], s = a + b, virtual = s - a;
    if (s !== a || (a - (s - virtual)) + (b - virtual) !== b || (a === 0 && b !== 0)) throw new Error('Invalid reference-body pair.');
    if (!elapsedSeconds) close(a + b, sum(initialBodyColumns.get(ids[i])!));
  }
  close(sum([...referenceBodyHighKilograms, ...referenceBodyLowKilograms]), budget.referenceBodyWaterKilograms!, scale);
  const total = sum([...MOISTURE_STOCK_FIELDS.map((key) => budget[key]), budget.referenceBodyWaterKilograms!]);
  close(total, initial, scale); close(budget.residualKilograms, total - initial, scale);
  if (budget.maximumRelativeLocalSurfaceLedgerResidual > 1e-12) throw new Error('Invalid regional-water local ledger.');
  close(budget.cumulativeEvaporationKilograms, surface.liquidEvaporation + surface.soilEvaporation + runoff.terminalEvaporation);
  close(budget.cumulativePrecipitationKilograms, surface.rain + surface.snowfall);
  close(runoff.sent, runoff.receivedTransit + runoff.terminalDelivery);
  const vaporScale = Math.max(budget.vaporKilograms, budget.cumulativeEvaporationKilograms, budget.cumulativePrecipitationKilograms, 1);
  close(budget.vaporKilograms, budget.cumulativeEvaporationKilograms - budget.cumulativePrecipitationKilograms, vaporScale);
  close(budget.vaporLedgerResidualKilograms, budget.vaporKilograms - budget.cumulativeEvaporationKilograms + budget.cumulativePrecipitationKilograms, vaporScale);
  for (let i = 0; i < n; i++) {
    const wet = world.water.bodyIds[i] !== 0, terminal = world.drainage.receivers[i] === i;
    if ((!terminal && runoffTransfers.terminalDelivery[i] !== 0)
      || (terminal && (stocks.pendingRunoffKilograms[i] !== 0 || runoffTransfers.receivedTransit[i] !== 0))
      || stocks.soilKilograms[i] > world.surface.areasSquareMeters[i] * contract.settings.surface.soilCapacityKilogramsPerSquareMeter
      || (wet && (stocks.surfaceKilograms[i] !== 0 || stocks.terminalWaterKilograms[i] !== 0 || stocks.soilKilograms[i] !== 0
        || cumulativeOutgoingKilograms[i] !== 0 || ['infiltration', 'soilEvaporation', 'liquidRunoff', 'soilDrainage']
          .some((key) => surfaceTransfers[key as keyof typeof surfaceTransfers][i] !== 0)))) {
      throw new Error('Invalid regional-water ownership.');
    }
    close(depthMeters[i], stocks.terminalWaterKilograms[i] / (1000 * world.surface.areasSquareMeters[i]));
    if (depthMeters[i] > 0) close(levelsMeters[i], world.terrain.elevation[i] + depthMeters[i]);
    else if (levelsMeters[i] !== 0) throw new Error('A dry regional level must use its zero placeholder.');
    if (!elapsedSeconds && (MOISTURE_STOCK_FIELDS.some((key) => stocks[key][i] !== 0)
      || cumulativeIncomingKilograms[i] !== 0 || cumulativeOutgoingKilograms[i] !== 0)) {
      throw new Error('Invalid initial regional-water partition.');
    }
  }
  if (!intervalSeconds && [...Object.values(surfaceTransfers), ...Object.values(runoffTransfers)]
    .some((array) => array.some((value) => value !== 0))) throw new Error('An observation cannot contain interval flows.');
  if (!elapsedSeconds && (Object.values(surface).some((value) => value !== 0)
    || Object.values(runoff).some((value) => value !== 0) || budget.cumulativeLakeCaptureKilograms !== 0)) {
    throw new Error('Invalid initial regional-water history.');
  }
  if (previous) {
    for (const key of SURFACE_TRANSFER_FIELDS) close(surface[key], previous.cumulativeSurfaceTransfers[key] + sum(surfaceTransfers[key]));
    for (const key of RUNOFF_TRANSFER_FIELDS) close(runoff[key], previous.cumulativeRunoffTransfers[key] + sum(runoffTransfers[key]));
  }
  return {
    epoch, modelVersion: contract.modelVersion, settings: contract.settings,
    temperatureSettings: DEFAULT_TEMPERATURE_SETTINGS, windSettings: DEFAULT_WIND_SETTINGS,
    elapsedSeconds, intervalSeconds, coupledSubsteps: h.coupledSubsteps as number,
    transportSubsteps: h.transportSubsteps as number,
    maximumAbsoluteLocalExchangeResidualKilograms: h.maximumAbsoluteLocalExchangeResidualKilograms as number,
    maximumAbsoluteRoutingResidualKilograms: h.maximumAbsoluteRoutingResidualKilograms as number,
    stocks, surfaceTransfers, runoffTransfers, budget,
    regionalSurface: { depthMeters, levelsMeters, cumulativeIncomingKilograms, cumulativeOutgoingKilograms,
      cumulativeTransferredKilograms: h.cumulativeFaceTransferKilograms,
      referenceBodyIds: Uint32Array.from(ids), referenceBodyHighKilograms, referenceBodyLowKilograms,
      explicitStabilityBoundSeconds: h.explicitStabilityBoundSeconds, flow },
  };
}
