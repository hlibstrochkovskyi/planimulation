import { Color, ShapeUtils, Vector2 } from 'three';
import type { Surface } from '../core/surface';
import { angularDistance, cross, dot, normalize, readVector } from '../core/vector';
import type { Vec3 } from '../core/vector';
import { projectArc, projectRegion } from './projection';
import type { Tectonics } from '../core/tectonics';
import { BOUNDARY_COLORS } from '../core/tectonics';
import type { Water } from '../core/water';
import { buildWaterSurface } from './water-surface';

/**
 * Each triangle lies inside one region's fan (center, corner A, corner B). `cornerPairs`
 * names those two dual corners and `weights` gives each vertex's (center, A, B)
 * barycentric weights, so the shader can interpolate display values between region
 * centers and shared corners. Interpolation is display-only; picking keeps `regions`.
 */
export interface ViewGeometry { positions: Float32Array; regions: Float32Array; lines: Float32Array;
  cornerPairs: Float32Array; weights: Float32Array;
  tectonicLines: Float32Array; tectonicColors: Float32Array;
  radialOffsets: Float32Array; lineOffsets: Float32Array; tectonicOffsets: Float32Array;
  waterPositions: Float32Array; waterRegions: Float32Array; waterOffsets: Float32Array;
  waterCornerPairs: Float32Array; waterWeights: Float32Array;
  waterLines: Float32Array; waterLineOffsets: Float32Array }
/** Shared dual corners and their incident regions, in compressed-row form. */
export interface ViewCorners { offsets: Uint32Array; regions: Uint32Array }
export interface ViewPair { flat: ViewGeometry; globe: ViewGeometry; corners: ViewCorners }

type Weights = readonly [number, number, number];
interface MapPoint { x: number; y: number; w: Weights }
const TAU = Math.PI * 2;

function clip(points: MapPoint[], bound: number, greater: boolean): MapPoint[] {
  const result: MapPoint[] = [];
  for (let i = 0; i < points.length; i++) {
    const a = points[i], b = points[(i + 1) % points.length];
    const insideA = greater ? a.x >= bound : a.x <= bound;
    const insideB = greater ? b.x >= bound : b.x <= bound;
    if (insideA) result.push(a);
    if (insideA !== insideB) {
      const t = (bound - a.x) / (b.x - a.x);
      result.push({ x: bound, y: a.y + (b.y - a.y) * t,
        w: [a.w[0] + (b.w[0] - a.w[0]) * t, a.w[1] + (b.w[1] - a.w[1]) * t, a.w[2] + (b.w[2] - a.w[2]) * t] });
    }
  }
  return result;
}

/** Gnomonic barycentric weights of a direction inside a spherical triangle. */
function barycentric(c: Vec3, a: Vec3, b: Vec3, p: Vec3): Weights {
  const wc = dot(p, cross(a, b)), wa = dot(c, cross(p, b)), wb = dot(c, cross(a, p)), sum = wc + wa + wb;
  return [wc / sum, wa / sum, wb / sum];
}

function longitudeNear(longitude: number, previous: number): number {
  while (longitude - previous > Math.PI) longitude -= TAU;
  while (longitude - previous < -Math.PI) longitude += TAU;
  return longitude;
}

/** Project one fan triangle to unwrapped equirectangular points, capping a contained pole. */
function projectFan(c: Vec3, a: Vec3, b: Vec3): MapPoint[] {
  const points: Vec3[] = [];
  for (const [p, q] of [[c, a], [a, b], [b, c]] as const) {
    const steps = Math.max(1, Math.ceil(angularDistance(p, q) / 0.04));
    for (let step = 0; step < steps; step++) {
      const t = step / steps;
      points.push(normalize([p[0] * (1 - t) + q[0] * t, p[1] * (1 - t) + q[1] * t, p[2] * (1 - t) + q[2] * t]));
    }
  }
  const isPole = (p: Vec3): boolean => Math.hypot(p[0], p[2]) < 1e-10;
  while (isPole(points[0])) points.push(points.shift()!);
  const weight = (p: Vec3): Weights => barycentric(c, a, b, p);
  const projected: { longitude: number; latitude: number; w: Weights }[] = [];
  for (let i = 0; i < points.length; i++) {
    const p = points[i], w = weight(p);
    const previous = projected.length ? projected[projected.length - 1].longitude : Math.atan2(p[2], p[0]);
    if (isPole(p)) {
      const next = points[(i + 1) % points.length];
      const latitude = Math.sign(p[1]) * Math.PI / 2;
      projected.push({ longitude: previous, latitude, w },
        { longitude: longitudeNear(Math.atan2(next[2], next[0]), previous), latitude, w });
    } else {
      projected.push({ longitude: longitudeNear(Math.atan2(p[2], p[0]), previous), latitude: Math.asin(Math.max(-1, Math.min(1, p[1]))), w });
    }
  }
  const first = projected[0];
  const closingLongitude = longitudeNear(first.longitude, projected[projected.length - 1].longitude);
  if (Math.abs(closingLongitude - first.longitude) > Math.PI) {
    const sign = Math.sign(c[1] + a[1] + b[1]), pole = weight([0, sign, 0]), latitude = sign * Math.PI / 2;
    projected.push({ longitude: closingLongitude, latitude: first.latitude, w: first.w },
      { longitude: closingLongitude, latitude, w: pole }, { longitude: first.longitude, latitude, w: pole });
  }
  return projected.map(({ longitude, latitude, w }) => ({ x: longitude / TAU + 0.5, y: 0.5 - latitude / Math.PI, w }));
}

/** Display-only triangulation. Clipping must not create new simulation regions. */
export function buildViewGeometry(surface: Surface, tectonics?: Tectonics, elevation?: Float64Array, water?: Water): ViewPair {
  if (elevation && elevation.length !== surface.areasSquareMeters.length) throw new Error('Invalid elevation field length.');
  // Every shared dual corner uses the same mean of its incident region heights.
  // This interpolation belongs to the viewer, not to the physical model.
  const shared = new Map<string, { sum: number; count: number }>();
  const key = (v: readonly number[]): string => v.join(',');
  if (elevation) for (let id = 0; id < elevation.length; id++) {
    for (let j = surface.boundaryOffsets[id]; j < surface.boundaryOffsets[id + 1]; j++) {
      const k = key(readVector(surface.boundaryDirections, j));
      const value = shared.get(k) ?? { sum: 0, count: 0 };
      value.sum += elevation[id]; value.count++; shared.set(k, value);
    }
  }
  const corner = (v: readonly number[]): number => {
    if (!elevation) return 0;
    const value = shared.get(key(v));
    if (!value) throw new Error('Missing shared relief corner.');
    return value.sum / value.count / surface.radiusMeters;
  };
  // Dual corners are identical directions in every incident region's boundary ring.
  const cornerIndex = new Map<string, number>(), incident: number[][] = [];
  const cornerOf = (j: number): number => {
    const k = key(readVector(surface.boundaryDirections, j));
    let index = cornerIndex.get(k);
    if (index === undefined) { index = incident.length; cornerIndex.set(k, index); incident.push([]); }
    return index;
  };
  const boundaryCorners = new Uint32Array(surface.boundaryDirections.length / 3);
  for (let id = 0; id < surface.areasSquareMeters.length; id++) {
    for (let j = surface.boundaryOffsets[id]; j < surface.boundaryOffsets[id + 1]; j++) {
      boundaryCorners[j] = cornerOf(j); incident[boundaryCorners[j]].push(id);
    }
  }
  const cornerOffsets = new Uint32Array(incident.length + 1);
  for (let k = 0; k < incident.length; k++) cornerOffsets[k + 1] = cornerOffsets[k] + incident[k].length;
  const corners: ViewCorners = { offsets: cornerOffsets, regions: Uint32Array.from(incident.flat()) };
  const radialOffsets: number[] = [], lineOffsets: number[] = [], tectonicOffsets: number[] = [];
  const flat = { positions: [] as number[], regions: [] as number[], lines: [] as number[], cornerPairs: [] as number[], weights: [] as number[], tectonicLines: [] as number[], tectonicColors: [] as number[] };
  const globe = { positions: [] as number[], regions: [] as number[], lines: [] as number[], cornerPairs: [] as number[], weights: [] as number[], tectonicLines: [] as number[], tectonicColors: [] as number[] };
  for (let id = 0; id < surface.areasSquareMeters.length; id++) {
    const polygon = projectRegion(surface, id);
    for (const shift of [-1, 0, 1]) {
      const clipped = clip(clip(polygon.map(([x, y]) => ({ x: x + shift, y, w: [0, 0, 0] as Weights })), 0, true), 1, false);
      for (let j = 0; j < clipped.length; j++) {
        const a = clipped[j], b = clipped[(j + 1) % clipped.length];
        flat.lines.push(a.x * 2 - 1, .5 - a.y, .0001, b.x * 2 - 1, .5 - b.y, .0001);
      }
    }
    const center = readVector(surface.centers, id);
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
    for (let j = start; j < end; j++) {
      const next = j + 1 === end ? start : j + 1;
      const a = readVector(surface.boundaryDirections, j);
      const b = readVector(surface.boundaryDirections, next);
      const pair = [boundaryCorners[j], boundaryCorners[next]];
      const fan = projectFan(center, a, b);
      for (const shift of [-1, 0, 1]) {
        const clipped = clip(clip(fan.map(p => ({ ...p, x: p.x + shift })), 0, true), 1, false);
        if (clipped.length < 3) continue;
        const contour = clipped.map(p => new Vector2(p.x * 2 - 1, .5 - p.y));
        for (const triangle of ShapeUtils.triangulateShape(contour, [])) {
          for (const vertex of triangle) {
            flat.positions.push(contour[vertex].x, contour[vertex].y, 0); flat.regions.push(id);
            flat.cornerPairs.push(...pair); flat.weights.push(...clipped[vertex].w);
          }
        }
      }
      globe.positions.push(...center, ...a, ...b); globe.regions.push(id, id, id);
      globe.cornerPairs.push(...pair, ...pair, ...pair); globe.weights.push(1, 0, 0, 0, 1, 0, 0, 0, 1);
      const ha = corner(a), hb = corner(b);
      radialOffsets.push((elevation?.[id] ?? 0) / surface.radiusMeters, ha, hb);
      lineOffsets.push(ha, hb);
      globe.lines.push(...a.map((v) => v * 1.0002), ...b.map((v) => v * 1.0002));
    }
  }
  if (tectonics) for (let i = 0; i < tectonics.boundaryTypes.length; i++) {
    const a = readVector(tectonics.boundaryDirections, i * 2), b = readVector(tectonics.boundaryDirections, i * 2 + 1);
    const color = new Color(BOUNDARY_COLORS[tectonics.boundaryTypes[i]]);
    const rgb = [color.r, color.g, color.b];
    globe.tectonicLines.push(...a.map((v) => v * 1.0008), ...b.map((v) => v * 1.0008));
    tectonicOffsets.push(corner(a), corner(b));
    globe.tectonicColors.push(...rgb, ...rgb);
    for (const [p, q] of projectArc(a, b)) {
      flat.tectonicLines.push(p[0] * 2 - 1, .5 - p[1], .0003, q[0] * 2 - 1, .5 - q[1], .0003);
      flat.tectonicColors.push(...rgb, ...rgb);
    }
  }
  const pack = (data: typeof flat): ViewGeometry => ({ positions: Float32Array.from(data.positions),
    regions: Float32Array.from(data.regions), lines: Float32Array.from(data.lines),
    cornerPairs: Float32Array.from(data.cornerPairs), weights: Float32Array.from(data.weights),
    tectonicLines: Float32Array.from(data.tectonicLines), tectonicColors: Float32Array.from(data.tectonicColors),
    radialOffsets: new Float32Array(0), lineOffsets: new Float32Array(0), tectonicOffsets: new Float32Array(0),
    waterPositions: new Float32Array(0), waterRegions: new Float32Array(0), waterOffsets: new Float32Array(0),
    waterCornerPairs: new Float32Array(0), waterWeights: new Float32Array(0),
    waterLines: new Float32Array(0), waterLineOffsets: new Float32Array(0) });
  const globeView = { ...pack(globe), radialOffsets: Float32Array.from(radialOffsets),
    lineOffsets: Float32Array.from(lineOffsets), tectonicOffsets: Float32Array.from(tectonicOffsets) };
  if (water) Object.assign(globeView, buildWaterSurface(globeView.positions, globeView.regions, water, surface.radiusMeters,
    { cornerPairs: globeView.cornerPairs, weights: globeView.weights }));
  return { flat: pack(flat), globe: globeView, corners };
}
