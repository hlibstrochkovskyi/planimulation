import type { Surface } from '../core/surface';
import { normalize, readVector } from '../core/vector';
import type { Vec3 } from '../core/vector';
import { projectArc } from './projection';

const ARC_STEPS = 4;
const ROUTE_LIFT = 1.0022;

export interface RouteOverlay {
  /** Flat map segment endpoints (x, y, z pairs). */
  flat: Float32Array;
  /** Globe unit-direction segment endpoints and radial height offsets. */
  globe: { positions: Float32Array; offsets: Float32Array };
}

/**
 * Display-only least-time foot route polyline between origin and target regions.
 * Subdivides each spherical edge arc and splits across the flat-map antimeridian seam.
 */
export function buildRouteOverlay(
  surface: Surface,
  elevation: Float64Array,
  bodyIds: Uint32Array,
  waterLevelMeters: number,
  path: readonly number[],
): RouteOverlay {
  if (path.length < 2) {
    return { flat: new Float32Array(0), globe: { positions: new Float32Array(0), offsets: new Float32Array(0) } };
  }
  const flat: number[] = [];
  const positions: number[] = [];
  const offsets: number[] = [];
  const radius = surface.radiusMeters;
  const height = (id: number): number => (bodyIds[id] !== 0 ? waterLevelMeters : elevation[id]);

  for (let step = 0; step + 1 < path.length; step++) {
    const u = path[step], v = path[step + 1];
    const a: Vec3 = readVector(surface.centers, u);
    const b: Vec3 = readVector(surface.centers, v);
    const ha = height(u), hb = height(v);
    let prev = a, prevH = ha;
    for (let k = 1; k <= ARC_STEPS; k++) {
      const t = k / ARC_STEPS;
      const cur = normalize([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]);
      const curH = ha + (hb - ha) * t;
      for (const [p, q] of projectArc(prev, cur)) {
        flat.push(p[0] * 2 - 1, 0.5 - p[1], 0.0008, q[0] * 2 - 1, 0.5 - q[1], 0.0008);
      }
      positions.push(prev[0] * ROUTE_LIFT, prev[1] * ROUTE_LIFT, prev[2] * ROUTE_LIFT, cur[0] * ROUTE_LIFT, cur[1] * ROUTE_LIFT, cur[2] * ROUTE_LIFT);
      offsets.push(prevH / radius, curH / radius);
      prev = cur;
      prevH = curH;
    }
  }
  return {
    flat: Float32Array.from(flat),
    globe: { positions: Float32Array.from(positions), offsets: Float32Array.from(offsets) },
  };
}

/** Origin marker in the same coordinates as the two route overlays. */
export function routeOrigin(
  surface: Surface,
  elevation: Float64Array,
  water: { bodyIds: Uint32Array; levelMeters: number },
  id: number,
): { flat: Float32Array; globe: Float32Array; offset: Float32Array } {
  const [x, y, z] = readVector(surface.centers, id);
  const height = water.bodyIds[id] ? water.levelMeters : elevation[id];
  return {
    flat: Float32Array.of(Math.atan2(z, x) / Math.PI, Math.asin(Math.max(-1, Math.min(1, y))) / Math.PI, 0.001),
    globe: Float32Array.of(x * ROUTE_LIFT, y * ROUTE_LIFT, z * ROUTE_LIFT),
    offset: Float32Array.of(height / surface.radiusMeters),
  };
}
