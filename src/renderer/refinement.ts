import type { Surface } from '../core/surface';
import type { Drainage } from '../core/drainage';
import type { Rivers } from '../core/rivers';
import type { Vec3 } from '../core/vector';

export const MAX_LOD_TRIANGLES = 120_000;
const DEFAULT_MESH_FREQ = 53.1;

export interface CoastField {
  /** Per-region symmetric, corner-rounded signed coastline field (<0 wet, >0 dry, in [-1, 1]). */
  phi: Float32Array;
  /** Per-region strict wet/dry sign (-1 wet, +1 dry). */
  strictMask: Float32Array;
  /** Per-region local water surface elevation in meters (sea level or adjacent lake level). */
  localWaterLevel: Float32Array;
  /** Per-region lake marker: 0 = land or ocean, 1 = inland lake. */
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
 * Inverse nominal cell spacing on the unit sphere (radians^-1) for resolution-adaptive
 * coastline and fractal detail frequencies (~53 at subd 6, ~106 at subd 7).
 */
export function meshFrequencyForRegions(regionCount: number): number {
  return Math.sqrt(Math.max(12, regionCount)) / 3.81;
}

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

/**
 * Orthonormal 3D rotation applied between fractal octaves so cubic lattice axes
 * never align across octaves (eliminates horizontal/vertical grid creases on ridges).
 */
function rot3x(_x: number, y: number, z: number): number { return 0.80 * y + 0.60 * z; }
function rot3y(x: number, y: number, z: number): number { return -0.80 * x + 0.36 * y - 0.48 * z; }
function rot3z(x: number, y: number, z: number): number { return -0.60 * x - 0.48 * y + 0.64 * z; }

/**
 * Four-octave domain-warped coastline perturbation in [-1, 1] evaluated on the 3D unit sphere.
 * Frequencies scale with `meshFreq` so hex edges break into natural bays and headlands at every subdivision.
 */
export function sampleCoastNoise(
  x: number,
  y: number,
  z: number,
  seedOffset: Vec3,
  meshFreq = DEFAULT_MESH_FREQ,
): number {
  const ox = seedOffset[0], oy = seedOffset[1], oz = seedOffset[2];
  const mf = Math.max(18, meshFreq);
  const wf = mf * 1.15;
  const wa = 0.28 / mf;
  const wx = (vnoise3(x * wf + ox + 3.1, y * wf + oy - 7.4, z * wf + oz + 11.2) * 2 - 1) * wa;
  const wy = (vnoise3(x * wf + ox - 9.7, y * wf + oy + 5.3, z * wf + oz - 4.8) * 2 - 1) * wa;
  const wz = (vnoise3(x * wf + ox + 6.5, y * wf + oy + 12.1, z * wf + oz + 2.9) * 2 - 1) * wa;
  let px = x + wx, py = y + wy, pz = z + wz;
  let sum = 0;
  let amp = 0.56;
  let freq = mf * 1.45;
  let norm = 0;
  for (let o = 0; o < 4; o++) {
    const n = vnoise3(px * freq + ox + o * 13.7, py * freq + oy + o * 7.3, pz * freq + oz + o * 19.1) * 2 - 1;
    sum += n * amp;
    norm += amp;
    const nx = rot3x(px, py, pz), ny = rot3y(px, py, pz), nz = rot3z(px, py, pz);
    px = nx; py = ny; pz = nz;
    freq *= 2.06;
    amp *= 0.50;
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
  meshFreq = DEFAULT_MESH_FREQ,
): number {
  if (!detailEnabled) return phiBase;
  const coastBand = smoothstep(0.01, 0.28, 1.0 - Math.abs(mStrict));
  const centerGuard = 1.0 - smoothstep(0.68, 0.93, wCenter);
  const coastMask = coastBand * centerGuard;
  if (coastMask <= 1e-5) return phiBase;
  const coastAmp = (0.38 + 0.16 * clamp(ruggedness, 0, 1)) * coastMask;
  return phiBase + coastAmp * sampleCoastNoise(x, y, z, seedOffset, meshFreq);
}

/**
 * Bounded deterministic fractal terrain detail (in meters) for globe LOD vertices
 * and river height draping. Scaled by local relief and shaped by model drainage:
 * crisp ridges on mountain divides, lowered concave valley floors along rivers,
 * near-flat on plains, strictly zero on water and smoothly tapered at coastlines.
 */
export function sampleDetailHeightMeters(
  x: number,
  y: number,
  z: number,
  localReliefMeters: number,
  phiDetail: number,
  seedOffset: Vec3,
  detailEnabled: boolean,
  valleyFactor = 0,
  meshFreq = DEFAULT_MESH_FREQ,
): number {
  if (!detailEnabled || phiDetail <= 0 || localReliefMeters <= 10) return 0;
  const coastTaper = smoothstep(0.0, 0.30, phiDetail);
  if (coastTaper <= 1e-5) return 0;
  const reliefAmp = Math.min(540, localReliefMeters * 0.36) * coastTaper;
  const ox = seedOffset[0] + 17.3, oy = seedOffset[1] - 23.1, oz = seedOffset[2] + 9.7;
  let fbm = 0;
  let ridge = 0;
  let amp = 0.54;
  let freq = Math.max(44, meshFreq * 0.92);
  let norm = 0;
  let ridgeWeight = 1.0;
  let px = x, py = y, pz = z;
  for (let o = 0; o < 4; o++) {
    const raw = vnoise3(px * freq + ox + o * 11.3, py * freq + oy + o * 17.9, pz * freq + oz + o * 5.7);
    const signed = raw * 2 - 1;
    let r = 1.0 - Math.sqrt(signed * signed + 0.022);
    r = r * r * ridgeWeight;
    ridgeWeight = clamp(r * 1.45, 0.25, 1.0);
    fbm += signed * amp;
    ridge += (r * 2 - 0.85) * amp;
    norm += amp;
    const nx = rot3x(px, py, pz), ny = rot3y(px, py, pz), nz = rot3z(px, py, pz);
    px = nx; py = ny; pz = nz;
    freq *= 2.04;
    amp *= 0.45;
  }
  const v = clamp(valleyFactor, 0, 1);
  const rugged = smoothstep(100, 1050, localReliefMeters);
  const ridgeMix = rugged * 0.72 * (1 - 0.65 * v);
  let combined = ((1 - ridgeMix) * fbm + ridgeMix * ridge) / norm;
  // Attenuate positive ridges and carve concave valley floor along model drainage channels
  // so rivers lie in the lowest local detail while divides carry sharp peaks.
  if (v > 1e-4) {
    if (combined > 0) combined *= 1 - 0.85 * v;
    combined -= 0.42 * v;
  }
  return clamp(combined * reliefAmp, -540, 540);
}

/**
 * Compute per-region symmetric, corner-rounded signed coastline field (`phi` in [-1, 1])
 * and strict wet/dry mask (`-1` wet, `+1` dry). Two relaxation passes with symmetric
 * sign floors (±0.55) eliminate hex-grid sawtooth along straight coasts and 1-cell
 * peninsulas while keeping every single-cell island centered and smooth.
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
  const strictMask = new Float32Array(n);
  const localWaterLevel = new Float32Array(n);
  const lakeMask = new Float32Array(n);

  let cur = new Float32Array(n);
  let next = new Float32Array(n);
  for (let id = 0; id < n; id++) {
    if (wetMask[id]) {
      const level = surfaceLevelsMeters ? surfaceLevelsMeters[id] : elevation[id] + depthMeters[id];
      localWaterLevel[id] = Number.isFinite(level) ? level : waterLevelMeters;
      strictMask[id] = -1;
      cur[id] = -1;
      lakeMask[id] = mainOceanId > 0 && bodyIds[id] === mainOceanId ? 0 : 1;
    } else {
      strictMask[id] = 1;
      cur[id] = 1;
      lakeMask[id] = 0;
    }
  }

  // Two symmetric neighbor relaxation passes round hex corners without depth/elevation bias.
  for (let pass = 0; pass < 2; pass++) {
    for (let id = 0; id < n; id++) {
      const start = surface.neighborOffsets[id], end = surface.neighborOffsets[id + 1];
      let sum = 0;
      for (let k = start; k < end; k++) sum += cur[surface.neighbors[k]];
      next[id] = 0.5 * cur[id] + 0.5 * (sum / Math.max(1, end - start));
    }
    const tmp = cur; cur = next; next = tmp;
  }

  // Enforce symmetric sign floor (±0.55) so 1-region peninsulas stay connected with zero
  // sawtooth and 1-region islands place their zero-contour near mid-cell for full noise room.
  const phi = new Float32Array(n);
  for (let id = 0; id < n; id++) {
    const s = strictMask[id];
    phi[id] = s * Math.max(0.55, s * cur[id]);
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
    localWaterLevel[id] = wetNeighbors > 0 ? levelSum / wetNeighbors : waterLevelMeters;
  }

  return { phi, strictMask, localWaterLevel, lakeMask };
}

/**
 * Compute per-region display drainage-valley factor in [0, 1] from model contributing area
 * and river discharge. Interfluves / divides stay 0; major river trunks reach 1 so decorative
 * detail carves valleys along model drainage and places rivers in the lowest local elevation.
 */
export function computeValleyField(
  surface: Surface,
  drainage: Drainage,
  wetMask: ArrayLike<number>,
  rivers?: Rivers | null,
): Float32Array {
  const n = surface.areasSquareMeters.length;
  const valley = new Float32Array(n);
  for (let id = 0; id < n; id++) {
    if (wetMask[id]) continue;
    const area = Math.max(1, surface.areasSquareMeters[id]);
    const cells = Math.max(1, drainage.contributingArea[id] / area);
    let v = smoothstep(0.35, 3.6, Math.log(cells));
    if (rivers && rivers.meanDischarge[id] > 50) {
      const qv = smoothstep(Math.log(60), Math.log(6000), Math.log(rivers.meanDischarge[id]));
      if (qv > v) v = qv;
    }
    valley[id] = v;
  }
  return valley;
}

/**
 * Shared GLSL noise, resolution-adaptive coastline zero-contour, drainage-shaped
 * analytical fractal micro-relief, and natural bathymetric water helpers.
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

// Orthonormal 3D rotation (column-major, matching TS rot3x/rot3y/rot3z) applied between octaves.
const mat3 OCT_ROT = mat3(
   0.00, -0.80, -0.60,
   0.80,  0.36, -0.48,
   0.60, -0.48,  0.64
);
const mat3 OCT_ROT_T = mat3(
   0.00,  0.80,  0.60,
  -0.80,  0.36, -0.48,
  -0.60, -0.48,  0.64
);

float sampleCoastNoise(vec3 p, vec3 seedOff, float meshFreq, float pixelArc) {
  float mf = max(18.0, meshFreq);
  float wf = mf * 1.15;
  float wa = 0.28 / mf;
  vec3 warp = vec3(
    vnoise3(p * wf + seedOff + vec3(3.1, -7.4, 11.2)) * 2.0 - 1.0,
    vnoise3(p * wf + seedOff + vec3(-9.7, 5.3, -4.8)) * 2.0 - 1.0,
    vnoise3(p * wf + seedOff + vec3(6.5, 12.1, 2.9)) * 2.0 - 1.0
  ) * wa;
  vec3 q = p + warp;
  float sum = 0.0;
  float amp = 0.56;
  float freq = mf * 1.45;
  float norm = 0.0;
  for (int o = 0; o < 6; o++) {
    float nyq = 1.0 - smoothstep(0.35, 0.95, pixelArc * freq);
    if (o >= 4 && nyq < 0.01) break;
    float fo = float(o);
    float n = vnoise3(q * freq + seedOff + vec3(fo * 13.7, fo * 7.3, fo * 19.1)) * 2.0 - 1.0;
    float w = amp * max(o < 2 ? 0.25 : 0.0, nyq);
    sum += n * w;
    norm += w;
    q = OCT_ROT * q;
    freq *= 2.06;
    amp *= 0.50;
  }
  return sum / max(norm, 0.001);
}

// Band-limited, octave-rotated analytical ridged + fBm terrain detail.
// Each octave rotates by OCT_ROT (removing grid creases), transforms gradients back via
// transpose(rot), decays slope energy cleanly (0.43 * 2.04 = 0.88/octave), and fades
// before Nyquist so orbit stays silky smooth while close-ups show crisp ridges & spurs.
vec4 sampleDetailGrad(vec3 p, vec3 seedOff, float rugged, float valley, float meshFreq, float pixelArc) {
  vec3 off = seedOff + vec3(17.3, -23.1, 9.7);
  float fbm = 0.0;
  vec3 fbmGrad = vec3(0.0);
  float ridge = 0.0;
  vec3 ridgeGrad = vec3(0.0);
  float amp = 0.56;
  float gAmp = 0.56;
  float freq = max(44.0, meshFreq * 0.88);
  float norm = 0.0;
  float gNorm = 0.0;
  float rw = 1.0;
  vec3 q = p;
  mat3 rotT = mat3(1.0);
  for (int o = 0; o < 7; o++) {
    float nyq = 1.0 - smoothstep(0.30, 0.88, pixelArc * freq);
    if (o >= 3 && nyq < 0.01) break;
    float fo = float(o);
    vec4 ng = vnoise3Grad(q * freq + off + vec3(fo * 11.3, fo * 17.9, fo * 5.7));
    float s = ng.x * 2.0 - 1.0;
    vec3 ds = rotT * (ng.yzw * (2.0 * freq));
    float ab = sqrt(s * s + 0.018);
    float t = 1.0 - ab;
    vec3 dt = -(s / ab) * ds;
    float r = t * t * rw;
    vec3 dr = (2.0 * t * rw) * dt;
    rw = clamp(r * 1.44, 0.24, 1.0);
    float w = amp * nyq;
    float gw = gAmp * nyq;
    fbm += s * w;
    fbmGrad += ds * gw;
    ridge += (r * 2.0 - 0.85) * w;
    ridgeGrad += (2.0 * dr) * gw;
    norm += w;
    gNorm += gw;
    // Rotate domain for next octave and track inverse transpose for exact sphere-space gradients.
    q = OCT_ROT * q;
    rotT = rotT * OCT_ROT_T;
    freq *= 2.04;
    amp *= 0.46;
    gAmp *= 0.43;
  }
  float mixR = rugged * 0.72 * (1.0 - 0.65 * clamp(valley, 0.0, 1.0));
  float invN = 1.0 / max(norm, 0.001);
  float invGN = 1.0 / max(gNorm, 0.001);
  return vec4(mix(fbm, ridge, mixR) * invN, mix(fbmGrad, ridgeGrad, mixR) * invGN);
}

// Calm, luminous Blue-Marble ocean calibrated to 9 km so v5 shelves, ridges and trenches all read cleanly.
vec3 naturalWater(float depth, float lakeMix) {
  float ld = log(1.0 + max(depth, 0.0) / 130.0) / log(1.0 + 9000.0 / 130.0);
  vec3 shoal = vec3(0.135, 0.295, 0.430);
  vec3 shelf = vec3(0.098, 0.228, 0.368);
  vec3 slope = vec3(0.062, 0.158, 0.298);
  vec3 deep  = vec3(0.038, 0.098, 0.214);
  vec3 trench = vec3(0.020, 0.056, 0.140);
  vec3 ocean = mix(shoal, shelf, smoothstep(0.0, 0.18, ld));
  ocean = mix(ocean, slope, smoothstep(0.16, 0.50, ld));
  ocean = mix(ocean, deep, smoothstep(0.46, 0.80, ld));
  ocean = mix(ocean, trench, smoothstep(0.76, 1.0, ld));
  vec3 lake = mix(vec3(0.125, 0.275, 0.392), vec3(0.064, 0.172, 0.292), smoothstep(0.0, 480.0, depth));
  return mix(ocean, lake, clamp(lakeMix, 0.0, 1.0));
}
`;

export interface LodWorkerCache {
  surface: Surface;
  elevation: Float64Array;
  /** Compact per-region ring of true dual corners (5 or 6 per region, omitting edge midpoints). */
  ringOffsets: Uint32Array;
  ringCorners: Uint32Array;
  ringDirections: Float32Array;
  cornerOffsets: Uint32Array;
  cornerRegions: Uint32Array;
  cornerElevation: Float32Array;
  fanNeighbors: Int32Array;
  meshFreq: number;
}

/**
 * Precompute compact 5/6-corner dual topology, shared corner elevations, and per-fan
 * neighbor lookup once when building view geometry, so camera-local LOD updates stay
 * O(visible_regions) and cover 2x more area per triangle budget than 12-fan rings.
 */
export function buildLodCache(
  surface: Surface,
  elevation: Float64Array,
  boundaryCorners: Uint32Array,
  cornerOffsets: Uint32Array,
  cornerRegions: Uint32Array,
  compactRing?: { ringOffsets: Uint32Array; ringCorners: Uint32Array; ringDirections: Float32Array },
): LodWorkerCache {
  const n = surface.areasSquareMeters.length;
  const cornerCount = cornerOffsets.length - 1;
  const cornerElevation = new Float32Array(cornerCount);
  for (let k = 0; k < cornerCount; k++) {
    let sum = 0;
    const start = cornerOffsets[k], end = cornerOffsets[k + 1];
    for (let j = start; j < end; j++) sum += elevation[cornerRegions[j]];
    cornerElevation[k] = sum / Math.max(1, end - start);
  }

  let ringOffsets: Uint32Array;
  let ringCorners: Uint32Array;
  let ringDirections: Float32Array;

  if (compactRing) {
    ringOffsets = compactRing.ringOffsets;
    ringCorners = compactRing.ringCorners;
    ringDirections = compactRing.ringDirections;
  } else {
    // Filter out degree-2 edge midpoints so each hex/pentagon has 6 or 5 dual corners.
    const keptCounts = new Uint8Array(n);
    let totalKept = 0;
    for (let id = 0; id < n; id++) {
      let c = 0;
      const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
      for (let j = start; j < end; j++) {
        const cid = boundaryCorners[j];
        if (cornerOffsets[cid + 1] - cornerOffsets[cid] >= 3) c++;
      }
      if (c < 3) c = end - start;
      keptCounts[id] = c;
      totalKept += c;
    }
    ringOffsets = new Uint32Array(n + 1);
    ringCorners = new Uint32Array(totalKept);
    ringDirections = new Float32Array(totalKept * 3);
    const dirs = surface.boundaryDirections;
    let cursor = 0;
    for (let id = 0; id < n; id++) {
      ringOffsets[id] = cursor;
      const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
      const filterDeg3 = keptCounts[id] < end - start;
      for (let j = start; j < end; j++) {
        const cid = boundaryCorners[j];
        if (filterDeg3 && cornerOffsets[cid + 1] - cornerOffsets[cid] < 3) continue;
        ringCorners[cursor] = cid;
        ringDirections[cursor * 3] = dirs[j * 3];
        ringDirections[cursor * 3 + 1] = dirs[j * 3 + 1];
        ringDirections[cursor * 3 + 2] = dirs[j * 3 + 2];
        cursor++;
      }
    }
    ringOffsets[n] = cursor;
  }

  // Map each shared dual edge (cA, cB) to the single neighbor region across it.
  const totalFans = ringCorners.length;
  const fanNeighbors = new Int32Array(totalFans).fill(-1);
  const edgeOwner = new Map<number, number>();
  const fanOfEdge = new Map<number, number>();
  const edgeKey = (a: number, b: number): number => (a < b ? a * 4_294_967_296 + b : b * 4_294_967_296 + a);

  for (let id = 0; id < n; id++) {
    const start = ringOffsets[id], end = ringOffsets[id + 1];
    for (let j = start; j < end; j++) {
      const next = j + 1 === end ? start : j + 1;
      const key = edgeKey(ringCorners[j], ringCorners[next]);
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

  return {
    surface,
    elevation,
    ringOffsets,
    ringCorners,
    ringDirections,
    cornerOffsets,
    cornerRegions,
    cornerElevation,
    fanNeighbors,
    meshFreq: meshFrequencyForRegions(n),
  };
}

export interface LodRequestParams {
  focus: Vec3;
  distance: number;
  tiltRad?: number;
  forward?: Vec3;
  seedOffset: Vec3;
  detailEnabled: boolean;
  waterLevelMeters: number;
  wetMask: Uint8Array;
  phi: Float32Array;
  strictMask: Float32Array;
  localWaterLevel: Float32Array;
  localReliefMeters: Float32Array;
  effectiveReliefMeters?: Float32Array;
  valleyFactor?: Float32Array;
}

/**
 * Build a watertight, bounded camera-local subdivided globe patch.
 * Regions inside the camera focus cone are subdivided into k×k fan sub-triangles;
 * outer perimeter edges bordering unrefined regions remain unsplit (1 segment)
 * with locked base corner heights so there are zero T-junctions or cracks.
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

  const { surface, elevation, ringOffsets, ringCorners, ringDirections, cornerOffsets, cornerRegions, cornerElevation, fanNeighbors, meshFreq } = cache;
  const n = surface.areasSquareMeters.length;
  const radius = surface.radiusMeters;
  const centers = surface.centers;
  const { focus, distance, seedOffset, detailEnabled, wetMask, phi, strictMask, localWaterLevel, localReliefMeters } = params;
  const reliefField = params.effectiveReliefMeters ?? localReliefMeters;
  const valleyField = params.valleyFactor;
  const tiltRad = params.tiltRad ?? 0;

  // When tilted, shift the LOD focus slightly forward toward the viewed horizon so
  // the full tilted viewport receives high-detail subdivision instead of wasting budget behind the camera.
  let fx = focus[0], fy = focus[1], fz = focus[2];
  if (tiltRad > 0.08 && params.forward) {
    const shift = Math.min(0.24, distance * Math.sin(tiltRad) * 0.55);
    fx += params.forward[0] * shift;
    fy += params.forward[1] * shift;
    fz += params.forward[2] * shift;
    const invF = 1 / Math.hypot(fx, fy, fz);
    fx *= invF; fy *= invF; fz *= invF;
  }

  let k = distance < 0.52 ? 4 : distance < 1.05 ? 3 : 2;
  const tiltBoost = Math.sin(clamp(tiltRad, 0, 1.35)) * distance * 0.42;
  // Tight frustum-matched cone keeps subdivision-7 close-ups at k = 4 inside the triangle budget.
  const maxAngle = distance < 0.52
    ? clamp(distance * 0.72 + tiltBoost + 0.075, 0.11, 0.52)
    : distance < 1.05
      ? clamp(distance * 0.68 + tiltBoost + 0.09, 0.28, 0.78)
      : clamp(distance * 0.62 + 0.08, 0.44, 1.02);
  const cosLimit = Math.cos(maxAngle);

  // Select visible dry land and coastal ocean regions (open deep ocean stays coarse).
  const candidates: Array<{ id: number; dotVal: number; fans: number }> = [];
  for (let id = 0; id < n; id++) {
    const d = centers[id * 3] * fx + centers[id * 3 + 1] * fy + centers[id * 3 + 2] * fz;
    if (d < cosLimit) continue;
    let include = wetMask[id] === 0;
    if (!include) {
      for (let m = surface.neighborOffsets[id]; m < surface.neighborOffsets[id + 1]; m++) {
        if (wetMask[surface.neighbors[m]] === 0) { include = true; break; }
      }
    }
    if (!include) continue;
    const fans = ringOffsets[id + 1] - ringOffsets[id];
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

  const cornerCount = cornerOffsets.length - 1;
  const cornerPhi = new Float32Array(cornerCount);
  const cornerStrict = new Float32Array(cornerCount);
  const cornerRelief = new Float32Array(cornerCount);
  const cornerWaterLevel = new Float32Array(cornerCount);
  const cornerValley = new Float32Array(cornerCount);
  const cornerVisited = new Uint8Array(cornerCount);

  const ensureCorner = (c: number): void => {
    if (cornerVisited[c]) return;
    cornerVisited[c] = 1;
    let pSum = 0, mSum = 0, rSum = 0, wSum = 0, vSum = 0;
    const start = cornerOffsets[c], end = cornerOffsets[c + 1];
    const count = Math.max(1, end - start);
    for (let j = start; j < end; j++) {
      const rid = cornerRegions[j];
      pSum += phi[rid];
      mSum += strictMask[rid];
      rSum += reliefField[rid];
      wSum += localWaterLevel[rid];
      if (valleyField) vSum += valleyField[rid];
    }
    cornerPhi[c] = pSum / count;
    cornerStrict[c] = mSum / count;
    cornerRelief[c] = rSum / count;
    cornerWaterLevel[c] = wSum / count;
    cornerValley[c] = vSum / count;
  };

  let triangleCount = 0;
  for (const { id } of chosen) {
    const start = ringOffsets[id], end = ringOffsets[id + 1];
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
    const relC = reliefField[id];
    const wlC = localWaterLevel[id];
    const valC = valleyField ? valleyField[id] : 0;
    const start = ringOffsets[id], end = ringOffsets[id + 1];

    for (let j = start; j < end; j++) {
      const next = j + 1 === end ? start : j + 1;
      const cA = ringCorners[j], cB = ringCorners[next];
      ensureCorner(cA);
      ensureCorner(cB);
      const ax = ringDirections[j * 3], ay = ringDirections[j * 3 + 1], az = ringDirections[j * 3 + 2];
      const bx = ringDirections[next * 3], by = ringDirections[next * 3 + 1], bz = ringDirections[next * 3 + 2];
      const elevA = cornerElevation[cA], elevB = cornerElevation[cB];
      const phiA = cornerPhi[cA], phiB = cornerPhi[cB];
      const mA = cornerStrict[cA], mB = cornerStrict[cB];
      const relA = cornerRelief[cA], relB = cornerRelief[cB];
      const wlA = cornerWaterLevel[cA], wlB = cornerWaterLevel[cB];
      const valA = cornerValley[cA], valB = cornerValley[cB];

      const nb = fanNeighbors[j];
      const outerSplit = nb >= 0 && inPatch[nb] === 1;
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
          const isRimLocked = (!outerSplit && r === k) || (cAOuter && r === k && c === 0) || (cBOuter && r === k && c === k);
          if (isRimLocked) {
            goff[gi] = Math.max(baseH, wl) / radius;
          } else {
            let rimFade = 1;
            if (!outerSplit) rimFade = Math.min(rimFade, (k - r) / k);
            if (cAOuter) rimFade = Math.min(rimFade, 1 - w1 * 0.85);
            if (cBOuter) rimFade = Math.min(rimFade, 1 - w2 * 0.85);
            const phiBase = w0 * phiC + w1 * phiA + w2 * phiB;
            const mInterp = w0 * mC + w1 * mA + w2 * mB;
            const relInterp = w0 * relC + w1 * relA + w2 * relB;
            const valInterp = w0 * valC + w1 * valA + w2 * valB;
            const rugged = smoothstep(95, 1150, relInterp);
            const phiDetail = sampleCoastPhi(px, py, pz, phiBase, mInterp, w0, rugged, seedOffset, detailEnabled && rimFade > 0.05, meshFreq);
            const detailM = sampleDetailHeightMeters(px, py, pz, relInterp, phiDetail, seedOffset, detailEnabled, valInterp, meshFreq) * rimFade;
            if (phiDetail <= 0) {
              goff[gi] = wl / radius;
            } else {
              const shoreRise = smoothstep(0.0, 0.30, phiDetail);
              const landHeight = Math.max(0, baseH - wl) * (0.20 + 0.80 * shoreRise) + detailM;
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
        const idxA = rowIndex(k, 0);
        const idxB = rowIndex(k, k);
        const split = Math.floor((k - 1) / 2);
        for (let c = 0; c < split; c++) {
          emitVertex(rowIndex(k - 1, c), id, cA, cB);
          emitVertex(idxA, id, cA, cB);
          emitVertex(rowIndex(k - 1, c + 1), id, cA, cB);
        }
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
