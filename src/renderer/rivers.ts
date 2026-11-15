import type { Surface } from '../core/surface';
import type { Drainage } from '../core/drainage';
import { midpoint, normalize, readVector } from '../core/vector';
import type { Vec3 } from '../core/vector';
import { projectArc } from './projection';

const CURVE_STEPS = 4;

function extrapolate(from: Vec3, toward: Vec3): Vec3 {
  return normalize([2 * from[0] - toward[0], 2 * from[1] - toward[1], 2 * from[2] - toward[2]]);
}

/** Uniform Catmull-Rom between p1 and p2, projected back to the unit sphere. */
function catmullRom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: number): Vec3 {
  const t2 = t * t, t3 = t2 * t;
  const blend = (a: number, b: number, c: number, d: number): number =>
    .5 * (2 * b + (c - a) * t + (2 * a - 5 * b + 4 * c - d) * t2 + (3 * b - a - 3 * c + d) * t3);
  return normalize([blend(p0[0], p1[0], p2[0], p3[0]), blend(p0[1], p1[1], p2[1], p3[1]), blend(p0[2], p1[2], p2[2], p3[2])]);
}

/** Line widths in CSS pixels for increasing contributing-area classes. */
export const DRAINAGE_LINE_WIDTHS = [1, 1.7, 2.6] as const;

export interface DrainageLines {
  /** Minimum contributing area of each width class, in m². */
  thresholdsSquareMeters: number[];
  /** Flat map segment endpoints (x, y, z pairs) per width class. */
  flat: Float32Array[];
  /** Globe unit-direction segment endpoints and their radial height offsets per width class. */
  globe: { positions: Float32Array; offsets: Float32Array }[];
}

/**
 * Display-only drainage lines along static bed receivers. Width follows the
 * contributing dry area, not discharge: no runoff, climate or channel hydraulics
 * are implied. Lines entering water end at the shared region boundary.
 */
export function buildDrainageLines(surface: Surface, drainage: Drainage, elevation: Float64Array, wet: ArrayLike<number>): DrainageLines {
  const n = elevation.length;
  if (surface.areasSquareMeters.length !== n || drainage.receivers.length !== n || wet.length !== n) throw new Error('Invalid drainage display input.');
  const meanArea = surface.areasSquareMeters.reduce((sum, area) => sum + area, 0) / n;
  // Show basins of at least ~100,000 km² (or ten regions on coarse meshes).
  const minimum = Math.max(10 * meanArea, 1e11);
  const thresholds = [minimum, minimum * 5, minimum * 25];
  const flat: number[][] = thresholds.map(() => []), globe = thresholds.map(() => ({ positions: [] as number[], offsets: [] as number[] }));
  const radius = surface.radiusMeters;
  // The largest upstream donor continues a river's main stem through each node.
  const mainDonor = new Int32Array(n).fill(-1);
  for (let i = 0; i < n; i++) {
    const r = drainage.receivers[i];
    if (r === i || wet[i] || drainage.contributingArea[i] < minimum) continue;
    const current = mainDonor[r];
    if (current < 0 || drainage.contributingArea[i] > drainage.contributingArea[current]) mainDonor[r] = i;
  }
  const center = (i: number): Vec3 => readVector(surface.centers, i);
  for (let i = 0; i < n; i++) {
    const r = drainage.receivers[i], area = drainage.contributingArea[i];
    if (wet[i] || r === i || area < minimum) continue;
    const width = area >= thresholds[2] ? 2 : area >= thresholds[1] ? 1 : 0;
    const a = center(i), receiver = center(r);
    const b: Vec3 = wet[r] ? midpoint(a, receiver) : receiver;
    const hb = wet[r] ? (elevation[i] + elevation[r]) / 2 : elevation[r];
    // Catmull-Rom through neighboring nodes keeps joined segments continuous.
    const before = mainDonor[i] >= 0 ? center(mainDonor[i]) : extrapolate(a, b);
    const rr = drainage.receivers[r];
    const after = wet[r] || rr === r ? extrapolate(b, a) : center(rr);
    const points: Vec3[] = [], heights: number[] = [];
    for (let step = 0; step <= CURVE_STEPS; step++) {
      const t = step / CURVE_STEPS;
      points.push(catmullRom(before, a, b, after, t)); heights.push(elevation[i] + (hb - elevation[i]) * t);
    }
    const target = globe[width], lift = 1.0003;
    for (let k = 0; k < CURVE_STEPS; k++) {
      const p = points[k], q = points[k + 1];
      for (const [x, y] of projectArc(p, q)) flat[width].push(x[0] * 2 - 1, .5 - x[1], .0004, y[0] * 2 - 1, .5 - y[1], .0004);
      target.positions.push(...p.map(v => v * lift), ...q.map(v => v * lift));
      target.offsets.push(heights[k] / radius, heights[k + 1] / radius);
    }
  }
  return { thresholdsSquareMeters: thresholds, flat: flat.map(v => Float32Array.from(v)),
    globe: globe.map(g => ({ positions: Float32Array.from(g.positions), offsets: Float32Array.from(g.offsets) })) };
}
