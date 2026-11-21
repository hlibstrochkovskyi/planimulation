import type { Surface } from '../core/surface';
import type { Drainage } from '../core/drainage';
import type { Rivers } from '../core/rivers';
import { angularDistance, cross, midpoint, normalize, readVector } from '../core/vector';
import type { Vec3 } from '../core/vector';
import { projectArc } from './projection';
import { meshFrequencyForRegions, sampleCoastPhi, sampleDetailHeightMeters, seedOffsetFromString } from './refinement';
import type { CoastField } from './refinement';
import type { ReliefMetrics } from './natural-color';

const CURVE_STEPS = 7;

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

/** Deterministic integer hash in [0, 1) for meander phase & jitter (never Math.random). */
function edgeHash(seed: number, a: number, b: number, salt: number): number {
  let h = (Math.imul(a + 1, 0x9e3779b1) ^ Math.imul(b + 1, 0x85ebca6b) ^ Math.imul(seed ^ salt, 0xc2b2ae35)) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d) >>> 0;
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39) >>> 0;
  return ((h ^ (h >>> 15)) >>> 0) / 0x100000000;
}

function seedHashInt(seed: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 0x01000193) >>> 0;
  return h >>> 0;
}

/** Line widths in CSS pixels for increasing contributing-area classes. */
export const DRAINAGE_LINE_WIDTHS = [1, 1.7, 2.6] as const;
/** Nearly even intervals on a log discharge scale (m³/s). */
export const RIVER_LINE_THRESHOLDS = [300, 1_000, 3_000, 10_000, 30_000] as const;
export const RIVER_LINE_WIDTHS = [0.85, 1.3, 2, 3, 3.9] as const;
/** Sleeker natural-surface widths that taper smoothly from headwaters to major trunks. */
export const NATURAL_RIVER_WIDTHS = [0.72, 1.02, 1.48, 2.15, 2.9] as const;
export const PERENNIAL_LINE_COLORS = [0x30486b, 0x124fa7, 0x0767d4, 0x1589f5, 0x40b9ff] as const;
export const SEASONAL_LINE_COLORS = [0x425e60, 0x2c6669, 0x25848b, 0x17a5a8, 0x37c7c3] as const;
/** Calm satellite-style river blues for Natural color surface (readable over dark forests, no neon cyan). */
export const NATURAL_PERENNIAL_COLORS = [0x38618a, 0x2f6294, 0x28659e, 0x2368a8, 0x206db2] as const;
export const NATURAL_SEASONAL_COLORS = [0x54737c, 0x486e79, 0x3e6a77, 0x346676, 0x2d6375] as const;

export interface RiverDisplayOptions {
  seed?: string;
  waterLevelMeters?: number;
  coastField?: CoastField;
  reliefMetrics?: ReliefMetrics;
  valleyFactor?: Float32Array;
  detailEnabled?: boolean;
}

export interface DrainageLines {
  /** Primary river segments visible at planet scale (per width/season class). */
  flat: Float32Array[];
  /** Globe unit-direction segment endpoints and refined radial height offsets per class. */
  globe: { positions: Float32Array; offsets: Float32Array }[];
  /** Minor coastal stubs & smallest headwaters shown when zoomed in (reduces planet-scale comb). */
  flatDetail: Float32Array[];
  globeDetail: { positions: Float32Array; offsets: Float32Array }[];
}

/**
 * Display-only curved river lines for static receivers and analytical spill paths.
 * Adds deterministic, cell-bounded meanders, smooth confluences, smoothed coastline
 * mouths, zoom-tiered coastal stub filtering, and refined globe terrain draping.
 */
export function buildDrainageLines(
  surface: Surface,
  drainage: Drainage,
  elevation: Float64Array,
  wet: ArrayLike<number>,
  rivers?: Rivers,
  options: RiverDisplayOptions = {},
): DrainageLines {
  const n = elevation.length;
  if (surface.areasSquareMeters.length !== n || drainage.receivers.length !== n || wet.length !== n) {
    throw new Error('Invalid drainage display input.');
  }
  const meanArea = surface.areasSquareMeters.reduce((sum, area) => sum + area, 0) / n;
  const minimum = Math.max(10 * meanArea, 1e11);
  const thresholds = rivers ? RIVER_LINE_THRESHOLDS : [minimum, minimum * 5, minimum * 25];
  const sizes = rivers ? 2 * thresholds.length : thresholds.length;

  const flat: number[][] = Array.from({ length: sizes }, () => []);
  const globe = flat.map(() => ({ positions: [] as number[], offsets: [] as number[] }));
  const flatDetail: number[][] = Array.from({ length: sizes }, () => []);
  const globeDetail = flatDetail.map(() => ({ positions: [] as number[], offsets: [] as number[] }));

  const weight = rivers?.meanDischarge ?? drainage.contributingArea;
  const widthFor = (flow: number, riverClass: number): number => {
    const width = thresholds.findLastIndex(threshold => flow >= threshold);
    if (width < 0 || riverClass === 0) return -1;
    return rivers && riverClass === 1 ? width + thresholds.length : width;
  };

  const radius = surface.radiusMeters;
  const detailEnabled = options.detailEnabled ?? true;
  const seedStr = options.seed ?? 'first-light';
  const seedInt = seedHashInt(seedStr);
  const seedOffset = seedOffsetFromString(seedStr);
  const coastField = options.coastField;
  const reliefMetrics = options.reliefMetrics;
  const valleyFactor = options.valleyFactor;
  const waterLevel = options.waterLevelMeters ?? 0;
  const meshFreq = meshFrequencyForRegions(n);

  // Identify main stem donor and donor counts for smooth confluences and node jitter bounds.
  const mainDonor = new Int32Array(n).fill(-1);
  const donorCount = new Uint16Array(n);
  const activeWidth = new Int8Array(n).fill(-1);
  for (let i = 0; i < n; i++) {
    const r = drainage.receivers[i];
    const w = widthFor(weight[i], rivers?.class[i] ?? 2);
    if (r === i || wet[i] || w < 0) continue;
    activeWidth[i] = w;
    donorCount[r]++;
    const current = mainDonor[r];
    if (current < 0 || weight[i] > weight[current]) mainDonor[r] = i;
  }

  // Compute upstream hop length and downstream distance-to-mouth to detect short coastal stubs.
  const order: number[] = [];
  for (let i = 0; i < n; i++) if (activeWidth[i] >= 0) order.push(i);
  order.sort((a, b) => weight[a] - weight[b]);
  const upstreamHops = new Uint16Array(n);
  for (const i of order) {
    const r = drainage.receivers[i];
    const nextHops = upstreamHops[i] + 1;
    if (nextHops > upstreamHops[r]) upstreamHops[r] = nextHops;
  }
  // Propagate total chain length back upstream so every segment of a short coastal stub knows its reach length.
  const chainLength = new Uint16Array(n);
  for (let k = order.length - 1; k >= 0; k--) {
    const i = order[k];
    const r = drainage.receivers[i];
    if (wet[r] || r === i || activeWidth[r] < 0) {
      chainLength[i] = Math.max(1, upstreamHops[i] + 1);
    } else {
      chainLength[i] = Math.max(upstreamHops[i] + 1, chainLength[r]);
    }
  }

  const rawCenter = (i: number): Vec3 => readVector(surface.centers, i);

  // Subtle deterministic node offset on non-confluence interior nodes breaks straight grid alignment
  // while staying strictly within 0.13 * edge spacing (< 1/3 of the cell's inscribed radius).
  const nodePoint = new Float64Array(n * 3);
  for (let i = 0; i < n; i++) {
    const c = rawCenter(i);
    let px = c[0], py = c[1], pz = c[2];
    const r = drainage.receivers[i];
    const d = mainDonor[i];
    if (detailEnabled && activeWidth[i] >= 0 && d >= 0 && donorCount[i] <= 2 && !wet[r] && r !== i) {
      const up = rawCenter(d), dn = rawCenter(r);
      const tx = dn[0] - up[0], ty = dn[1] - up[1], tz = dn[2] - up[2];
      const tLen = Math.hypot(tx, ty, tz);
      if (tLen > 1e-8) {
        const perp = cross(c, [tx / tLen, ty / tLen, tz / tLen]);
        const pLen = Math.hypot(perp[0], perp[1], perp[2]);
        if (pLen > 1e-8) {
          const span = angularDistance(c, dn);
          const slope = Math.abs(elevation[i] - elevation[r]) / Math.max(1, span * radius);
          const flatness = 1 / (1 + slope / 0.008);
          const jitterScale = donorCount[i] === 1 ? 0.135 : 0.075;
          const jitter = (edgeHash(seedInt, i, r, 11) * 2 - 1) * jitterScale * span * flatness;
          const shifted = normalize([px + (perp[0] / pLen) * jitter, py + (perp[1] / pLen) * jitter, pz + (perp[2] / pLen) * jitter]);
          px = shifted[0]; py = shifted[1]; pz = shifted[2];
        }
      }
    }
    nodePoint[i * 3] = px; nodePoint[i * 3 + 1] = py; nodePoint[i * 3 + 2] = pz;
  }

  const pointOf = (i: number): Vec3 => [nodePoint[i * 3], nodePoint[i * 3 + 1], nodePoint[i * 3 + 2]];

  /** Bisect along great circle between dry `origin` and wet `receiver` to find smoothed coastline zero-contour. */
  const findCoastMouth = (i: number, r: number, origin: Vec3, receiver: Vec3): Vec3 => {
    if (!coastField) return midpoint(origin, receiver);
    const phiI = coastField.phi[i], phiR = coastField.phi[r];
    const mI = coastField.strictMask[i], mR = coastField.strictMask[r];
    const rugI = reliefMetrics?.ruggedness[i] ?? 0;
    const rugR = reliefMetrics?.ruggedness[r] ?? 0;
    let lo = 0.12, hi = 0.88;
    for (let iter = 0; iter < 8; iter++) {
      const t = 0.5 * (lo + hi);
      const p = normalize([
        origin[0] * (1 - t) + receiver[0] * t,
        origin[1] * (1 - t) + receiver[1] * t,
        origin[2] * (1 - t) + receiver[2] * t,
      ]);
      const wCenter = Math.abs(1 - 2 * t);
      const phiVal = sampleCoastPhi(
        p[0], p[1], p[2],
        phiI * (1 - t) + phiR * t,
        mI * (1 - t) + mR * t,
        wCenter,
        rugI * (1 - t) + rugR * t,
        seedOffset,
        detailEnabled,
        meshFreq,
      );
      if (phiVal > 0) lo = t; else hi = t;
    }
    const tMouth = 0.5 * (lo + hi);
    return normalize([
      origin[0] * (1 - tMouth) + receiver[0] * tMouth,
      origin[1] * (1 - tMouth) + receiver[1] * tMouth,
      origin[2] * (1 - tMouth) + receiver[2] * tMouth,
    ]);
  };

  const addPath = (i: number, r: number, width: number, smooth: boolean, isDetailTier: boolean): void => {
    const origin = pointOf(i), receiver = pointOf(r);
    const entersWater = Boolean(wet[r]);
    const leavesWater = Boolean(wet[i]);
    const mouth = entersWater ? findCoastMouth(i, r, origin, receiver)
      : leavesWater ? findCoastMouth(r, i, receiver, origin)
        : origin;

    const a = leavesWater ? mouth : origin;
    const b = entersWater ? mouth : receiver;
    const localWlI = coastField ? coastField.localWaterLevel[i] : waterLevel;
    const localWlR = coastField ? coastField.localWaterLevel[r] : waterLevel;
    const ha = leavesWater ? localWlI : Math.max(localWlI, elevation[i]);
    const hb = entersWater ? localWlR : Math.max(localWlR, elevation[r]);

    // Smooth confluences: tributaries entering a main-stem node curve slightly toward
    // the downstream direction so junctions form natural acute Y-confluences.
    const before = smooth && mainDonor[i] >= 0 ? pointOf(mainDonor[i]) : extrapolate(a, b);
    const rr = drainage.receivers[r];
    let after: Vec3;
    if (!smooth || entersWater || rr === r) {
      after = extrapolate(b, a);
    } else if (mainDonor[r] >= 0 && mainDonor[r] !== i) {
      // Tributary entering confluence at r: blend straight continuation with downstream stem direction
      const down = pointOf(rr);
      const straight = extrapolate(b, a);
      after = normalize([
        down[0] * 0.65 + straight[0] * 0.35,
        down[1] * 0.65 + straight[1] * 0.35,
        down[2] * 0.65 + straight[2] * 0.35,
      ]);
    } else {
      after = pointOf(rr);
    }

    const chordAngle = angularDistance(a, b);
    const segmentMeters = Math.max(1, chordAngle * radius);
    const segSlope = Math.abs(ha - hb) / segmentMeters;
    const flatness = 1 / (1 + segSlope / 0.009);
    const classBin = width % thresholds.length;
    const classFactor = 0.65 + 0.14 * classBin;
    const spillScale = smooth ? 1.0 : 0.72;
    // Strictly bounded within 0.185 * chordAngle so meanders never exit the cells or cross neighbor rivers.
    const meanderAmp = detailEnabled && chordAngle > 1e-5
      ? Math.min(0.185 * chordAngle, (0.048 + 0.132 * flatness) * classFactor * spillScale * chordAngle)
      : 0;

    const bendSign = edgeHash(seedInt, i, r, 31) < 0.5 ? -1 : 1;
    const phase2 = edgeHash(seedInt, i, r, 73) * Math.PI * 2;
    const freq1 = 1.9 + 0.35 * edgeHash(seedInt, i, r, 97);
    const freq2 = 3.2 + 0.75 * edgeHash(seedInt, i, r, 131);

    const relI = reliefMetrics?.effectiveReliefMeters[i] ?? reliefMetrics?.localReliefMeters[i] ?? 0;
    const relR = entersWater ? 0 : (reliefMetrics?.effectiveReliefMeters[r] ?? reliefMetrics?.localReliefMeters[r] ?? 0);
    const valI = valleyFactor ? valleyFactor[i] : 0;
    const valR = entersWater ? 0 : (valleyFactor ? valleyFactor[r] : 0);
    const phiI = coastField ? coastField.phi[i] : 1;
    const phiR = coastField ? (entersWater ? 0 : coastField.phi[r]) : 1;

    const basePoints: Vec3[] = [];
    for (let step = 0; step <= CURVE_STEPS; step++) {
      basePoints.push(catmullRom(before, a, b, after, step / CURVE_STEPS));
    }

    const points: Vec3[] = [];
    const heights: number[] = [];
    for (let step = 0; step <= CURVE_STEPS; step++) {
      const t = step / CURVE_STEPS;
      let p = basePoints[step];
      if (meanderAmp > 0 && step > 0 && step < CURVE_STEPS) {
        const prev = basePoints[step - 1], next = basePoints[step + 1];
        const tan: Vec3 = [next[0] - prev[0], next[1] - prev[1], next[2] - prev[2]];
        const tLen = Math.hypot(tan[0], tan[1], tan[2]);
        if (tLen > 1e-9) {
          const perp = cross(p, [tan[0] / tLen, tan[1] / tLen, tan[2] / tLen]);
          const pLen = Math.hypot(perp[0], perp[1], perp[2]);
          if (pLen > 1e-9) {
            const mouthTaper = entersWater ? Math.pow(1 - t, 0.65) : leavesWater ? Math.pow(t, 0.65) : 1;
            const env = Math.sin(Math.PI * t) * mouthTaper;
            const wave = bendSign * (0.76 * Math.sin(Math.PI * freq1 * t) + 0.24 * Math.sin(Math.PI * freq2 * t + phase2));
            const disp = meanderAmp * env * wave;
            p = normalize([
              p[0] + (perp[0] / pLen) * disp,
              p[1] + (perp[1] / pLen) * disp,
              p[2] + (perp[2] / pLen) * disp,
            ]);
          }
        }
      }
      points.push(p);

      // Follow the exact refined terrain elevation on the globe (smooth base + valley-carved fractal detail).
      const hBase = ha + (hb - ha) * t;
      const wl = localWlI + (localWlR - localWlI) * t;
      const relInterp = relI + (relR - relI) * t;
      const valInterp = valI + (valR - valI) * t;
      const phiInterp = Math.max(0, phiI + (phiR - phiI) * t);
      const detailM = sampleDetailHeightMeters(p[0], p[1], p[2], relInterp, phiInterp, seedOffset, detailEnabled, valInterp, meshFreq);
      heights.push(Math.max(wl, hBase + detailM));
    }

    const flatBucket = isDetailTier ? flatDetail[width] : flat[width];
    const globeBucket = isDetailTier ? globeDetail[width] : globe[width];
    const lift = 1.00032;
    for (let k = 0; k < CURVE_STEPS; k++) {
      const p = points[k], q = points[k + 1];
      for (const [x, y] of projectArc(p, q)) {
        flatBucket.push(x[0] * 2 - 1, .5 - x[1], .0004, y[0] * 2 - 1, .5 - y[1], .0004);
      }
      globeBucket.positions.push(p[0] * lift, p[1] * lift, p[2] * lift, q[0] * lift, q[1] * lift, q[2] * lift);
      globeBucket.offsets.push(heights[k] / radius, heights[k + 1] / radius);
    }
  };

  for (let i = 0; i < n; i++) {
    const r = drainage.receivers[i], width = activeWidth[i];
    if (width < 0) continue;
    const classBin = width % thresholds.length;
    // Short 1–2 hop coastal stubs in the smallest discharge classes move to the zoomed-in detail tier
    // so planet-scale views read cleanly without a coastal "comb" of parallel whiskers.
    const isShortCoastalStub = (wet[r] || wet[drainage.receivers[r]]) && chainLength[i] <= 2 && classBin <= 1;
    const isTinyHeadwater = rivers ? (classBin === 0 && chainLength[i] <= 3) : (classBin === 0 && chainLength[i] <= 2);
    addPath(i, r, width, true, isShortCoastalStub || isTinyHeadwater);
  }

  for (const { from, to, class: riverClass, meanDischarge } of rivers?.spillSegments ?? []) {
    const width = widthFor(meanDischarge, riverClass);
    if (width >= 0) addPath(from, to, width, false, false);
  }

  return {
    flat: flat.map(v => Float32Array.from(v)),
    globe: globe.map(g => ({ positions: Float32Array.from(g.positions), offsets: Float32Array.from(g.offsets) })),
    flatDetail: flatDetail.map(v => Float32Array.from(v)),
    globeDetail: globeDetail.map(g => ({ positions: Float32Array.from(g.positions), offsets: Float32Array.from(g.offsets) })),
  };
}
