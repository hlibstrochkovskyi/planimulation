import type { Surface } from '../core/surface';
import type { Vec3 } from '../core/vector';

export const MAX_LOD_TRIANGLES = 110_000;

export interface CoastField {
  /** Per-region compressed signed height relative to local water surface (<0 wet, >0 dry). */
  phi: Float32Array;
  /** Per-region strict wet/dry sign (-1 wet, +1 dry). */
  strictMask: Float32Array;
  /** Per-region local water surface elevation in meters (sea level or adjacent lake level). */
  localWaterLevel: Float32Array;
  /** Per-region lake marker: 0 = land, 0 = ocean (waterKind 1), 1 = inland lake (waterKind 2). */
  lakeMask: Float32Array;
}

export interface LodPatchData {
  positions: Float32Array;
  regions: Float32Array;
  cornerPairs: Float32Array;
  weights: Float32Array;
  radialOffsets: Float32Array;
  replacedRegions: Uint32Array;
  subdivision: number;
  triangleCount: number;
}

const clamp = (v: number, lo: number, hi: number): number => (v < lo ? lo : v > hi ? hi : v);
const smoothstep = (e0: number, e1: number, x: number): number => {
  const t = clamp((x - e0) / (e1 - e0), 0, 1);
  return t * t * (3 - 2 * t);
};
const fract = (x: number): number => x - Math.floor(x);

/**
 * Deterministic 3D domain offset derived from the world seed string.
 * Consumes no model random stream and never uses Math.random().
 */
export function seedOffsetFromString(seed: string): Vec3 {
  let h1 = 0x811c9dc5;
  let h2 = 0x9e3779b1;
  let h3 = 0x85ebca6b;
  for (let i = 0; i < seed.length; i++) {
    const ch = seed.charCodeAt(i);
    h1 = Math.imul(h1 ^ ch, 0x01000193) >>> 0;
    h2 = Math.imul(h2 ^ (ch + i * 31), 0x2c1b3c6d) >>> 0;
    h3 = Math.imul(h3 ^ (ch * 17 + i), 0x297a2d39) >>> 0;
  }
  const map = (h: number): number => ((h >>> 0) / 0xffffffff) * 80 - 40;
  return [map(h1), map(h2), map(h3)];
}

/** Sine-free 3D lattice hash matching the GLSL shader implementation. */
function hash3(ix: number, iy: number, iz: number): number {
  let px = fract(ix * 0.1031);
  let py = fract(iy * 0.1030);
  let pz = fract(iz * 0.0973);
  const d = px * (py + 33.33) + py * (px + 33.33) + pz * (pz + 33.33);
  px += d; py += d; pz += d;
  return fract((px + py) * pz);
}

/** C2-continuous 3D value noise in [0, 1], matching GLSL `vnoise3`. */
export function vnoise3(x: number, y: number, z: number): number {
  const ix = Math.floor(x), iy = Math.floor(y), iz = Math.floor(z);
  const fx = x - ix, fy = y - iy, fz = z - iz;
  const ux = fx * fx * fx * (fx * (fx * 6 - 15) + 10);
  const uy = fy * fy * fy * (fy * (fy * 6 - 15) + 10);
  const uz = fz * fz * fz * (fz * (fz * 6 - 15) + 10);

  const n000 = hash3(ix, iy, iz);
  const n100 = hash3(ix + 1, iy, iz);
  const n010 = hash3(ix, iy + 1, iz);
  const n110 = hash3(ix + 1, iy + 1, iz);
  const n001 = hash3(ix, iy, iz + 1);
  const n101 = hash3(ix + 1, iy, iz + 1);
  const n011 = hash3(ix, iy + 1, iz + 1);
  const n111 = hash3(ix + 1, iy + 1, iz + 1);

  const nx00 = n000 + (n100 - n000) * ux;
  const nx10 = n010 + (n110 - n010) * ux;
  const nx01 = n001 + (n101 - n001) * ux;
  const nx11 = n011 + (n111 - n011) * ux;
  const nxy0 = nx00 + (nx10 - nx00) * uy;
  const nxy1 = nx01 + (nx11 - nx01) * uy;
  return nxy0 + (nxy1 - nxy0) * uz;
}

/** Four-octave domain-warped coastline perturbation in [-1, 1] evaluated on the 3D unit sphere. */
export function sampleCoastNoise(x: number, y: number, z: number, seedOffset: Vec3): number {
  const ox = seedOffset[0], oy = seedOffset[1], oz = seedOffset[2];
  // Gentle 3D domain warp breaks regular blob symmetry into organic bays and headlands.
  const wx = (vnoise3(x * 34.0 + ox + 3.1, y * 34.0 + oy - 7.4, z * 34.0 + oz + 11.2) * 2 - 1) * 0.011;
  const wy = (vnoise3(x * 34.0 + ox - 9.7, y * 34.0 + oy + 5.3, z * 34.0 + oz - 4.8) * 2 - 1) * 0.011;
  const wz = (vnoise3(x * 34.0 + ox + 6.5, y * 34.0 + oy + 12.1, z * 34.0 + oz + 2.9) * 2 - 1) * 0.011;
  const px = x + wx, py = y + wy, pz = z + wz;
  let sum = 0;
  let amp = 0.54;
  let freq = 48.0;
  let norm = 0;
  for (let o = 0; o < 4; o++) {
    const n = vnoise3(px * freq + ox + o * 13.7, py * freq + oy + o * 7.3, pz * freq + oz + o * 19.1) * 2 - 1;
    sum += n * amp;
    norm += amp;
    freq *= 2.12;
    amp *= 0.52;
  }
  return sum / norm;
}

/**
 * Evaluate the smooth, center-preserving coastline signed field at unit-sphere point `p`.
 * Strictly preserves wet/dry sign at every model region center (`wCenter == 1`) and
 * stays zero-perturbation in all interior land/ocean regions (`|mStrict| == 1`).
 */
export function sampleCoastPhi(
  x: number,
  y: number,
  z: number,
  phiBase: number,
  mStrict: number,
  wCenter: number,
  ruggedness: number,
  seedOffset: Vec3,
  detailEnabled: boolean,
): number {
  if (!detailEnabled) return phiBase;
  const coastBand = smoothstep(0.02, 0.32, 1.0 - Math.abs(mStrict));
  const centerGuard = 1.0 - smoothstep(0.72, 0.94, wCenter);
  const coastMask = coastBand * centerGuard;
  if (coastMask <= 1e-5) return phiBase;
  const coastAmp = (0.28 + 0.18 * clamp(ruggedness, 0, 1)) * coastMask;
  return phiBase + coastAmp * sampleCoastNoise(x, y, z, seedOffset);
}

/**
 * Bounded deterministic fractal terrain detail (in meters) for globe LOD vertices
 * and river height draping. Scaled by local relief: crisp in mountains, almost zero
 * on plains, strictly zero on water and smoothly tapered to zero at coastlines.
 */
export function sampleDetailHeightMeters(
  x: number,
  y: number,
  z: number,
  localReliefMeters: number,
  phiDetail: number,
  seedOffset: Vec3,
  detailEnabled: boolean,
): number {
  if (!detailEnabled || phiDetail <= 0 || localReliefMeters <= 10) return 0;
  const coastTaper = smoothstep(0.0, 0.26, phiDetail);
  if (coastTaper <= 1e-5) return 0;
  const reliefAmp = Math.min(540, localReliefMeters * 0.32) * coastTaper;
  const ox = seedOffset[0] + 17.3, oy = seedOffset[1] - 23.1, oz = seedOffset[2] + 9.7;
  let fbm = 0;
  let ridge = 0;
  let amp = 0.54;
  let freq = 44.0;
  let norm = 0;
  let ridgeWeight = 1.0;
  for (let o = 0; o < 4; o++) {
    const raw = vnoise3(x * freq + ox + o * 11.3, y * freq + oy + o * 17.9, z * freq + oz + o * 5.7);
    const signed = raw * 2 - 1;
    let r = 1.0 - Math.sqrt(signed * signed + 0.008);
    r = r * r * ridgeWeight;
    ridgeWeight = clamp(r * 1.45, 0.25, 1.0);
    fbm += signed * amp;
    ridge += (r * 2 - 0.85) * amp;
    norm += amp;
    freq *= 2.08;
    amp *= 0.50;
  }
  const rugged = smoothstep(120, 1050, localReliefMeters);
  const combined = ((1 - rugged * 0.62) * fbm + rugged * 0.62 * ridge) / norm;
  return combined * reliefAmp;
}

/**
 * Compute per-region signed elevation relative to local water surface (`phi`)
 * and strict wet/dry mask (`-1` wet, `+1` dry) for smooth zero-contour coastlines.
 */
export function computeCoastField(
  surface: Surface,
  elevation: Float64Array,
  wetMask: ArrayLike<number>,
  depthMeters: ArrayLike<number>,
  waterLevelMeters: number,
  bodyIds: ArrayLike<number>,
  mainOceanId: number,
  surfaceLevelsMeters?: ArrayLike<number>,
): CoastField {
  const n = elevation.length;
  const phi = new Float32Array(n);
  const strictMask = new Float32Array(n);
  const localWaterLevel = new Float32Array(n);
  const lakeMask = new Float32Array(n);

  for (let id = 0; id < n; id++) {
    if (wetMask[id]) {
      const level = surfaceLevelsMeters ? surfaceLevelsMeters[id] : elevation[id] + depthMeters[id];
      localWaterLevel[id] = Number.isFinite(level) ? level : waterLevelMeters;
      const d = Math.max(depthMeters[id] ?? 0, 12);
      phi[id] = -Math.asinh(d / 160);
      strictMask[id] = -1;
      lakeMask[id] = mainOceanId > 0 && bodyIds[id] === mainOceanId ? 0 : 1;
    } else {
      strictMask[id] = 1;
      lakeMask[id] = 0;
    }
  }

  for (let id = 0; id < n; id++) {
    if (wetMask[id]) continue;
    let levelSum = 0;
    let wetNeighbors = 0;
    for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
      const nb = surface.neighbors[k];
      if (wetMask[nb]) {
        levelSum += localWaterLevel[nb];
        wetNeighbors++;
      }
    }
    const refLevel = wetNeighbors > 0 ? levelSum / wetNeighbors : waterLevelMeters;
    localWaterLevel[id] = refLevel;
    const h = Math.max(elevation[id] - refLevel, 12);
    phi[id] = Math.asinh(h / 160);
  }

  return { phi, strictMask, localWaterLevel, lakeMask };
}

/**
 * Shared GLSL noise, coastline zero-contour, natural water palette, and soft
 * relief lighting helpers injected into the SurfaceMap fragment shader.
 */
export const REFINEMENT_GLSL = `
float hash3(vec3 p) {
  p = fract(p * vec3(0.1031, 0.1030, 0.0973));
  p += dot(p, p.yxz + 33.33);
  return fract((p.x + p.y) * p.z);
}

float vnoise3(vec3 p) {
  vec3 i = floor(p);
  vec3 f = p - i;
  vec3 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  float n000 = hash3(i);
  float n100 = hash3(i + vec3(1.0, 0.0, 0.0));
  float n010 = hash3(i + vec3(0.0, 1.0, 0.0));
  float n110 = hash3(i + vec3(1.0, 1.0, 0.0));
  float n001 = hash3(i + vec3(0.0, 0.0, 1.0));
  float n101 = hash3(i + vec3(1.0, 0.0, 1.0));
  float n011 = hash3(i + vec3(0.0, 1.0, 1.0));
  float n111 = hash3(i + vec3(1.0, 1.0, 1.0));
  float nx00 = mix(n000, n100, u.x);
  float nx10 = mix(n010, n110, u.x);
  float nx01 = mix(n001, n101, u.x);
  float nx11 = mix(n011, n111, u.x);
  return mix(mix(nx00, nx10, u.y), mix(nx01, nx11, u.y), u.z);
}

// Analytical value + 3D gradient in a single 8-corner lookup (3x faster than finite differences).
vec4 vnoise3Grad(vec3 p) {
  vec3 i = floor(p);
  vec3 f = p - i;
  vec3 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  vec3 du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
  float a = hash3(i);
  float b = hash3(i + vec3(1.0, 0.0, 0.0));
  float c = hash3(i + vec3(0.0, 1.0, 0.0));
  float d = hash3(i + vec3(1.0, 1.0, 0.0));
  float e = hash3(i + vec3(0.0, 0.0, 1.0));
  float f1 = hash3(i + vec3(1.0, 0.0, 1.0));
  float g = hash3(i + vec3(0.0, 1.0, 1.0));
  float h = hash3(i + vec3(1.0, 1.0, 1.0));
  float k0 = a;
  float k1 = b - a;
  float k2 = c - a;
  float k3 = e - a;
  float k4 = a - b - c + d;
  float k5 = a - c - e + g;
  float k6 = a - b - e + f1;
  float k7 = -a + b + c - d + e - f1 - g + h;
  float val = k0 + k1*u.x + k2*u.y + k3*u.z + k4*u.x*u.y + k5*u.y*u.z + k6*u.z*u.x + k7*u.x*u.y*u.z;
  vec3 grad = du * vec3(
    k1 + k4*u.y + k6*u.z + k7*u.y*u.z,
    k2 + k5*u.z + k4*u.x + k7*u.z*u.x,
    k3 + k6*u.x + k5*u.y + k7*u.x*u.y
  );
  return vec4(val, grad);
}

float sampleCoastNoise(vec3 p, vec3 seedOff, float fineWeight) {
  vec3 warp = vec3(
    vnoise3(p * 34.0 + seedOff + vec3(3.1, -7.4, 11.2)) * 2.0 - 1.0,
    vnoise3(p * 34.0 + seedOff + vec3(-9.7, 5.3, -4.8)) * 2.0 - 1.0,
    vnoise3(p * 34.0 + seedOff + vec3(6.5, 12.1, 2.9)) * 2.0 - 1.0
  ) * 0.011;
  vec3 q = p + warp;
  float sum = 0.0;
  float amp = 0.54;
  float freq = 48.0;
  float norm = 0.0;
  for (int o = 0; o < 4; o++) {
    float fo = float(o);
    float n = vnoise3(q * freq + seedOff + vec3(fo * 13.7, fo * 7.3, fo * 19.1)) * 2.0 - 1.0;
    sum += n * amp;
    norm += amp;
    freq *= 2.12;
    amp *= 0.52;
  }
  if (fineWeight > 0.01) {
    float n4 = vnoise3(q * freq + seedOff + vec3(54.8, 29.2, 76.4)) * 2.0 - 1.0;
    sum += n4 * amp * fineWeight;
    norm += amp * fineWeight;
    freq *= 2.10;
    amp *= 0.50;
    float n5 = vnoise3(q * freq + seedOff + vec3(68.5, 36.5, 95.5)) * 2.0 - 1.0;
    sum += n5 * amp * fineWeight * 0.72;
    norm += amp * fineWeight * 0.72;
  }
  return sum / norm;
}

// Returns vec4(value, d/dx, d/dy, d/dz) of ridged + fBm terrain detail in one pass.
vec4 sampleDetailGrad(vec3 p, vec3 seedOff, float rugged, float fineWeight) {
  vec3 off = seedOff + vec3(17.3, -23.1, 9.7);
  float fbm = 0.0;
  vec3 fbmGrad = vec3(0.0);
  float ridge = 0.0;
  vec3 ridgeGrad = vec3(0.0);
  float amp = 0.54;
  float freq = 44.0;
  float norm = 0.0;
  float rw = 1.0;
  for (int o = 0; o < 4; o++) {
    float fo = float(o);
    vec4 ng = vnoise3Grad(p * freq + off + vec3(fo * 11.3, fo * 17.9, fo * 5.7));
    float s = ng.x * 2.0 - 1.0;
    vec3 ds = ng.yzw * (2.0 * freq);
    float ab = sqrt(s * s + 0.008);
    float t = 1.0 - ab;
    vec3 dt = -(s / ab) * ds;
    float r = t * t * rw;
    vec3 dr = (2.0 * t * rw) * dt;
    rw = clamp(r * 1.45, 0.25, 1.0);
    fbm += s * amp;
    fbmGrad += ds * amp;
    ridge += (r * 2.0 - 0.85) * amp;
    ridgeGrad += (2.0 * dr) * amp;
    norm += amp;
    freq *= 2.08;
    amp *= 0.50;
  }
  if (fineWeight > 0.01) {
    vec4 ng4 = vnoise3Grad(p * freq + off + vec3(45.2, 71.6, 22.8));
    float s4 = ng4.x * 2.0 - 1.0;
    vec3 ds4 = ng4.yzw * (2.0 * freq);
    float ab4 = sqrt(s4 * s4 + 0.008);
    float t4 = 1.0 - ab4;
    float r4 = t4 * t4 * rw;
    vec3 dr4 = (2.0 * t4 * rw) * (-(s4 / ab4) * ds4);
    float w4 = amp * fineWeight;
    fbm += s4 * w4;
    fbmGrad += ds4 * w4;
    ridge += (r4 * 2.0 - 0.85) * w4;
    ridgeGrad += (2.0 * dr4) * w4;
    norm += w4;
    freq *= 2.08;
    amp *= 0.48;
    vec4 ng5 = vnoise3Grad(p * freq + off + vec3(56.5, 89.5, 28.5));
    float s5 = ng5.x * 2.0 - 1.0;
    vec3 ds5 = ng5.yzw * (2.0 * freq);
    float w5 = amp * fineWeight * 0.72;
    fbm += s5 * w5;
    fbmGrad += ds5 * w5;
    ridge += ((1.0 - abs(s5)) * 1.5 - 0.75) * w5;
    ridgeGrad += (-sign(s5) * 1.5 * ds5) * w5;
    norm += w5;
  }
  float mixR = rugged * 0.62;
  return vec4(mix(fbm, ridge, mixR) / norm, mix(fbmGrad, ridgeGrad, mixR) / norm);
}

// Calm, luminous Blue-Marble ocean with smooth logarithmic bathymetry and distinct lakes.
vec3 naturalWater(float depth, float lakeMix) {
  float ld = log(1.0 + max(depth, 0.0) / 140.0) / log(1.0 + 6000.0 / 140.0);
  vec3 shelf = vec3(0.108, 0.236, 0.372);
  vec3 slope = vec3(0.068, 0.168, 0.308);
  vec3 deep  = vec3(0.042, 0.108, 0.228);
  vec3 abyss = vec3(0.028, 0.074, 0.172);
  vec3 ocean = mix(shelf, slope, smoothstep(0.0, 0.38, ld));
  ocean = mix(ocean, deep, smoothstep(0.32, 0.74, ld));
  ocean = mix(ocean, abyss, smoothstep(0.68, 1.0, ld));
  vec3 lake = mix(vec3(0.115, 0.258, 0.375), vec3(0.066, 0.175, 0.295), smoothstep(0.0, 500.0, depth));
  return mix(ocean, lake, clamp(lakeMix, 0.0, 1.0));
}
`;

export interface LodWorkerCache {
  surface: Surface;
  elevation: Float64Array;
  boundaryCorners: Uint32Array;
  cornerOffsets: Uint32Array;
  cornerRegions: Uint32Array;
  cornerElevation: Float32Array;
  fanNeighbors: Int32Array;
}

/**
 * Precompute shared dual-corner elevations and per-fan neighbor lookup once when
 * building view geometry, so camera-local LOD updates stay O(visible_regions).
 */
export function buildLodCache(
  surface: Surface,
  elevation: Float64Array,
  boundaryCorners: Uint32Array,
  cornerOffsets: Uint32Array,
  cornerRegions: Uint32Array,
): LodWorkerCache {
  const cornerCount = cornerOffsets.length - 1;
  const cornerElevation = new Float32Array(cornerCount);
  for (let k = 0; k < cornerCount; k++) {
    let sum = 0;
    const start = cornerOffsets[k], end = cornerOffsets[k + 1];
    for (let j = start; j < end; j++) sum += elevation[cornerRegions[j]];
    cornerElevation[k] = sum / Math.max(1, end - start);
  }

  // Map each directed/undirected corner pair (cA, cB) on a region's boundary to the
  // single neighbor region sharing that dual edge, so outer LOD edges match cleanly.
  const edgeOwner = new Map<number, number>();
  const n = surface.areasSquareMeters.length;
  const totalFans = surface.boundaryDirections.length / 3;
  const fanNeighbors = new Int32Array(totalFans).fill(-1);
  const edgeKey = (a: number, b: number): number => (a < b ? a * 4_294_967_296 + b : b * 4_294_967_296 + a);
  const fanOfEdge = new Map<number, number>();

  for (let id = 0; id < n; id++) {
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
    for (let j = start; j < end; j++) {
      const next = j + 1 === end ? start : j + 1;
      const key = edgeKey(boundaryCorners[j], boundaryCorners[next]);
      const otherRegion = edgeOwner.get(key);
      if (otherRegion === undefined) {
        edgeOwner.set(key, id);
        fanOfEdge.set(key, j);
      } else {
        const otherFan = fanOfEdge.get(key)!;
        fanNeighbors[j] = otherRegion;
        fanNeighbors[otherFan] = id;
      }
    }
  }

  return { surface, elevation, boundaryCorners, cornerOffsets, cornerRegions, cornerElevation, fanNeighbors };
}

export interface LodRequestParams {
  focus: Vec3;
  distance: number;
  seedOffset: Vec3;
  detailEnabled: boolean;
  waterLevelMeters: number;
  wetMask: Uint8Array;
  phi: Float32Array;
  strictMask: Float32Array;
  localWaterLevel: Float32Array;
  localReliefMeters: Float32Array;
}

/**
 * Build a watertight, bounded camera-local subdivided globe patch.
 * Regions inside the camera focus cone are subdivided into k×k fan sub-triangles;
 * outer perimeter edges bordering unrefined regions remain unsplit (1 segment)
 * with zero detail displacement so there are zero T-junctions or cracks.
 */
export function buildLodPatch(cache: LodWorkerCache, params: LodRequestParams): LodPatchData {
  const empty = (): LodPatchData => ({
    positions: new Float32Array(0),
    regions: new Float32Array(0),
    cornerPairs: new Float32Array(0),
    weights: new Float32Array(0),
    radialOffsets: new Float32Array(0),
    replacedRegions: new Uint32Array(0),
    subdivision: 1,
    triangleCount: 0,
  });

  if (params.distance >= 1.65) return empty();

  const { surface, elevation, boundaryCorners, cornerOffsets, cornerRegions, cornerElevation, fanNeighbors } = cache;
  const n = surface.areasSquareMeters.length;
  const radius = surface.radiusMeters;
  const centers = surface.centers;
  const boundaryDirs = surface.boundaryDirections;
  const { focus, distance, seedOffset, detailEnabled, wetMask, phi, strictMask, localWaterLevel, localReliefMeters } = params;

  let k = distance < 0.48 ? 4 : distance < 0.98 ? 3 : 2;
  const maxAngle = distance < 0.48
    ? clamp(distance * 0.95 + 0.14, 0.20, 0.62)
    : distance < 0.98
      ? clamp(distance * 0.85 + 0.14, 0.34, 0.92)
      : clamp(distance * 0.72 + 0.10, 0.50, 1.15);
  const cosLimit = Math.cos(maxAngle);

  // Select visible dry land and coastal ocean regions (open deep ocean stays coarse).
  const candidates: Array<{ id: number; dotVal: number; fans: number }> = [];
  for (let id = 0; id < n; id++) {
    const d = centers[id * 3] * focus[0] + centers[id * 3 + 1] * focus[1] + centers[id * 3 + 2] * focus[2];
    if (d < cosLimit) continue;
    let include = wetMask[id] === 0;
    if (!include) {
      // Include coastal ocean/lake cells touching at least one dry region so coastlines are continuous.
      for (let m = surface.neighborOffsets[id]; m < surface.neighborOffsets[id + 1]; m++) {
        if (wetMask[surface.neighbors[m]] === 0) { include = true; break; }
      }
    }
    if (!include) continue;
    const fans = surface.boundaryOffsets[id + 1] - surface.boundaryOffsets[id];
    candidates.push({ id, dotVal: d, fans });
  }

  if (!candidates.length) return empty();

  let totalFans = 0;
  for (const c of candidates) totalFans += c.fans;

  while (totalFans * k * k > MAX_LOD_TRIANGLES && k > 2) k--;
  let chosen = candidates;
  if (totalFans * k * k > MAX_LOD_TRIANGLES) {
    candidates.sort((a, b) => b.dotVal - a.dotVal);
    chosen = [];
    let accFans = 0;
    const maxFans = Math.floor(MAX_LOD_TRIANGLES / (k * k));
    for (const c of candidates) {
      if (accFans + c.fans > maxFans) break;
      chosen.push(c);
      accFans += c.fans;
    }
    totalFans = accFans;
  }

  if (!chosen.length) return empty();

  const inPatch = new Uint8Array(n);
  const replacedRegions = new Uint32Array(chosen.length);
  for (let i = 0; i < chosen.length; i++) {
    inPatch[chosen[i].id] = 1;
    replacedRegions[i] = chosen[i].id;
  }

  // Compute corner-averaged phi, strictMask, relief, and localWaterLevel for corners touched by chosen regions.
  const cornerCount = cornerOffsets.length - 1;
  const cornerPhi = new Float32Array(cornerCount);
  const cornerStrict = new Float32Array(cornerCount);
  const cornerRelief = new Float32Array(cornerCount);
  const cornerWaterLevel = new Float32Array(cornerCount);
  const cornerVisited = new Uint8Array(cornerCount);

  const ensureCorner = (c: number): void => {
    if (cornerVisited[c]) return;
    cornerVisited[c] = 1;
    let pSum = 0, mSum = 0, rSum = 0, wSum = 0;
    const start = cornerOffsets[c], end = cornerOffsets[c + 1];
    const count = Math.max(1, end - start);
    for (let j = start; j < end; j++) {
      const rid = cornerRegions[j];
      pSum += phi[rid];
      mSum += strictMask[rid];
      rSum += localReliefMeters[rid];
      wSum += localWaterLevel[rid];
    }
    cornerPhi[c] = pSum / count;
    cornerStrict[c] = mSum / count;
    cornerRelief[c] = rSum / count;
    cornerWaterLevel[c] = wSum / count;
  };

  // Count exact emitted triangles (interior fan: k*k; outer rim fan with unsplit edge AB: (k-1)^2 + k).
  let triangleCount = 0;
  for (const { id } of chosen) {
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
    for (let j = start; j < end; j++) {
      const nb = fanNeighbors[j];
      const outerSplit = nb >= 0 && inPatch[nb] === 1;
      triangleCount += outerSplit ? k * k : (k - 1) * (k - 1) + k;
    }
  }

  const vertexCount = triangleCount * 3;
  const positions = new Float32Array(vertexCount * 3);
  const regions = new Float32Array(vertexCount);
  const cornerPairs = new Float32Array(vertexCount * 2);
  const weights = new Float32Array(vertexCount * 3);
  const radialOffsets = new Float32Array(vertexCount);

  // Reusable per-fan grid arrays (max k = 4 -> 15 grid points)
  const maxGrid = ((k + 1) * (k + 2)) / 2;
  const gx = new Float32Array(maxGrid);
  const gy = new Float32Array(maxGrid);
  const gz = new Float32Array(maxGrid);
  const gw0 = new Float32Array(maxGrid);
  const gw1 = new Float32Array(maxGrid);
  const gw2 = new Float32Array(maxGrid);
  const goff = new Float32Array(maxGrid);

  const rowIndex = (r: number, c: number): number => (r * (r + 1)) / 2 + c;
  let vCursor = 0;

  const emitVertex = (gi: number, id: number, cA: number, cB: number): void => {
    const p3 = vCursor * 3;
    positions[p3] = gx[gi];
    positions[p3 + 1] = gy[gi];
    positions[p3 + 2] = gz[gi];
    regions[vCursor] = id;
    cornerPairs[vCursor * 2] = cA;
    cornerPairs[vCursor * 2 + 1] = cB;
    weights[p3] = gw0[gi];
    weights[p3 + 1] = gw1[gi];
    weights[p3 + 2] = gw2[gi];
    radialOffsets[vCursor] = goff[gi];
    vCursor++;
  };

  for (const { id } of chosen) {
    const cx = centers[id * 3], cy = centers[id * 3 + 1], cz = centers[id * 3 + 2];
    const elevC = elevation[id];
    const phiC = phi[id];
    const mC = strictMask[id];
    const relC = localReliefMeters[id];
    const wlC = localWaterLevel[id];
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];

    // Check if any corner of this region touches an unrefined neighbor; taper detail on outer rim.
    for (let j = start; j < end; j++) {
      const next = j + 1 === end ? start : j + 1;
      const cA = boundaryCorners[j], cB = boundaryCorners[next];
      ensureCorner(cA);
      ensureCorner(cB);
      const ax = boundaryDirs[j * 3], ay = boundaryDirs[j * 3 + 1], az = boundaryDirs[j * 3 + 2];
      const bx = boundaryDirs[next * 3], by = boundaryDirs[next * 3 + 1], bz = boundaryDirs[next * 3 + 2];
      const elevA = cornerElevation[cA], elevB = cornerElevation[cB];
      const phiA = cornerPhi[cA], phiB = cornerPhi[cB];
      const mA = cornerStrict[cA], mB = cornerStrict[cB];
      const relA = cornerRelief[cA], relB = cornerRelief[cB];
      const wlA = cornerWaterLevel[cA], wlB = cornerWaterLevel[cB];

      const nb = fanNeighbors[j];
      const outerSplit = nb >= 0 && inPatch[nb] === 1;
      // Also check whether corners cA / cB are incident to any region outside the patch,
      // so shared rim corners stay locked to exact base corner elevation (zero cracks!).
      let cAOuter = !outerSplit;
      if (!cAOuter) {
        for (let ci = cornerOffsets[cA]; ci < cornerOffsets[cA + 1]; ci++) {
          if (!inPatch[cornerRegions[ci]]) { cAOuter = true; break; }
        }
      }
      let cBOuter = !outerSplit;
      if (!cBOuter) {
        for (let ci = cornerOffsets[cB]; ci < cornerOffsets[cB + 1]; ci++) {
          if (!inPatch[cornerRegions[ci]]) { cBOuter = true; break; }
        }
      }

      for (let r = 0; r <= k; r++) {
        for (let c = 0; c <= r; c++) {
          const gi = rowIndex(r, c);
          const w0 = (k - r) / k;
          const w1 = (r - c) / k;
          const w2 = c / k;
          let px = w0 * cx + w1 * ax + w2 * bx;
          let py = w0 * cy + w1 * ay + w2 * by;
          let pz = w0 * cz + w1 * az + w2 * bz;
          const invLen = 1 / Math.hypot(px, py, pz);
          px *= invLen; py *= invLen; pz *= invLen;

          gx[gi] = px; gy[gi] = py; gz[gi] = pz;
          gw0[gi] = w0; gw1[gi] = w1; gw2[gi] = w2;

          const baseH = w0 * elevC + w1 * elevA + w2 * elevB;
          const wl = w0 * wlC + w1 * wlA + w2 * wlB;
          // Lock rim vertices touching unrefined regions to exact base height so seams are watertight.
          const isRimLocked = (!outerSplit && r === k) || (cAOuter && r === k && c === 0) || (cBOuter && r === k && c === k);
          if (isRimLocked) {
            goff[gi] = Math.max(baseH, wl) / radius;
          } else {
            // Smoothly fade detail on rim-adjacent rows so transition is visually seamless.
            let rimFade = 1;
            if (!outerSplit) rimFade = Math.min(rimFade, (k - r) / k);
            if (cAOuter) rimFade = Math.min(rimFade, 1 - w1 * 0.85);
            if (cBOuter) rimFade = Math.min(rimFade, 1 - w2 * 0.85);
            const phiBase = w0 * phiC + w1 * phiA + w2 * phiB;
            const mInterp = w0 * mC + w1 * mA + w2 * mB;
            const relInterp = w0 * relC + w1 * relA + w2 * relB;
            const rugged = smoothstep(80, 1400, relInterp);
            const phiDetail = sampleCoastPhi(px, py, pz, phiBase, mInterp, w0, rugged, seedOffset, detailEnabled && rimFade > 0.05);
            const detailM = sampleDetailHeightMeters(px, py, pz, relInterp, phiDetail, seedOffset, detailEnabled) * rimFade;
            if (phiDetail <= 0) {
              goff[gi] = wl / radius;
            } else {
              // Rise smoothly out of the zero-contour coastline with zero abrupt shoreline cliff.
              const shoreRise = smoothstep(0.0, 0.24, phiDetail);
              const landHeight = Math.max(0, baseH - wl) * (0.25 + 0.75 * shoreRise) + detailM;
              goff[gi] = (wl + Math.max(0, landHeight)) / radius;
            }
          }
        }
      }

      const uniformRows = outerSplit ? k : k - 1;
      for (let r = 0; r < uniformRows; r++) {
        for (let c = 0; c <= r; c++) {
          const i0 = rowIndex(r, c);
          const i1 = rowIndex(r + 1, c);
          const i2 = rowIndex(r + 1, c + 1);
          emitVertex(i0, id, cA, cB);
          emitVertex(i1, id, cA, cB);
          emitVertex(i2, id, cA, cB);
          if (c < r) {
            const i3 = rowIndex(r, c + 1);
            emitVertex(i0, id, cA, cB);
            emitVertex(i2, id, cA, cB);
            emitVertex(i3, id, cA, cB);
          }
        }
      }

      if (!outerSplit) {
        // Watertight transition strip: row (k - 1) has k vertices; row k keeps ONLY corners A and B.
        const idxA = rowIndex(k, 0);
        const idxB = rowIndex(k, k);
        const split = Math.floor((k - 1) / 2);
        for (let c = 0; c < split; c++) {
          emitVertex(rowIndex(k - 1, c), id, cA, cB);
          emitVertex(idxA, id, cA, cB);
          emitVertex(rowIndex(k - 1, c + 1), id, cA, cB);
        }
        // Center bridge triangle spans the unsplit outer edge A -> B.
        emitVertex(rowIndex(k - 1, split), id, cA, cB);
        emitVertex(idxA, id, cA, cB);
        emitVertex(idxB, id, cA, cB);
        for (let c = split; c < k - 1; c++) {
          emitVertex(rowIndex(k - 1, c), id, cA, cB);
          emitVertex(idxB, id, cA, cB);
          emitVertex(rowIndex(k - 1, c + 1), id, cA, cB);
        }
      }
    }
  }

  return { positions, regions, cornerPairs, weights, radialOffsets, replacedRegions, subdivision: k, triangleCount };
}
