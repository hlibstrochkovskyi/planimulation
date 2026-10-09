import type { World } from '../core/world';
import { SOIL_STOCKS, SOIL_TRANSFERS } from '../shared/soil-moisture';
import type { PairedMassField, SoilMoistureFrame } from '../shared/soil-moisture';
import { sum } from '../native/water-validation';

const format = new Intl.NumberFormat('en', { maximumSignificantDigits: 6 });
const names = { liquid: 'Unified terrestrial liquid', soil: 'Terrestrial soil water', snow: 'Snow water equivalent',
  vapor: 'Atmospheric vapor', drainage: 'Delayed soil drainage' };
const transfers = { rain: 'Rain', snowfall: 'Snowfall', melt: 'Melt', liquidEvaporation: 'Liquid/body evaporation',
  soilEvaporation: 'Soil evaporation', infiltration: 'Infiltration', soilDrainage: 'Soil drainage' };
const raw = (v: number): string => Object.is(v, -0) ? '-0' : String(v);
const volume = (kg: number): string => `${format.format(kg / 1e12)} km³ WE`;
function pairedTotal(p: PairedMassField): number { return sum([...p.high, ...p.low]); }
type Rows = Array<[string, string]>;

/** Formatting only: rounded summary values never replace physical stock pairs. */
export function soilWaterInspection(world: World, frame: SoilMoistureFrame, region: number | null): {
  budgetRows: Rows; selectionRows: Rows; budgetNote: string; selectionNote: string;
} {
  const budgetRows: Rows = [
    ['Initial mobile partition', volume(frame.budget.initialKilograms)],
    ...SOIL_STOCKS.map((k): [string, string] => [names[k], volume(pairedTotal(frame.stocks[k]))]),
    ['Finite reference-body liquid · counted once', volume(pairedTotal(frame.referenceBodies))],
    ['Total current owned water', volume(frame.budget.storedKilograms)],
    ['Relative global mass residual', frame.budget.relativeGlobalResidual.toExponential(3)],
    ['Maximum relative local ledger residual', frame.budget.maximumRelativeLocalResidual.toExponential(3)],
    ['Cumulative surface-face transfers · flow integral', volume(sum(frame.cumulativeSurfaceOutgoingKilograms))],
    ['Deferred numerical requests · lifetime count', format.format(frame.resolution.deferredRequests)],
    ['Summed deferred requests · repeated requests, not lost mass', `${raw(frame.resolution.summedDeferredRequestKilograms)} kg`],
    ['Largest deferred request · retained at donor', `${raw(frame.resolution.maximumDeferredRequestKilograms)} kg`],
  ];
  const budgetNote = `${frame.modelVersion} · five paired regional owners plus finite reference-body liquid, counted once per body. `
    + 'retainDonor numerical policy: unresolved transfers remain at their donors; diagnostics are not stocks or completed flow. '
    + 'Soil stays active under terrestrial liquid films. Reference heads/coasts are prescribed; empirical exchange and capped mobility are not calibrated. '
    + 'Save preserves all native components. Summary volumes are rounded; selected owners show exact received high/low components.';
  const selectionRows: Rows = [];
  if (region !== null) {
    if (!Number.isSafeInteger(region) || region < 0 || region >= world.stats.regionCount) throw new Error('Invalid soil-water inspection region.');
    const area = world.surface.areasSquareMeters[region], column = (kg: number) => `${format.format(kg / area)} mm WE`;
    for (const k of SOIL_STOCKS) {
      const p = frame.stocks[k];
      selectionRows.push([names[k], `${column(p.high[region])} · rounded leading display`],
        [`${names[k]} · exact high / signed low`, `${raw(p.high[region])} kg / ${raw(p.low[region])} kg`]);
    }
    selectionRows.push(['Regional liquid depth · leading component', `${format.format(frame.regionalLiquidDepthMeters[region])} m`],
      ['Visible water depth · reference heads prescribed', `${format.format(frame.visibleWaterDepthMeters[region])} m`],
      ['Cumulative surface-face inflow', volume(frame.cumulativeSurfaceIncomingKilograms[region])],
      ['Cumulative surface-face outflow', volume(frame.cumulativeSurfaceOutgoingKilograms[region])]);
    const slot = frame.referenceBodyIds.indexOf(world.water.bodyIds[region]);
    if (slot >= 0) {
      selectionRows.push(['Reference body owner', `Body ${frame.referenceBodyIds[slot]} · shared stock, not allocated to this region`],
        ['Whole reference-body mobile liquid · exact high / signed low', `${raw(frame.referenceBodies.high[slot])} kg / ${raw(frame.referenceBodies.low[slot])} kg`]);
    }
    for (const k of SOIL_TRANSFERS) {
      selectionRows.push([`Last interval · ${transfers[k]}`, column(frame.intervalLocalTransfers[k][region])],
        [`Cumulative ${transfers[k]} · exact high / signed low`, `${raw(frame.cumulativeLocalTransfers[k].high[region])} kg / ${raw(frame.cumulativeLocalTransfers[k].low[region])} kg`]);
    }
    selectionRows.push(['Last interval · delayed drainage departure', column(frame.intervalDrainageSentKilograms[region])],
      ['Mean delayed drainage departure', frame.intervalSeconds
        ? `${format.format(frame.intervalDrainageSentKilograms[region] / 1000 / frame.intervalSeconds)} m³/s · not hydraulic discharge` : 'No interval']);
  }
  const selectionNote = `At ${frame.elapsedSeconds} elapsed seconds. High and signed low belong to the same owner, not two stocks. `
    + `Last-interval transfers cover ${frame.intervalSeconds / 3600} h; cumulative histories are flow integrals, not additional water. `
    + (region === null ? 'Select a region to inspect exact components.' : `Delayed soil drainage receiver: ${world.drainage.receivers[region] === region ? 'self · terminal delivery' : `region ${world.drainage.receivers[region]}`}. `)
    + 'Water masks/caps approximate whole computational regions, not resolved shorelines.';
  return { budgetRows, selectionRows, budgetNote, selectionNote };
}
