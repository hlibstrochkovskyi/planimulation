import type { MoistureFrame, MoistureStock } from '../shared/seasonal-moisture';

export const MOISTURE_LAYERS = {
  liquidWater: { title: 'Local liquid', stock: 'surfaceKilograms', maximum: 1000, unit: 'mm WE' },
  snowWater: { title: 'Snow water equivalent', stock: 'snowKilograms', maximum: 1000, unit: 'mm WE' },
  soilWater: { title: 'Soil water', stock: 'soilKilograms', maximum: 150, unit: 'mm WE' },
  runoffWater: { title: 'Runoff in transit', stock: 'pendingRunoffKilograms', maximum: 1000, unit: 'mm WE' },
  terminalWater: { title: 'Terminal water column · not lake depth', stock: 'terminalWaterKilograms', maximum: 1000, unit: 'mm WE' },
  vaporWater: { title: 'Atmospheric vapor column', stock: 'vaporKilograms', maximum: 60, unit: 'mm WE' },
  precipitation: { title: 'Last-interval precipitation · mean rate', maximum: 20, unit: 'mm/day' },
  runoffFlow: { title: 'Last-interval drainage departure · mean rate, not hydraulic discharge', maximum: 1000000, unit: 'm³/s' },
  regionalDepth: { title: 'Regional pooled liquid depth · initial land only', maximum: 1000, unit: 'm' },
  surfaceInflow: { title: 'Cumulative neighboring surface inflow · not a stock or rate', maximum: 1000, unit: 'km³' },
  surfaceOutflow: { title: 'Cumulative neighboring surface outflow · not a stock or rate', maximum: 1000, unit: 'km³' },
} as const;
export type MoistureLayer = keyof typeof MOISTURE_LAYERS;
export function isMoistureLayer(layer: string): layer is MoistureLayer { return Object.hasOwn(MOISTURE_LAYERS, layer); }
export function isRegionalLayer(layer: string): boolean { return ['regionalDepth', 'surfaceInflow', 'surfaceOutflow'].includes(layer); }

/** 1 kg/m² = 1 mm water equivalent at 1,000 kg/m³. Pool columns
 * use a fixed reference-region footprint, never a computed lake wet area. */
export function moistureLayerValue(frame: MoistureFrame, layer: MoistureLayer, region: number, area: number): number {
  const info = MOISTURE_LAYERS[layer];
  if (isRegionalLayer(layer)) {
    const regional = frame.regionalSurface;
    if (!regional) throw new Error('Regional layer requires the regional ownership mode.');
    return layer === 'regionalDepth' ? regional.depthMeters[region]
      : (layer === 'surfaceInflow' ? regional.cumulativeIncomingKilograms[region] : regional.cumulativeOutgoingKilograms[region]) / 1e12;
  }
  if ('stock' in info) return frame.stocks[info.stock as MoistureStock][region] / area;
  if (frame.intervalSeconds === 0) return 0;
  return layer === 'precipitation'
    ? (frame.surfaceTransfers.rain[region] + frame.surfaceTransfers.snowfall[region]) / area * 86400 / frame.intervalSeconds
    : frame.runoffTransfers.sent[region] / 1000 / frame.intervalSeconds;
}

/** Fixed logarithmic display scales do not renormalize on every frame. Values
 * above the visible range saturate in color only; the inspector keeps the stock. */
export function moistureLayerColor(value: number, layer: MoistureLayer): number {
  return Math.min(1, Math.log1p(value) / Math.log1p(MOISTURE_LAYERS[layer].maximum));
}

export function moistureCalendar(seconds: number): string {
  const day = Math.floor(seconds / 86400), hour = Math.floor(seconds % 86400 / 3600), minute = Math.floor(seconds % 3600 / 60);
  return `Year ${Math.floor(day / 365) + 1} · day ${day % 365 + 1} · ${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')} · elapsed ${(seconds / 86400).toFixed(3)} days`;
}
