import type { World } from '../core/world';
import type { ClimateNormals } from '../core/climate-normals';
import { aridityThreshold } from '../core/climate-normals';
import type { VegetationFrame } from '../core/vegetation';
import { COVER_ARIDITY, WOODY_ARIDITY } from '../core/vegetation';
import type { TemperatureNormals } from '../core/seasonal-temperature';

export type SurfaceStyle = 'natural' | 'hypsometric';

type Rgb = readonly [number, number, number];

export interface ReliefMetrics {
  /** Maximum elevation difference to any neighbor on dry land (meters); 0 on water. */
  localReliefMeters: Float32Array;
  /** Dimensionless ruggedness in [0, 1] scaled for display rock/detail blending. */
  ruggedness: Float32Array;
  /** Dimensionless raw slope magnitude (rise / run) for steep-rock exposure. */
  slope: Float32Array;
}

export interface NaturalSurfaceData {
  /** Per-region RGBA float texels (r, g, b in [0, 1], a = rockiness/alpine exposure in [0, 1]). */
  colors: Float32Array;
  /** Per-region water kind: 0 = land, 1 = ocean, 2 = inland lake. */
  waterKind: Uint8Array;
}

const clamp = (v: number, lo: number, hi: number): number => (v < lo ? lo : v > hi ? hi : v);
const smoothstep = (e0: number, e1: number, x: number): number => {
  const t = clamp((x - e0) / (e1 - e0), 0, 1);
  return t * t * (3 - 2 * t);
};
const mixRgb = (a: Rgb, b: Rgb, t: number): [number, number, number] => [
  a[0] + (b[0] - a[0]) * t,
  a[1] + (b[1] - a[1]) * t,
  a[2] + (b[2] - a[2]) * t,
];

/**
 * Calm, Blue-Marble-inspired sRGB anchors (0..1) for natural surface coloring.
 * Forest greens stay deep and desaturated so relief, deserts, grasslands and ice
 * read like orbital satellite imagery rather than saturated game terrain.
 */
const COLOR_ICE: Rgb = [0.89, 0.915, 0.935];
const COLOR_ALPINE_SNOW: Rgb = [0.87, 0.895, 0.92];
const COLOR_TUNDRA_DRY: Rgb = [0.47, 0.46, 0.39];
const COLOR_TUNDRA_MOIST: Rgb = [0.39, 0.425, 0.315];

const COLOR_HOT_DESERT_SAND: Rgb = [0.745, 0.665, 0.525];
const COLOR_HOT_DESERT_OCHRE: Rgb = [0.685, 0.565, 0.425];
const COLOR_COLD_DESERT: Rgb = [0.585, 0.545, 0.485];
const COLOR_ARID_STEPPE: Rgb = [0.575, 0.555, 0.415];

const COLOR_SAVANNA_DRY: Rgb = [0.545, 0.535, 0.345];
const COLOR_SAVANNA_MOIST: Rgb = [0.415, 0.485, 0.285];
const COLOR_GRASS_DRY: Rgb = [0.505, 0.525, 0.365];
const COLOR_GRASS_MOIST: Rgb = [0.375, 0.455, 0.295];

const COLOR_BOREAL_FOREST: Rgb = [0.155, 0.235, 0.19];
const COLOR_TEMPERATE_FOREST: Rgb = [0.175, 0.275, 0.165];
const COLOR_TROPICAL_SEASONAL: Rgb = [0.195, 0.295, 0.155];
const COLOR_TROPICAL_RAINFOREST: Rgb = [0.125, 0.235, 0.135];

const COLOR_ROCK_WARM: Rgb = [0.495, 0.455, 0.405];
const COLOR_ROCK_ALPINE: Rgb = [0.43, 0.425, 0.42];

/** Deterministic subtle per-region variation in [-1, 1] without consuming model RNG. */
function hashRegion(id: number, salt: number): number {
  let h = (Math.imul(id + 1, 0x9e3779b1) ^ Math.imul(salt + 1, 0x85ebca6b)) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d) >>> 0;
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39) >>> 0;
  h = (h ^ (h >>> 15)) >>> 0;
  return (h / 0xffffffff) * 2 - 1;
}

/**
 * Compute per-region display relief metrics from model elevation and wet mask.
 * Pure display helper; never modifies physical elevation or water arrays.
 */
export function computeReliefMetrics(
  surface: World['surface'],
  elevation: Float64Array,
  wetMask: ArrayLike<number>,
  waterLevelMeters: number,
): ReliefMetrics {
  const n = elevation.length;
  const localReliefMeters = new Float32Array(n);
  const ruggedness = new Float32Array(n);
  const slope = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    if (wetMask[i]) continue;
    const h0 = Math.max(elevation[i], waterLevelMeters);
    let maxDiff = 0;
    let maxSlope = 0;
    let sumSq = 0;
    let count = 0;
    for (let k = surface.neighborOffsets[i]; k < surface.neighborOffsets[i + 1]; k++) {
      const j = surface.neighbors[k];
      const hj = wetMask[j] ? waterLevelMeters : Math.max(elevation[j], waterLevelMeters);
      const diff = Math.abs(hj - h0);
      const dist = surface.neighborDistancesMeters[k];
      if (diff > maxDiff) maxDiff = diff;
      if (dist > 1) {
        const s = diff / dist;
        if (s > maxSlope) maxSlope = s;
      }
      sumSq += diff * diff;
      count++;
    }
    const rms = count > 0 ? Math.sqrt(sumSq / count) : 0;
    // Blend RMS and max difference so ridges and escarpments both carry crisp detail.
    const relief = 0.55 * rms + 0.45 * maxDiff;
    localReliefMeters[i] = relief;
    slope[i] = maxSlope;
    ruggedness[i] = smoothstep(80, 1400, relief);
  }
  return { localReliefMeters, ruggedness, slope };
}

/**
 * Fallback woody & herb fractions derived directly from climate-normals-1 before
 * vegetation-2 frames arrive. Uses the same smooth Köppen aridity ramps.
 */
function fallbackCoverFromClimate(climate: ClimateNormals, id: number): readonly [number, number] {
  const warm = climate.warmestMonthCelsius[id];
  if (warm < 0) return [0, 0];
  const mean = climate.annualMeanCelsius[id];
  const threshold = aridityThreshold(mean);
  const ratio = threshold <= 0 ? 10 : climate.annualPrecipitationMm[id] / threshold;
  const total = smoothstep(COVER_ARIDITY[0], COVER_ARIDITY[1], ratio);
  if (warm < 10) return [0, total * clamp(warm / 10, 0, 1)];
  const woody = smoothstep(WOODY_ARIDITY[0], WOODY_ARIDITY[1], ratio);
  return [woody, Math.max(0, total - woody)];
}

/**
 * Compute calm, natural satellite-style colors for every region from model fields.
 * Never mutates simulation arrays. Wet regions receive a marker in `waterKind`
 * (ocean vs inland lake) and borrow neighbor dry colors during corner blending.
 */
export function computeNaturalSurface(
  world: World,
  climate: ClimateNormals | null,
  vegetation: VegetationFrame | null,
  temperature: TemperatureNormals | null,
  relief: ReliefMetrics,
  options: { month: number; seasonalSnow: boolean; wetMask?: ArrayLike<number>; bodyIds?: ArrayLike<number>; mainOceanId?: number },
): NaturalSurfaceData {
  const n = world.stats.regionCount;
  const colors = new Float32Array(n * 4);
  const waterKind = new Uint8Array(n);
  const wetMask = options.wetMask ?? world.water.bodyIds;
  const bodyIds = options.bodyIds ?? world.water.bodyIds;
  const mainOcean = options.mainOceanId ?? world.water.mainOceanId;
  const seaLevel = world.water.levelMeters;
  const elevation = world.terrain.elevation;
  const centers = world.surface.centers;
  const monthlyTemp = options.seasonalSnow && temperature ? temperature.monthlyTemperatureCelsius[options.month] : null;

  for (let id = 0; id < n; id++) {
    if (wetMask[id]) {
      waterKind[id] = mainOcean > 0 && bodyIds[id] === mainOcean ? 1 : 2;
      continue;
    }

    const elevAboveSea = Math.max(0, elevation[id] - seaLevel);
    const localRelief = relief.localReliefMeters[id];
    const localSlope = relief.slope[id];
    const absLat = Math.abs(Math.asin(clamp(centers[id * 3 + 1], -1, 1)));
    const variation = hashRegion(id, 17);

    if (!climate) {
      // Earliest fallback before climate normals load: calm latitude + altitude natural tones.
      const polar = smoothstep(1.12, 1.36, absLat + elevAboveSea / 9500);
      const subtropic = Math.exp(-Math.pow((absLat - 0.44) / 0.24, 2));
      let rgb = mixRgb(COLOR_TEMPERATE_FOREST, COLOR_SAVANNA_DRY, subtropic * 0.65);
      rgb = mixRgb(rgb, COLOR_ROCK_ALPINE, smoothstep(1800, 3800, elevAboveSea) * 0.7);
      rgb = mixRgb(rgb, COLOR_ICE, polar);
      colors[id * 4] = rgb[0];
      colors[id * 4 + 1] = rgb[1];
      colors[id * 4 + 2] = rgb[2];
      colors[id * 4 + 3] = smoothstep(900, 3200, elevAboveSea) * 0.7;
      continue;
    }

    const mean = climate.annualMeanCelsius[id];
    const warm = climate.warmestMonthCelsius[id];
    const cold = climate.coldestMonthCelsius[id];
    const precip = climate.annualPrecipitationMm[id];
    const driest = climate.driestMonthMm[id];
    const threshold = aridityThreshold(mean);
    const aridity = threshold <= 0 ? 4 : precip / threshold;

    const [woody, herb] = vegetation
      ? [clamp(vegetation.woody[id], 0, 1), clamp(vegetation.herb[id], 0, 1)]
      : fallbackCoverFromClimate(climate, id);

    // 1. Bare soil / desert substrate according to warmth & aridity
    const warmth = smoothstep(6, 22, mean);
    const desertMix = smoothstep(0.35, 1.15, aridity);
    const hotDesert = mixRgb(COLOR_HOT_DESERT_SAND, COLOR_HOT_DESERT_OCHRE, clamp(0.45 + 0.35 * variation + 0.25 * (1 - desertMix), 0, 1));
    let bare = mixRgb(COLOR_COLD_DESERT, hotDesert, warmth);
    bare = mixRgb(bare, COLOR_ARID_STEPPE, smoothstep(0.65, 1.35, aridity) * 0.45);

    // Tundra substrate blends in cleanly where warmest month is below 10 °C
    const tundraFactor = 1 - smoothstep(4.5, 11.5, warm);
    const tundraBase = mixRgb(COLOR_TUNDRA_DRY, COLOR_TUNDRA_MOIST, smoothstep(0.6, 2.0, aridity));
    bare = mixRgb(bare, tundraBase, tundraFactor);

    // 2. Herbaceous cover (grassland / steppe / savanna / tundra moss)
    const moistureTint = smoothstep(0.8, 2.2, aridity);
    const warmGrass = mixRgb(COLOR_SAVANNA_DRY, COLOR_SAVANNA_MOIST, moistureTint);
    const coolGrass = mixRgb(COLOR_GRASS_DRY, COLOR_GRASS_MOIST, moistureTint);
    let herbColor = mixRgb(coolGrass, warmGrass, smoothstep(10, 21, mean));
    herbColor = mixRgb(herbColor, COLOR_TUNDRA_MOIST, tundraFactor * 0.75);

    // 3. Woody / forest canopy color (boreal → temperate → tropical seasonal → rainforest)
    const tropicalness = smoothstep(13, 19, cold);
    const evergreenTropical = smoothstep(35, 85, driest);
    const tropicalForest = mixRgb(COLOR_TROPICAL_SEASONAL, COLOR_TROPICAL_RAINFOREST, evergreenTropical);
    const temperateWeight = smoothstep(-5, 3, cold) * (1 - tropicalness) + smoothstep(19, 24, warm) * (1 - tropicalness);
    const midLatForest = mixRgb(COLOR_BOREAL_FOREST, COLOR_TEMPERATE_FOREST, clamp(temperateWeight, 0, 1));
    const forestColor = mixRgb(midLatForest, tropicalForest, tropicalness);

    // Blend substrate → herbs → woody canopy smoothly
    const totalCover = clamp(woody + herb, 0, 1);
    let rgb = mixRgb(bare, herbColor, smoothstep(0.05, 0.85, totalCover));
    rgb = mixRgb(rgb, forestColor, smoothstep(0.12, 0.82, woody));

    // 4. Bare rock on steep slopes and high mountain belts (exposes granite/slate through cover)
    const highRock = smoothstep(1750, 3900, elevAboveSea);
    const steepRock = smoothstep(0.008, 0.026, localSlope) * smoothstep(350, 1400, localRelief);
    const rockExposure = clamp(Math.max(highRock * 0.82, steepRock * 0.78) * (1 - woody * 0.35), 0, 0.88);
    const rockColor = mixRgb(COLOR_ROCK_ALPINE, COLOR_ROCK_WARM, warmth * (1 - highRock * 0.7));
    rgb = mixRgb(rgb, rockColor, rockExposure);

    // 5. Permanent polar & high-alpine snow/ice caps (warmest month below ~0 °C)
    const permanentIce = 1 - smoothstep(-1.2, 1.4, warm);
    // High alpine snowline: summits whose warmest month stays near freezing and have moisture
    const alpineSnow = (1 - smoothstep(0.5, 4.2, warm)) * smoothstep(2300, 4400, elevAboveSea) * smoothstep(80, 350, precip) * (1 - steepRock * 0.45);
    const iceMask = clamp(Math.max(permanentIce, alpineSnow * 0.85), 0, 1);
    const snowTone = mixRgb(COLOR_ALPINE_SNOW, COLOR_ICE, permanentIce);
    rgb = mixRgb(rgb, snowTone, iceMask);

    // 6. Optional seasonal snow tint from monthly temperature normal
    if (monthlyTemp && iceMask < 0.98) {
      const monthT = monthlyTemp[id];
      const frost = 1 - smoothstep(-3.0, 0.8, monthT);
      const snowMoisture = smoothstep(60, 420, precip);
      // Forests mask partial snow from orbit, keeping dark evergreen needles peeking through.
      const canopyRetention = 1 - woody * 0.42;
      const seasonalMask = clamp(frost * snowMoisture * canopyRetention * 0.78, 0, 0.82);
      rgb = mixRgb(rgb, COLOR_ALPINE_SNOW, seasonalMask);
    }

    // Subtle deterministic tonal micro-variation (±2.5%) so large flat biomes never look painted
    const tone = 1 + variation * 0.024 * (1 - iceMask * 0.7);
    colors[id * 4] = clamp(rgb[0] * tone, 0, 1);
    colors[id * 4 + 1] = clamp(rgb[1] * tone, 0, 1);
    colors[id * 4 + 2] = clamp(rgb[2] * tone, 0, 1);
    // Pack rock/alpine exposure in alpha channel for fragment-shader micro-detail modulation
    colors[id * 4 + 3] = clamp(rockExposure * 0.7 + relief.ruggedness[id] * 0.3, 0, 1);
  }

  return { colors, waterKind };
}
