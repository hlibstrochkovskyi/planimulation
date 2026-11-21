import { Color, ShapeUtils, Vector2 } from 'three';
import type { Surface } from '../core/surface';
import { angularDistance, cross, dot, normalize, readVector } from '../core/vector';
import type { Vec3 } from '../core/vector';
import { projectArc, projectRegion } from './projection';
import type { Tectonics } from '../core/tectonics';
import { BOUNDARY_COLORS } from '../core/tectonics';
import type { Water } from '../core/water';
import type { WaterFrame } from '../shared/desktop-api';
import type { RegionalWaterDisplay } from './regional-water';
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
  waterLines: Float32Array; waterLineOffsets: Float32Array;
  indices?: Uint32Array; waterIndices?: Uint32Array; landCoastIndexCount?: number }
/** Shared dual corners and their incident regions, in compressed-row form. */
export interface ViewCorners { offsets: Uint32Array; regions: Uint32Array; boundaryCorners?: Uint32Array }
export interface CompactDualRing { ringOffsets: Uint32Array; ringCorners: Uint32Array; ringDirections: Float32Array }
export interface ViewPair { flat: ViewGeometry; globe: ViewGeometry; corners: ViewCorners; compactRing?: CompactDualRing }

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

/**
 * Fast allocation-free numeric corner deduplication over `surface.boundaryDirections`.
 * Dual corners are bit-exact identical Float64 unit directions across incident regions.
 */
function deduplicateBoundaryCorners(surface: Surface): {
  boundaryCorners: Uint32Array;
  cornerOffsets: Uint32Array;
  cornerRegions: Uint32Array;
  cornerCount: number;
  cornerX: Float64Array;
  cornerY: Float64Array;
  cornerZ: Float64Array;
  lookupCorner: (x: number, y: number, z: number) => number;
} {
  const dirs = surface.boundaryDirections;
  const totalBoundary = dirs.length / 3;
  const n = surface.areasSquareMeters.length;
  // Upper bound on unique corners: totalBoundary / 2 + 16
  let cap = 16;
  const target = Math.max(32, totalBoundary * 2);
  while (cap < target) cap <<= 1;
  const mask = cap - 1;
  const table = new Int32Array(cap).fill(-1);
  const maxUnique = Math.max(16, Math.ceil(totalBoundary * 0.75) + 16);
  let cornerX = new Float64Array(maxUnique);
  let cornerY = new Float64Array(maxUnique);
  let cornerZ = new Float64Array(maxUnique);
  let counts = new Uint16Array(maxUnique);
  let cornerCount = 0;

  const hashXYZ = (x: number, y: number, z: number): number => {
    const hx = Math.round(x * 1e7) | 0;
    const hy = Math.round(y * 1e7) | 0;
    const hz = Math.round(z * 1e7) | 0;
    return (Math.imul(hx, 73856093) ^ Math.imul(hy, 19349663) ^ Math.imul(hz, 83492791)) >>> 0;
  };

  const growArrays = (): void => {
    const nextLen = cornerX.length * 2;
    const nx = new Float64Array(nextLen); nx.set(cornerX); cornerX = nx;
    const ny = new Float64Array(nextLen); ny.set(cornerY); cornerY = ny;
    const nz = new Float64Array(nextLen); nz.set(cornerZ); cornerZ = nz;
    const nc = new Uint16Array(nextLen); nc.set(counts); counts = nc;
  };

  const boundaryCorners = new Uint32Array(totalBoundary);
  for (let j = 0; j < totalBoundary; j++) {
    const x = dirs[j * 3], y = dirs[j * 3 + 1], z = dirs[j * 3 + 2];
    let slot = hashXYZ(x, y, z) & mask;
    let cid = -1;
    while (true) {
      const existing = table[slot];
      if (existing === -1) {
        if (cornerCount >= cornerX.length) growArrays();
        cid = cornerCount++;
        cornerX[cid] = x; cornerY[cid] = y; cornerZ[cid] = z;
        table[slot] = cid;
        break;
      }
      if (cornerX[existing] === x && cornerY[existing] === y && cornerZ[existing] === z) {
        cid = existing;
        break;
      }
      slot = (slot + 1) & mask;
    }
    boundaryCorners[j] = cid;
    counts[cid]++;
  }

  const cornerOffsets = new Uint32Array(cornerCount + 1);
  for (let k = 0; k < cornerCount; k++) cornerOffsets[k + 1] = cornerOffsets[k] + counts[k];
  const cornerRegions = new Uint32Array(cornerOffsets[cornerCount]);
  const writeCursor = new Uint32Array(cornerOffsets.subarray(0, cornerCount));
  for (let id = 0; id < n; id++) {
    for (let j = surface.boundaryOffsets[id]; j < surface.boundaryOffsets[id + 1]; j++) {
      const cid = boundaryCorners[j];
      cornerRegions[writeCursor[cid]++] = id;
    }
  }

  const lookupCorner = (x: number, y: number, z: number): number => {
    let slot = hashXYZ(x, y, z) & mask;
    while (true) {
      const existing = table[slot];
      if (existing === -1) return -1;
      if (cornerX[existing] === x && cornerY[existing] === y && cornerZ[existing] === z) return existing;
      slot = (slot + 1) & mask;
    }
  };

  return { boundaryCorners, cornerOffsets, cornerRegions, cornerCount, cornerX, cornerY, cornerZ, lookupCorner };
}

class Float32Builder {
  data: Float32Array;
  length = 0;
  constructor(initialCapacity = 1024) { this.data = new Float32Array(Math.max(16, initialCapacity)); }
  ensure(extra: number): void {
    const needed = this.length + extra;
    if (needed <= this.data.length) return;
    let cap = this.data.length;
    while (cap < needed) cap *= 2;
    const next = new Float32Array(cap);
    next.set(this.data.subarray(0, this.length));
    this.data = next;
  }
  push1(a: number): void { this.ensure(1); this.data[this.length++] = a; }
  push2(a: number, b: number): void { this.ensure(2); const i = this.length; this.data[i] = a; this.data[i + 1] = b; this.length = i + 2; }
  push3(a: number, b: number, c: number): void {
    this.ensure(3); const i = this.length; this.data[i] = a; this.data[i + 1] = b; this.data[i + 2] = c; this.length = i + 3;
  }
  push6(a: number, b: number, c: number, d: number, e: number, f: number): void {
    this.ensure(6); const i = this.length;
    this.data[i] = a; this.data[i + 1] = b; this.data[i + 2] = c;
    this.data[i + 3] = d; this.data[i + 4] = e; this.data[i + 5] = f;
    this.length = i + 6;
  }
  finish(): Float32Array { return this.data.slice(0, this.length); }
}

class Uint32Builder {
  data: Uint32Array;
  length = 0;
  constructor(initialCapacity = 1024) { this.data = new Uint32Array(Math.max(16, initialCapacity)); }
  ensure(extra: number): void {
    const needed = this.length + extra;
    if (needed <= this.data.length) return;
    let cap = this.data.length;
    while (cap < needed) cap *= 2;
    const next = new Uint32Array(cap);
    next.set(this.data.subarray(0, this.length));
    this.data = next;
  }
  push3(a: number, b: number, c: number): void {
    this.ensure(3); const i = this.length; this.data[i] = a; this.data[i + 1] = b; this.data[i + 2] = c; this.length = i + 3;
  }
  finish(): Uint32Array { return this.data.slice(0, this.length); }
}

function buildTectonicLines(
  tectonics: Tectonics | undefined,
  elevation: Float64Array | undefined,
  cornerOffsetByDir: (x: number, y: number, z: number) => number,
): { flatLines: Float32Array; flatColors: Float32Array; globeLines: Float32Array; globeColors: Float32Array; globeOffsets: Float32Array } {
  if (!tectonics || !tectonics.boundaryTypes.length) {
    return {
      flatLines: new Float32Array(0), flatColors: new Float32Array(0),
      globeLines: new Float32Array(0), globeColors: new Float32Array(0), globeOffsets: new Float32Array(0),
    };
  }
  const m = tectonics.boundaryTypes.length;
  const globeLines = new Float32Array(m * 6);
  const globeColors = new Float32Array(m * 6);
  const globeOffsets = new Float32Array(m * 2);
  const flatLines = new Float32Builder(m * 6);
  const flatColors = new Float32Builder(m * 6);
  const palette = BOUNDARY_COLORS.map((hex) => { const c = new Color(hex); return [c.r, c.g, c.b] as const; });
  for (let i = 0; i < m; i++) {
    const a = readVector(tectonics.boundaryDirections, i * 2), b = readVector(tectonics.boundaryDirections, i * 2 + 1);
    const [r, g, bl] = palette[tectonics.boundaryTypes[i]];
    const i6 = i * 6;
    globeLines[i6] = a[0] * 1.0008; globeLines[i6 + 1] = a[1] * 1.0008; globeLines[i6 + 2] = a[2] * 1.0008;
    globeLines[i6 + 3] = b[0] * 1.0008; globeLines[i6 + 4] = b[1] * 1.0008; globeLines[i6 + 5] = b[2] * 1.0008;
    globeColors[i6] = r; globeColors[i6 + 1] = g; globeColors[i6 + 2] = bl;
    globeColors[i6 + 3] = r; globeColors[i6 + 4] = g; globeColors[i6 + 5] = bl;
    globeOffsets[i * 2] = elevation ? cornerOffsetByDir(a[0], a[1], a[2]) : 0;
    globeOffsets[i * 2 + 1] = elevation ? cornerOffsetByDir(b[0], b[1], b[2]) : 0;
    for (const [p, q] of projectArc(a, b)) {
      flatLines.push6(p[0] * 2 - 1, .5 - p[1], .0003, q[0] * 2 - 1, .5 - q[1], .0003);
      flatColors.push6(r, g, bl, r, g, bl);
    }
  }
  return { flatLines: flatLines.finish(), flatColors: flatColors.finish(), globeLines, globeColors, globeOffsets };
}

/** Display-only triangulation. Clipping must not create new simulation regions. */
export function buildViewGeometry(surface: Surface, tectonics?: Tectonics, elevation?: Float64Array, water?: Water): ViewPair {
  const n = surface.areasSquareMeters.length;
  if (elevation && elevation.length !== n) throw new Error('Invalid elevation field length.');
  const { boundaryCorners, cornerOffsets, cornerRegions, cornerCount, lookupCorner } = deduplicateBoundaryCorners(surface);
  const cornerElev = new Float64Array(cornerCount);
  if (elevation) {
    for (let k = 0; k < cornerCount; k++) {
      let sum = 0;
      const start = cornerOffsets[k], end = cornerOffsets[k + 1];
      for (let j = start; j < end; j++) sum += elevation[cornerRegions[j]];
      cornerElev[k] = sum / Math.max(1, end - start) / surface.radiusMeters;
    }
  }
  const corners: ViewCorners = { offsets: cornerOffsets, regions: cornerRegions, boundaryCorners };

  const totalFans = surface.boundaryDirections.length / 3;
  const globePositions = new Float32Array(totalFans * 9);
  const globeRegions = new Float32Array(totalFans * 3);
  const globeCornerPairs = new Float32Array(totalFans * 6);
  const globeWeights = new Float32Array(totalFans * 9);
  const radialOffsets = new Float32Array(totalFans * 3);
  const globeLines = new Float32Array(totalFans * 6);
  const lineOffsets = new Float32Array(totalFans * 2);

  const flatPositions = new Float32Builder(totalFans * 9 + 256);
  const flatRegions = new Float32Builder(totalFans * 3 + 128);
  const flatCornerPairs = new Float32Builder(totalFans * 6 + 256);
  const flatWeights = new Float32Builder(totalFans * 9 + 256);
  const flatLines = new Float32Builder(totalFans * 6 + 256);

  const dirs = surface.boundaryDirections;
  const centers = surface.centers;
  const radius = surface.radiusMeters;
  // Minimum cosine for fast-path edge length (< 0.04 rad)
  const cosEdgeLimit = Math.cos(0.039);

  let fanCursor = 0;
  for (let id = 0; id < n; id++) {
    const cx = centers[id * 3], cy = centers[id * 3 + 1], cz = centers[id * 3 + 2];
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
    const elevC = elevation ? elevation[id] / radius : 0;

    // Fast non-seam, non-polar check for flat region boundary lines and fan triangles.
    let fastRegion = Math.abs(cy) < 0.985;
    let lonMin = Infinity, lonMax = -Infinity;
    if (fastRegion) {
      const cLon = Math.atan2(cz, cx);
      lonMin = lonMax = cLon;
      for (let j = start; j < end; j++) {
        const ax = dirs[j * 3], ay = dirs[j * 3 + 1], az = dirs[j * 3 + 2];
        if (Math.abs(ay) >= 0.985 || (cx * ax + cy * ay + cz * az) < cosEdgeLimit) { fastRegion = false; break; }
        const lon = Math.atan2(az, ax);
        if (lon < lonMin) lonMin = lon;
        if (lon > lonMax) lonMax = lon;
      }
      if (fastRegion && (lonMax - lonMin > 1.5 || lonMin < -Math.PI + 0.05 || lonMax > Math.PI - 0.05)) fastRegion = false;
    }

    if (fastRegion) {
      const cFx = (Math.atan2(cz, cx) / TAU + 0.5) * 2 - 1;
      const cFy = Math.asin(Math.max(-1, Math.min(1, cy))) / Math.PI;
      for (let j = start; j < end; j++) {
        const next = j + 1 === end ? start : j + 1;
        const ax = dirs[j * 3], ay = dirs[j * 3 + 1], az = dirs[j * 3 + 2];
        const bx = dirs[next * 3], by = dirs[next * 3 + 1], bz = dirs[next * 3 + 2];
        const aFx = (Math.atan2(az, ax) / TAU + 0.5) * 2 - 1;
        const aFy = Math.asin(Math.max(-1, Math.min(1, ay))) / Math.PI;
        const bFx = (Math.atan2(bz, bx) / TAU + 0.5) * 2 - 1;
        const bFy = Math.asin(Math.max(-1, Math.min(1, by))) / Math.PI;
        const cA = boundaryCorners[j], cB = boundaryCorners[next];

        flatLines.push6(aFx, aFy, .0001, bFx, bFy, .0001);
        flatPositions.push3(cFx, cFy, 0); flatPositions.push3(aFx, aFy, 0); flatPositions.push3(bFx, bFy, 0);
        flatRegions.push3(id, id, id);
        flatCornerPairs.push6(cA, cB, cA, cB, cA, cB);
        flatWeights.push3(1, 0, 0); flatWeights.push3(0, 1, 0); flatWeights.push3(0, 0, 1);

        const p9 = fanCursor * 9, p6 = fanCursor * 6, p3 = fanCursor * 3, p2 = fanCursor * 2;
        globePositions[p9] = cx; globePositions[p9 + 1] = cy; globePositions[p9 + 2] = cz;
        globePositions[p9 + 3] = ax; globePositions[p9 + 4] = ay; globePositions[p9 + 5] = az;
        globePositions[p9 + 6] = bx; globePositions[p9 + 7] = by; globePositions[p9 + 8] = bz;
        globeRegions[p3] = id; globeRegions[p3 + 1] = id; globeRegions[p3 + 2] = id;
        globeCornerPairs[p6] = cA; globeCornerPairs[p6 + 1] = cB;
        globeCornerPairs[p6 + 2] = cA; globeCornerPairs[p6 + 3] = cB;
        globeCornerPairs[p6 + 4] = cA; globeCornerPairs[p6 + 5] = cB;
        globeWeights[p9] = 1; globeWeights[p9 + 4] = 1; globeWeights[p9 + 8] = 1;
        const ha = cornerElev[cA], hb = cornerElev[cB];
        radialOffsets[p3] = elevC; radialOffsets[p3 + 1] = ha; radialOffsets[p3 + 2] = hb;
        lineOffsets[p2] = ha; lineOffsets[p2 + 1] = hb;
        globeLines[p6] = ax * 1.0002; globeLines[p6 + 1] = ay * 1.0002; globeLines[p6 + 2] = az * 1.0002;
        globeLines[p6 + 3] = bx * 1.0002; globeLines[p6 + 4] = by * 1.0002; globeLines[p6 + 5] = bz * 1.0002;
        fanCursor++;
      }
      continue;
    }

    const polygon = projectRegion(surface, id);
    for (const shift of [-1, 0, 1]) {
      const clipped = clip(clip(polygon.map(([x, y]) => ({ x: x + shift, y, w: [0, 0, 0] as Weights })), 0, true), 1, false);
      for (let j = 0; j < clipped.length; j++) {
        const a = clipped[j], b = clipped[(j + 1) % clipped.length];
        flatLines.push6(a.x * 2 - 1, .5 - a.y, .0001, b.x * 2 - 1, .5 - b.y, .0001);
      }
    }
    const center: Vec3 = [cx, cy, cz];
    for (let j = start; j < end; j++) {
      const next = j + 1 === end ? start : j + 1;
      const a = readVector(dirs, j);
      const b = readVector(dirs, next);
      const cA = boundaryCorners[j], cB = boundaryCorners[next];
      const fan = projectFan(center, a, b);
      for (const shift of [-1, 0, 1]) {
        const clipped = clip(clip(fan.map(p => ({ ...p, x: p.x + shift })), 0, true), 1, false);
        if (clipped.length < 3) continue;
        const contour = clipped.map(p => new Vector2(p.x * 2 - 1, .5 - p.y));
        for (const triangle of ShapeUtils.triangulateShape(contour, [])) {
          for (const vertex of triangle) {
            flatPositions.push3(contour[vertex].x, contour[vertex].y, 0);
            flatRegions.push1(id);
            flatCornerPairs.push2(cA, cB);
            const w = clipped[vertex].w;
            flatWeights.push3(w[0], w[1], w[2]);
          }
        }
      }
      const p9 = fanCursor * 9, p6 = fanCursor * 6, p3 = fanCursor * 3, p2 = fanCursor * 2;
      globePositions[p9] = cx; globePositions[p9 + 1] = cy; globePositions[p9 + 2] = cz;
      globePositions[p9 + 3] = a[0]; globePositions[p9 + 4] = a[1]; globePositions[p9 + 5] = a[2];
      globePositions[p9 + 6] = b[0]; globePositions[p9 + 7] = b[1]; globePositions[p9 + 8] = b[2];
      globeRegions[p3] = id; globeRegions[p3 + 1] = id; globeRegions[p3 + 2] = id;
      globeCornerPairs[p6] = cA; globeCornerPairs[p6 + 1] = cB;
      globeCornerPairs[p6 + 2] = cA; globeCornerPairs[p6 + 3] = cB;
      globeCornerPairs[p6 + 4] = cA; globeCornerPairs[p6 + 5] = cB;
      globeWeights[p9] = 1; globeWeights[p9 + 4] = 1; globeWeights[p9 + 8] = 1;
      const ha = cornerElev[cA], hb = cornerElev[cB];
      radialOffsets[p3] = elevC; radialOffsets[p3 + 1] = ha; radialOffsets[p3 + 2] = hb;
      lineOffsets[p2] = ha; lineOffsets[p2 + 1] = hb;
      globeLines[p6] = a[0] * 1.0002; globeLines[p6 + 1] = a[1] * 1.0002; globeLines[p6 + 2] = a[2] * 1.0002;
      globeLines[p6 + 3] = b[0] * 1.0002; globeLines[p6 + 4] = b[1] * 1.0002; globeLines[p6 + 5] = b[2] * 1.0002;
      fanCursor++;
    }
  }

  const tec = buildTectonicLines(tectonics, elevation, (x, y, z) => {
    const cid = lookupCorner(x, y, z);
    if (cid < 0) throw new Error('Missing shared relief corner.');
    return cornerElev[cid];
  });

  const flatView: ViewGeometry = {
    positions: flatPositions.finish(), regions: flatRegions.finish(), lines: flatLines.finish(),
    cornerPairs: flatCornerPairs.finish(), weights: flatWeights.finish(),
    tectonicLines: tec.flatLines, tectonicColors: tec.flatColors,
    radialOffsets: new Float32Array(0), lineOffsets: new Float32Array(0), tectonicOffsets: new Float32Array(0),
    waterPositions: new Float32Array(0), waterRegions: new Float32Array(0), waterOffsets: new Float32Array(0),
    waterCornerPairs: new Float32Array(0), waterWeights: new Float32Array(0),
    waterLines: new Float32Array(0), waterLineOffsets: new Float32Array(0),
  };
  const globeView: ViewGeometry = {
    positions: globePositions, regions: globeRegions, lines: globeLines,
    cornerPairs: globeCornerPairs, weights: globeWeights,
    tectonicLines: tec.globeLines, tectonicColors: tec.globeColors,
    radialOffsets, lineOffsets, tectonicOffsets: tec.globeOffsets,
    waterPositions: new Float32Array(0), waterRegions: new Float32Array(0), waterOffsets: new Float32Array(0),
    waterCornerPairs: new Float32Array(0), waterWeights: new Float32Array(0),
    waterLines: new Float32Array(0), waterLineOffsets: new Float32Array(0),
  };
  if (water) Object.assign(globeView, buildWaterSurface(globeView.positions, globeView.regions, water, surface.radiusMeters,
    { cornerPairs: globeView.cornerPairs, weights: globeView.weights }));
  return { flat: flatView, globe: globeView, corners };
}

/**
 * Build an indexed water surface mesh matching the compact 6-fan per-region globe bed.
 * Used for both initial render builds and live water-frame updates in SurfaceMap.
 */
export function buildIndexedWaterSurface(
  surface: Surface,
  ring: CompactDualRing,
  water: Water | WaterFrame | RegionalWaterDisplay,
): {
  waterPositions: Float32Array;
  waterRegions: Float32Array;
  waterOffsets: Float32Array;
  waterCornerPairs: Float32Array;
  waterWeights: Float32Array;
  waterIndices: Uint32Array;
  waterLines: Float32Array;
  waterLineOffsets: Float32Array;
} {
  const mask = 'wetMask' in water ? water.wetMask : water.bodyIds;
  const levels = 'surfaceLevelsMeters' in water ? water.surfaceLevelsMeters : null;
  const flatLevel = 'levelMeters' in water ? water.levelMeters : 0;
  const radius = surface.radiusMeters;
  const n = surface.areasSquareMeters.length;
  const { ringOffsets, ringCorners, ringDirections } = ring;
  const centers = surface.centers;

  let totalVerts = 0;
  let totalFans = 0;
  for (let id = 0; id < n; id++) {
    if (!mask[id]) continue;
    const deg = ringOffsets[id + 1] - ringOffsets[id];
    totalVerts += 1 + deg;
    totalFans += deg;
  }

  const waterPositions = new Float32Array(totalVerts * 3);
  const waterRegions = new Float32Array(totalVerts);
  const waterOffsets = new Float32Array(totalVerts);
  const waterCornerPairs = new Float32Array(totalVerts * 2);
  const waterWeights = new Float32Array(totalVerts * 3);
  const waterIndices = new Uint32Array(totalFans * 3);
  const waterLines = new Float32Array(totalFans * 6);
  const waterLineOffsets = new Float32Array(totalFans * 2);

  let vCursor = 0;
  let iCursor = 0;
  let lCursor = 0;
  for (let id = 0; id < n; id++) {
    if (!mask[id]) continue;
    const start = ringOffsets[id], end = ringOffsets[id + 1];
    const deg = end - start;
    const offset = (levels ? levels[id] : flatLevel) / radius;
    const centerIdx = vCursor;
    waterPositions[vCursor * 3] = centers[id * 3];
    waterPositions[vCursor * 3 + 1] = centers[id * 3 + 1];
    waterPositions[vCursor * 3 + 2] = centers[id * 3 + 2];
    waterRegions[vCursor] = id;
    waterOffsets[vCursor] = offset;
    waterWeights[vCursor * 3] = 1;
    vCursor++;

    for (let k = 0; k < deg; k++) {
      const j = start + k;
      const cid = ringCorners[j];
      const ax = ringDirections[j * 3], ay = ringDirections[j * 3 + 1], az = ringDirections[j * 3 + 2];
      waterPositions[vCursor * 3] = ax;
      waterPositions[vCursor * 3 + 1] = ay;
      waterPositions[vCursor * 3 + 2] = az;
      waterRegions[vCursor] = id;
      waterOffsets[vCursor] = offset;
      waterCornerPairs[vCursor * 2] = cid;
      waterCornerPairs[vCursor * 2 + 1] = cid;
      waterWeights[vCursor * 3 + 1] = 1;
      vCursor++;
    }

    for (let k = 0; k < deg; k++) {
      const nextK = k + 1 === deg ? 0 : k + 1;
      waterIndices[iCursor++] = centerIdx;
      waterIndices[iCursor++] = centerIdx + 1 + k;
      waterIndices[iCursor++] = centerIdx + 1 + nextK;
      const j = start + k, jNext = start + nextK;
      const l6 = lCursor * 6, l2 = lCursor * 2;
      waterLines[l6] = ringDirections[j * 3] * 1.0002;
      waterLines[l6 + 1] = ringDirections[j * 3 + 1] * 1.0002;
      waterLines[l6 + 2] = ringDirections[j * 3 + 2] * 1.0002;
      waterLines[l6 + 3] = ringDirections[jNext * 3] * 1.0002;
      waterLines[l6 + 4] = ringDirections[jNext * 3 + 1] * 1.0002;
      waterLines[l6 + 5] = ringDirections[jNext * 3 + 2] * 1.0002;
      waterLineOffsets[l2] = offset;
      waterLineOffsets[l2 + 1] = offset;
      lCursor++;
    }
  }

  return { waterPositions, waterRegions, waterOffsets, waterCornerPairs, waterWeights, waterIndices, waterLines, waterLineOffsets };
}

/**
 * Compact indexed renderer geometry for live desktop viewing (`geometry.worker.ts`).
 * Collapses degree-2 edge midpoints so each hexagon/pentagon emits 6/5 dual-corner fans
 * sharing 7/6 indexed vertices (2x fewer triangles, 2.5x fewer corners, 5.1x fewer vertices),
 * and partitions globe bed indices into (dry + coastal) first and (interior ocean) second
 * so surface layers skip underwater ocean-bed triangles via drawRange.
 */
export function buildRenderViewGeometry(surface: Surface, tectonics?: Tectonics, elevation?: Float64Array, water?: Water): ViewPair {
  const n = surface.areasSquareMeters.length;
  if (elevation && elevation.length !== n) throw new Error('Invalid elevation field length.');
  const raw = deduplicateBoundaryCorners(surface);

  // Remap ONLY true dual corners (incidence >= 3) to compact IDs 0..trueCount-1.
  // Falls back to all corners on degenerate test inputs if no degree>=3 corners exist.
  const oldToNew = new Int32Array(raw.cornerCount).fill(-1);
  let trueCount = 0;
  for (let c = 0; c < raw.cornerCount; c++) {
    if (raw.cornerOffsets[c + 1] - raw.cornerOffsets[c] >= 3) oldToNew[c] = trueCount++;
  }
  const filterDeg3 = trueCount > 0;
  if (!filterDeg3) {
    for (let c = 0; c < raw.cornerCount; c++) oldToNew[c] = c;
    trueCount = raw.cornerCount;
  }

  const cornerOffsets = new Uint32Array(trueCount + 1);
  for (let c = 0; c < raw.cornerCount; c++) {
    const nc = oldToNew[c];
    if (nc >= 0) cornerOffsets[nc + 1] = raw.cornerOffsets[c + 1] - raw.cornerOffsets[c];
  }
  for (let c = 0; c < trueCount; c++) cornerOffsets[c + 1] += cornerOffsets[c];
  const cornerRegions = new Uint32Array(cornerOffsets[trueCount]);
  for (let c = 0; c < raw.cornerCount; c++) {
    const nc = oldToNew[c];
    if (nc < 0) continue;
    cornerRegions.set(raw.cornerRegions.subarray(raw.cornerOffsets[c], raw.cornerOffsets[c + 1]), cornerOffsets[nc]);
  }

  const cornerElev = new Float64Array(trueCount);
  const radius = surface.radiusMeters;
  if (elevation) {
    for (let k = 0; k < trueCount; k++) {
      let sum = 0;
      const start = cornerOffsets[k], end = cornerOffsets[k + 1];
      for (let j = start; j < end; j++) sum += elevation[cornerRegions[j]];
      cornerElev[k] = sum / Math.max(1, end - start) / radius;
    }
  }

  const dirs = surface.boundaryDirections;
  const ringOffsets = new Uint32Array(n + 1);
  let totalRing = 0;
  for (let id = 0; id < n; id++) {
    ringOffsets[id] = totalRing;
    for (let j = surface.boundaryOffsets[id]; j < surface.boundaryOffsets[id + 1]; j++) {
      if (!filterDeg3 || oldToNew[raw.boundaryCorners[j]] >= 0) totalRing++;
    }
  }
  ringOffsets[n] = totalRing;
  const ringCorners = new Uint32Array(totalRing);
  const ringDirections = new Float32Array(totalRing * 3);
  let rCur = 0;
  for (let id = 0; id < n; id++) {
    for (let j = surface.boundaryOffsets[id]; j < surface.boundaryOffsets[id + 1]; j++) {
      const nc = oldToNew[raw.boundaryCorners[j]];
      if (filterDeg3 && nc < 0) continue;
      ringCorners[rCur] = nc;
      ringDirections[rCur * 3] = dirs[j * 3];
      ringDirections[rCur * 3 + 1] = dirs[j * 3 + 1];
      ringDirections[rCur * 3 + 2] = dirs[j * 3 + 2];
      rCur++;
    }
  }
  const compactRing: CompactDualRing = { ringOffsets, ringCorners, ringDirections };
  const corners: ViewCorners = { offsets: cornerOffsets, regions: cornerRegions };

  // Partition regions into (dry + coastal wet) vs (interior wet) so surface-mode globe
  // rendering skips underwater interior-ocean bed triangles via setDrawRange.
  const isInteriorWet = new Uint8Array(n);
  let landCoastFans = 0;
  if (water) {
    for (let id = 0; id < n; id++) {
      let interior = water.bodyIds[id] !== 0;
      if (interior) {
        for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
          if (water.bodyIds[surface.neighbors[k]] === 0) { interior = false; break; }
        }
      }
      isInteriorWet[id] = interior ? 1 : 0;
      if (!interior) landCoastFans += ringOffsets[id + 1] - ringOffsets[id];
    }
  } else {
    landCoastFans = totalRing;
  }

  const totalGlobeVerts = n + totalRing;
  const globePositions = new Float32Array(totalGlobeVerts * 3);
  const globeRegions = new Float32Array(totalGlobeVerts);
  const globeCornerPairs = new Float32Array(totalGlobeVerts * 2);
  const globeWeights = new Float32Array(totalGlobeVerts * 3);
  const radialOffsets = new Float32Array(totalGlobeVerts);
  const globeIndices = new Uint32Array(totalRing * 3);
  const globeLines = new Float32Array(totalRing * 6);
  const lineOffsets = new Float32Array(totalRing * 2);

  const flatPositions = new Float32Builder(totalGlobeVerts * 3 + 4096);
  const flatRegions = new Float32Builder(totalGlobeVerts + 2048);
  const flatCornerPairs = new Float32Builder(totalGlobeVerts * 2 + 4096);
  const flatWeights = new Float32Builder(totalGlobeVerts * 3 + 4096);
  const flatIndices = new Uint32Builder(totalRing * 3 + 4096);
  const flatLines = new Float32Builder(totalRing * 6 + 2048);

  const centers = surface.centers;
  const cosEdgeLimit = Math.cos(0.039);
  let gVert = 0;
  let gIdxFront = 0;
  let gIdxBack = landCoastFans * 3;
  let gLine = 0;
  let fVert = 0;

  for (let id = 0; id < n; id++) {
    const cx = centers[id * 3], cy = centers[id * 3 + 1], cz = centers[id * 3 + 2];
    const start = ringOffsets[id], end = ringOffsets[id + 1];
    const deg = end - start;
    const elevC = elevation ? elevation[id] / radius : 0;

    // Emit indexed globe region vertices: center + deg dual corners.
    const gCenter = gVert;
    globePositions[gVert * 3] = cx; globePositions[gVert * 3 + 1] = cy; globePositions[gVert * 3 + 2] = cz;
    globeRegions[gVert] = id;
    globeWeights[gVert * 3] = 1;
    radialOffsets[gVert] = elevC;
    gVert++;

    for (let k = 0; k < deg; k++) {
      const j = start + k;
      const cid = ringCorners[j];
      globePositions[gVert * 3] = ringDirections[j * 3];
      globePositions[gVert * 3 + 1] = ringDirections[j * 3 + 1];
      globePositions[gVert * 3 + 2] = ringDirections[j * 3 + 2];
      globeRegions[gVert] = id;
      globeCornerPairs[gVert * 2] = cid;
      globeCornerPairs[gVert * 2 + 1] = cid;
      globeWeights[gVert * 3 + 1] = 1;
      radialOffsets[gVert] = cornerElev[cid];
      gVert++;
    }

    const interior = isInteriorWet[id] === 1;
    for (let k = 0; k < deg; k++) {
      const nextK = k + 1 === deg ? 0 : k + 1;
      if (interior) {
        globeIndices[gIdxBack++] = gCenter;
        globeIndices[gIdxBack++] = gCenter + 1 + k;
        globeIndices[gIdxBack++] = gCenter + 1 + nextK;
      } else {
        globeIndices[gIdxFront++] = gCenter;
        globeIndices[gIdxFront++] = gCenter + 1 + k;
        globeIndices[gIdxFront++] = gCenter + 1 + nextK;
      }
      const j = start + k, jNext = start + nextK;
      const l6 = gLine * 6, l2 = gLine * 2;
      globeLines[l6] = ringDirections[j * 3] * 1.0002;
      globeLines[l6 + 1] = ringDirections[j * 3 + 1] * 1.0002;
      globeLines[l6 + 2] = ringDirections[j * 3 + 2] * 1.0002;
      globeLines[l6 + 3] = ringDirections[jNext * 3] * 1.0002;
      globeLines[l6 + 4] = ringDirections[jNext * 3 + 1] * 1.0002;
      globeLines[l6 + 5] = ringDirections[jNext * 3 + 2] * 1.0002;
      lineOffsets[l2] = cornerElev[ringCorners[j]];
      lineOffsets[l2 + 1] = cornerElev[ringCorners[jNext]];
      gLine++;
    }

    // Flat projection: fast indexed path for the 99.6% non-seam, non-polar regions.
    let fastRegion = Math.abs(cy) < 0.985;
    let lonMin = Infinity, lonMax = -Infinity;
    if (fastRegion) {
      const cLon = Math.atan2(cz, cx);
      lonMin = lonMax = cLon;
      for (let j = start; j < end; j++) {
        const ax = ringDirections[j * 3], ay = ringDirections[j * 3 + 1], az = ringDirections[j * 3 + 2];
        if (Math.abs(ay) >= 0.985 || (cx * ax + cy * ay + cz * az) < cosEdgeLimit) { fastRegion = false; break; }
        const lon = Math.atan2(az, ax);
        if (lon < lonMin) lonMin = lon;
        if (lon > lonMax) lonMax = lon;
      }
      if (fastRegion && (lonMax - lonMin > 1.5 || lonMin < -Math.PI + 0.05 || lonMax > Math.PI - 0.05)) fastRegion = false;
    }

    if (fastRegion) {
      const fCenter = fVert;
      const cFx = (Math.atan2(cz, cx) / TAU + 0.5) * 2 - 1;
      const cFy = Math.asin(Math.max(-1, Math.min(1, cy))) / Math.PI;
      flatPositions.push3(cFx, cFy, 0);
      flatRegions.push1(id);
      flatCornerPairs.push2(0, 0);
      flatWeights.push3(1, 0, 0);
      fVert++;

      for (let k = 0; k < deg; k++) {
        const j = start + k;
        const ax = ringDirections[j * 3], ay = ringDirections[j * 3 + 1], az = ringDirections[j * 3 + 2];
        const aFx = (Math.atan2(az, ax) / TAU + 0.5) * 2 - 1;
        const aFy = Math.asin(Math.max(-1, Math.min(1, ay))) / Math.PI;
        const cid = ringCorners[j];
        flatPositions.push3(aFx, aFy, 0);
        flatRegions.push1(id);
        flatCornerPairs.push2(cid, cid);
        flatWeights.push3(0, 1, 0);
        fVert++;
      }

      for (let k = 0; k < deg; k++) {
        const nextK = k + 1 === deg ? 0 : k + 1;
        flatIndices.push3(fCenter, fCenter + 1 + k, fCenter + 1 + nextK);
        const pA = (fCenter + 1 + k) * 3, pB = (fCenter + 1 + nextK) * 3;
        flatLines.push6(
          flatPositions.data[pA], flatPositions.data[pA + 1], .0001,
          flatPositions.data[pB], flatPositions.data[pB + 1], .0001,
        );
      }
      continue;
    }

    // Seam/polar/coarse fallback: exact geodesic fan projection and clipping.
    const polygon = projectRegion(surface, id);
    for (const shift of [-1, 0, 1]) {
      const clipped = clip(clip(polygon.map(([x, y]) => ({ x: x + shift, y, w: [0, 0, 0] as Weights })), 0, true), 1, false);
      for (let j = 0; j < clipped.length; j++) {
        const a = clipped[j], b = clipped[(j + 1) % clipped.length];
        flatLines.push6(a.x * 2 - 1, .5 - a.y, .0001, b.x * 2 - 1, .5 - b.y, .0001);
      }
    }
    const center: Vec3 = [cx, cy, cz];
    for (let k = 0; k < deg; k++) {
      const j = start + k, jNext = start + (k + 1 === deg ? 0 : k + 1);
      const a: Vec3 = [ringDirections[j * 3], ringDirections[j * 3 + 1], ringDirections[j * 3 + 2]];
      const b: Vec3 = [ringDirections[jNext * 3], ringDirections[jNext * 3 + 1], ringDirections[jNext * 3 + 2]];
      const cA = ringCorners[j], cB = ringCorners[jNext];
      const fan = projectFan(center, a, b);
      for (const shift of [-1, 0, 1]) {
        const clipped = clip(clip(fan.map(p => ({ ...p, x: p.x + shift })), 0, true), 1, false);
        if (clipped.length < 3) continue;
        const contour = clipped.map(p => new Vector2(p.x * 2 - 1, .5 - p.y));
        const baseV = fVert;
        for (let v = 0; v < contour.length; v++) {
          flatPositions.push3(contour[v].x, contour[v].y, 0);
          flatRegions.push1(id);
          flatCornerPairs.push2(cA, cB);
          const w = clipped[v].w;
          flatWeights.push3(w[0], w[1], w[2]);
          fVert++;
        }
        for (const triangle of ShapeUtils.triangulateShape(contour, [])) {
          flatIndices.push3(baseV + triangle[0], baseV + triangle[1], baseV + triangle[2]);
        }
      }
    }
  }

  const tec = buildTectonicLines(tectonics, elevation, (x, y, z) => {
    const rawCid = raw.lookupCorner(x, y, z);
    if (rawCid < 0) return 0;
    const nc = oldToNew[rawCid];
    if (nc >= 0) return cornerElev[nc];
    // Degree-2 tectonic vertex (edge midpoint): average its 2 incident regions.
    let sum = 0;
    const s = raw.cornerOffsets[rawCid], e = raw.cornerOffsets[rawCid + 1];
    for (let j = s; j < e; j++) sum += elevation![raw.cornerRegions[j]];
    return sum / Math.max(1, e - s) / radius;
  });

  const flatView: ViewGeometry = {
    positions: flatPositions.finish(), regions: flatRegions.finish(), lines: flatLines.finish(),
    cornerPairs: flatCornerPairs.finish(), weights: flatWeights.finish(),
    indices: flatIndices.finish(),
    tectonicLines: tec.flatLines, tectonicColors: tec.flatColors,
    radialOffsets: new Float32Array(0), lineOffsets: new Float32Array(0), tectonicOffsets: new Float32Array(0),
    waterPositions: new Float32Array(0), waterRegions: new Float32Array(0), waterOffsets: new Float32Array(0),
    waterCornerPairs: new Float32Array(0), waterWeights: new Float32Array(0),
    waterLines: new Float32Array(0), waterLineOffsets: new Float32Array(0),
  };
  const globeView: ViewGeometry = {
    positions: globePositions, regions: globeRegions, lines: globeLines,
    cornerPairs: globeCornerPairs, weights: globeWeights,
    indices: globeIndices,
    landCoastIndexCount: landCoastFans * 3,
    tectonicLines: tec.globeLines, tectonicColors: tec.globeColors,
    radialOffsets, lineOffsets, tectonicOffsets: tec.globeOffsets,
    waterPositions: new Float32Array(0), waterRegions: new Float32Array(0), waterOffsets: new Float32Array(0),
    waterCornerPairs: new Float32Array(0), waterWeights: new Float32Array(0),
    waterLines: new Float32Array(0), waterLineOffsets: new Float32Array(0),
  };
  if (water) Object.assign(globeView, buildIndexedWaterSurface(surface, compactRing, water));
  return { flat: flatView, globe: globeView, corners, compactRing };
}
