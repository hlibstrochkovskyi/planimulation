import type { Surface } from './surface';
import type { Terrain } from './terrain';
import type { WaterSettings } from './recipe';

/** Initial state at a shared reference-datum level; disconnected bodies are not communicating reservoirs. */
export interface Water {
  levelMeters: number;
  resolvedVolumeCubicMeters: number;
  depthMeters: Float64Array;
  bodyIds: Uint32Array;
  mainOceanId: number;
}

export function summarizeWater(surface: Surface, water: Water) {
  const bodies = new Map<number, { id: number; regionCount: number; areaSquareMeters: number; volumeCubicMeters: number }>();
  let total = 0, wet = 0, ocean = 0, maximumDepthMeters = 0;
  for (let id = 0; id < water.bodyIds.length; id++) {
    const area = surface.areasSquareMeters[id], bodyId = water.bodyIds[id]; total += area;
    if (!bodyId) continue;
    const body = bodies.get(bodyId) ?? { id: bodyId, regionCount: 0, areaSquareMeters: 0, volumeCubicMeters: 0 };
    body.regionCount++; body.areaSquareMeters += area; body.volumeCubicMeters += area * water.depthMeters[id];
    bodies.set(bodyId, body); wet += area;
    if (bodyId === water.mainOceanId) ocean += area;
    maximumDepthMeters = Math.max(maximumDepthMeters, water.depthMeters[id]);
  }
  return { levelMeters: water.levelMeters, resolvedVolumeCubicMeters: water.resolvedVolumeCubicMeters,
    waterAreaFraction: wet / total, mainOceanAreaFraction: ocean / total,
    inlandWaterAreaFraction: Math.max(0, wet - ocean) / total, mainOceanId: water.mainOceanId,
    bodyCount: bodies.size, maximumDepthMeters, bodies: [...bodies.values()] };
}

/** Validate transported stocks and canonical connected components before publishing a world. */
export function validateWater(surface: Surface, terrain: Terrain, water: Water, settings: WaterSettings): void {
  const n = terrain.elevation.length;
  const fail = (): never => { throw new Error('Invalid native water state.'); };
  if (water.depthMeters.length !== n || water.bodyIds.length !== n || !Number.isFinite(water.levelMeters)
    || !Number.isFinite(water.resolvedVolumeCubicMeters) || water.resolvedVolumeCubicMeters < 0) fail();
  let volume = 0;
  for (let i = 0; i < n; i++) {
    const depth = Math.max(0, water.levelMeters - terrain.elevation[i]);
    if (!Number.isFinite(water.depthMeters[i]) || water.depthMeters[i] < 0
      || Math.abs(water.depthMeters[i] - depth) > 1e-8 || (water.bodyIds[i] > 0) !== (depth > 0)
      || (water.bodyIds[i] > 0) !== (water.depthMeters[i] > 0)) fail();
    volume += surface.areasSquareMeters[i] * water.depthMeters[i];
  }
  const close = (a: number, b: number): boolean => Math.abs(a - b) <= Math.max(1e-9, b * 1e-10);
  if (!close(volume, water.resolvedVolumeCubicMeters)
    || (settings.mode === 'volume' && !close(volume, settings.volumeCubicMeters))) fail();
  const visited = new Uint8Array(n), queue = new Uint32Array(n);
  let count = 0, largest = 0, main = 0;
  for (let start = 0; start < n; start++) {
    if (visited[start] || water.bodyIds[start] === 0) continue;
    count++; visited[start] = 1; queue[0] = start;
    let cursor = 0, size = 1, area = 0;
    while (cursor < size) {
      const id = queue[cursor++]; area += surface.areasSquareMeters[id];
      if (water.bodyIds[id] !== count) fail();
      for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
        const next = surface.neighbors[k];
        if (!visited[next] && water.bodyIds[next] > 0) { visited[next] = 1; queue[size++] = next; }
      }
    }
    if (area > largest) { largest = area; main = count; }
  }
  if (water.mainOceanId !== main) fail();
}
