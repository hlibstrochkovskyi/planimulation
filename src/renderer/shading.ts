import type { Surface } from '../core/surface';
import { cross, dot, normalize, readVector } from '../core/vector';
import type { Vec3 } from '../core/vector';

/** Local east and north unit vectors; longitude increases toward +Z and north is +Y. */
export function localFrame(p: Vec3): { east: Vec3; north: Vec3 } {
  const horizontal = Math.hypot(p[0], p[2]);
  const east: Vec3 = horizontal < 1e-12 ? [1, 0, 0] : [-p[2] / horizontal, 0, p[0] / horizontal];
  return { east, north: cross(east, p) };
}

/**
 * Display-only hillshade gradients. Each region fits a least-squares plane to its
 * neighbors in the local east/north frame. Land treats neighboring water as the
 * water surface and water treats neighboring land as that surface, so coasts do
 * not produce artificial cliffs. Values are dimensionless slopes multiplied by
 * `scale`, chosen so that strong dry-land slopes shade visibly at any resolution.
 */
export function reliefGradients(surface: Surface, elevation: Float64Array, wet: ArrayLike<number>, levelMeters: number):
  { gradients: Float32Array; scale: number } {
  const n = elevation.length;
  if (surface.areasSquareMeters.length !== n || wet.length !== n || !Number.isFinite(levelMeters)) throw new Error('Invalid relief shading input.');
  const raw = new Float64Array(n * 2);
  for (let i = 0; i < n; i++) {
    const p = readVector(surface.centers, i), { east, north } = localFrame(p);
    const height = (j: number): number => wet[i] ? Math.min(elevation[j], levelMeters) : Math.max(elevation[j], levelMeters);
    const own = wet[i] ? elevation[i] : Math.max(elevation[i], levelMeters);
    let ee = 0, en = 0, nn = 0, eh = 0, nh = 0;
    for (let k = surface.neighborOffsets[i]; k < surface.neighborOffsets[i + 1]; k++) {
      const j = surface.neighbors[k], q = readVector(surface.centers, j), along = dot(p, q);
      const tangent: Vec3 = [q[0] - p[0] * along, q[1] - p[1] * along, q[2] - p[2] * along];
      if (Math.hypot(...tangent) < 1e-15) continue;
      const t = normalize(tangent), distance = surface.neighborDistancesMeters[k];
      const e = dot(t, east) * distance, nd = dot(t, north) * distance, dh = height(j) - own;
      ee += e * e; en += e * nd; nn += nd * nd; eh += e * dh; nh += nd * dh;
    }
    const determinant = ee * nn - en * en;
    if (determinant > 0) {
      raw[i * 2] = (eh * nn - nh * en) / determinant;
      raw[i * 2 + 1] = (nh * ee - eh * en) / determinant;
    }
  }
  // Bathymetric slopes stay weak so that blended coastal corners do not darken land.
  for (let i = 0; i < n; i++) if (wet[i]) { raw[i * 2] *= .25; raw[i * 2 + 1] *= .25; }
  const land: number[] = [];
  for (let i = 0; i < n; i++) if (!wet[i]) land.push(Math.hypot(raw[i * 2], raw[i * 2 + 1]));
  land.sort((a, b) => a - b);
  const reference = land.length ? land[Math.floor((land.length - 1) * 0.9)] : 0;
  const scale = reference > 0 ? Math.min(1e5, Math.max(1, 0.9 / reference)) : 1;
  return { gradients: Float32Array.from(raw, value => value * scale), scale };
}

/**
 * Display-only signed wet mask (-1 wet, +1 dry) relaxed over neighbors so the
 * interpolated shoreline rounds region corners. Every region keeps its own sign
 * with at least `floor` magnitude, so single-region islands and lakes stay visible.
 */
export function relaxedMask(surface: Surface, wet: ArrayLike<number>, iterations = 2, floor = .3): Float32Array {
  const n = wet.length;
  let current = Float32Array.from({ length: n }, (_, i) => wet[i] ? -1 : 1), next = new Float32Array(n);
  for (let iteration = 0; iteration < iterations; iteration++) {
    for (let i = 0; i < n; i++) {
      const start = surface.neighborOffsets[i], end = surface.neighborOffsets[i + 1];
      let sum = 0;
      for (let k = start; k < end; k++) sum += current[surface.neighbors[k]];
      next[i] = .5 * current[i] + .5 * sum / Math.max(1, end - start);
    }
    [current, next] = [next, current];
  }
  for (let i = 0; i < n; i++) {
    const sign = wet[i] ? -1 : 1;
    current[i] = sign * Math.max(floor, sign * current[i]);
  }
  return current;
}
