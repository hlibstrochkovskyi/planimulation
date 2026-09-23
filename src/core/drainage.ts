import type { Surface } from './surface';
import type { Water } from './water';

export interface Drainage {
  receivers: Uint32Array;
  outlets: Uint32Array;
  flatSteps: Uint32Array;
  contributingArea: Float64Array;
}

/** Graph and budget validation, independent of the native flat-routing implementation. */
export function validateDrainage(s: Surface, heights: Float64Array, w: Water, d: Drainage): void {
  const n = heights.length, fail = (): never => { throw new Error('Invalid native drainage state.'); };
  if (Object.values(d).some((array) => array.length !== n)) fail();
  const incoming = new Uint32Array(n), area = new Float64Array(n), roots = new Map<number, number>();
  for (let i = 0; i < n; i++) if (w.bodyIds[i] && !roots.has(w.bodyIds[i])) roots.set(w.bodyIds[i], i);
  for (let i = 0; i < n; i++) {
    const r = d.receivers[i];
    if (r >= n || d.outlets[i] >= n || d.flatSteps[i] >= n || !Number.isFinite(d.contributingArea[i]) || d.contributingArea[i] < 0) fail();
    if (w.bodyIds[i]) { if (r !== i || d.flatSteps[i] !== 0) fail(); }
    else area[i] = s.areasSquareMeters[i];
    if (r === i) {
      if (d.flatSteps[i] !== 0 || (!w.bodyIds[i] && s.neighbors.subarray(s.neighborOffsets[i], s.neighborOffsets[i + 1]).some((j) => heights[j] < heights[i]))) fail();
      continue;
    }
    if (!s.neighbors.subarray(s.neighborOffsets[i], s.neighborOffsets[i + 1]).includes(r) || heights[r] > heights[i]) fail();
    if (heights[r] === heights[i] ? d.flatSteps[i] !== d.flatSteps[r] + 1 : d.flatSteps[i] !== 0) fail();
    incoming[r]++;
  }
  const order: number[] = [];
  for (let i = 0; i < n; i++) if (!incoming[i]) order.push(i);
  for (let cursor = 0; cursor < order.length; cursor++) {
    const i = order[cursor], r = d.receivers[i];
    if (r === i) continue;
    area[r] += area[i]; if (--incoming[r] === 0) order.push(r);
  }
  if (order.length !== n) fail();
  for (let j = order.length - 1; j >= 0; j--) {
    const i = order[j], r = d.receivers[i];
    const outlet = w.bodyIds[i] ? roots.get(w.bodyIds[i]) : r === i ? i : d.outlets[r];
    if (d.outlets[i] !== outlet || Math.abs(area[i] - d.contributingArea[i]) > Math.max(1e-6, area[i] * 1e-10)) fail();
  }
}

export function summarizeDrainage(s: Surface, w: Water, d: Drainage) {
  const catchments = new Map<number, { outletRegion: number; waterBodyId: number; landAreaSquareMeters: number }>();
  let land = 0, closed = 0, maximum = 0, flatRoutedRegions = 0;
  for (let i = 0; i < d.outlets.length; i++) {
    const outlet = d.outlets[i];
    const entry = catchments.get(outlet) ?? { outletRegion: outlet, waterBodyId: w.bodyIds[outlet], landAreaSquareMeters: 0 };
    if (!w.bodyIds[i]) {
      entry.landAreaSquareMeters += s.areasSquareMeters[i]; land += s.areasSquareMeters[i];
      if (!entry.waterBodyId) closed += s.areasSquareMeters[i];
    }
    catchments.set(outlet, entry);
    maximum = Math.max(maximum, d.contributingArea[i]);
    if (d.flatSteps[i]) flatRoutedRegions++;
  }
  return { catchmentCount: catchments.size, closedSinkCount: [...catchments.values()].filter((c) => !c.waterBodyId).length,
    dryLandAreaSquareMeters: land, closedDrainageLandFraction: land ? closed / land : 0,
    maximumContributingAreaSquareMeters: maximum, flatRoutedRegions, catchments: [...catchments.values()] };
}
