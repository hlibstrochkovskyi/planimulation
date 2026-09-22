import type { Surface } from './surface';

export interface Crust {
  threshold: number;
  potential: Float64Array;
  continentality: Float64Array;
  thicknessMeters: Float64Array;
  densityKgPerCubicMeter: Float64Array;
}

/** Read-only diagnostics. Continental-dominant crust is not emerged land. */
export function summarizeCrust(surface: Surface, crust: Crust): {
  continentalAreaFraction: number; meanContinentality: number;
  continentalPatchCount: number; largestContinentalPatchAreaSquareMeters: number;
  threshold: number;
} {
  let total = 0, continental = 0, weighted = 0, patchCount = 0, largest = 0;
  const visited = new Uint8Array(crust.continentality.length);
  for (let id = 0; id < visited.length; id++) {
    const area = surface.areasSquareMeters[id], c = crust.continentality[id];
    total += area; weighted += c * area;
    if (c > 0.5) continental += area;
    if (visited[id] || c <= 0.5) continue;
    patchCount++;
    let patchArea = 0;
    const queue = [id]; visited[id] = 1;
    for (let k = 0; k < queue.length; k++) {
      const cell = queue[k]; patchArea += surface.areasSquareMeters[cell];
      for (let j = surface.neighborOffsets[cell]; j < surface.neighborOffsets[cell + 1]; j++) {
        const next = surface.neighbors[j];
        if (!visited[next] && crust.continentality[next] > 0.5) { visited[next] = 1; queue.push(next); }
      }
    }
    largest = Math.max(largest, patchArea);
  }
  return { continentalAreaFraction: continental / total, meanContinentality: weighted / total,
    continentalPatchCount: patchCount, largestContinentalPatchAreaSquareMeters: largest, threshold: crust.threshold };
}
