import { BufferAttribute, BufferGeometry, DataTexture, DoubleSide, FloatType, FrontSide, Group, LineBasicMaterial,
  LineSegments, Mesh, MOUSE, NearestFilter, OrthographicCamera, PerspectiveCamera, Raycaster, RGBAFormat,
  Points, Scene, ShaderMaterial, Vector2, Vector3, WebGLRenderer } from 'three';
import type { Object3D } from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { LineSegments2 } from 'three/addons/lines/LineSegments2.js';
import { LineSegmentsGeometry } from 'three/addons/lines/LineSegmentsGeometry.js';
import { LineMaterial } from 'three/addons/lines/LineMaterial.js';
import type { World } from '../core/world';
import type { Water } from '../core/water';
import type { WaterFrame } from '../shared/desktop-api';
import { speedCmPerYear } from '../core/tectonics';
import { buildIndexedWaterSurface } from './view-geometry';
import type { CompactDualRing, ViewCorners, ViewGeometry, ViewPair } from './view-geometry';
import { relaxedMask, reliefGradients } from './shading';
import { buildDrainageLines, DRAINAGE_LINE_WIDTHS, NATURAL_PERENNIAL_COLORS, NATURAL_RIVER_WIDTHS,
  NATURAL_SEASONAL_COLORS, PERENNIAL_LINE_COLORS, RIVER_LINE_WIDTHS, SEASONAL_LINE_COLORS } from './rivers';
import { displaceDirections, effectiveExaggeration } from './relief';
import { computeNaturalSurface, computeReliefMetrics } from './natural-color';
import type { ReliefMetrics, SurfaceStyle } from './natural-color';
import { computeCoastField, computeValleyField, meshFrequencyForRegions, REFINEMENT_GLSL, seedOffsetFromString } from './refinement';
import type { CoastField, LodPatchData } from './refinement';
import { summarizeTerrain } from '../core/terrain';
import { buildWaterSurface, pickSurface } from './water-surface';
import { summarizeBasins } from '../core/basins';
import type { TemperatureNormals } from '../core/seasonal-temperature';
import { BIOMES } from '../core/climate-normals';
import type { ClimateNormals } from '../core/climate-normals';
import type { Rivers } from '../core/rivers';
import type { VegetationFrame } from '../core/vegetation';
import type { WildFoodFrame } from '../core/wild-food';
import { isRegionPassable } from '../core/traversal';
import { ISOCHRONE_DAYS, UNREACHABLE_COLOR, travelTimeColor, travelTimeCoordinate } from './travel-scale';
import type { TraversalCosts, TravelField, TravelSeason } from '../core/traversal';
import { buildRouteOverlay, routeOrigin } from './route';
import type { WindNormals } from '../core/seasonal-wind';
import type { SeasonalDisplayFrame } from '../shared/seasonal-display';
import { hasRegionalSurface, isSoilMoistureFrame } from '../shared/seasonal-display';
import { isMoistureLayer, supportsMoistureLayer, moistureLayerColor, moistureLayerValue } from './seasonal-moisture';
import type { MoistureLayer } from './seasonal-moisture';
import { regionalWaterDisplay } from './regional-water';
import type { RegionalWaterDisplay } from './regional-water';
import { PlanetCamera } from './planet-camera';
import type { PlanetView } from './planet-camera';
import { SpaceBackdrop } from './space';

export type Layer = 'surface' | 'signal' | 'area' | 'latitude' | 'plates' | 'boundaries' | 'speed' | 'crust' | 'thickness' | 'elevation' | 'uplift' | 'depth' | 'waterBodies' | 'catchments' | 'contributingArea' | 'basins' | 'spill' | 'temperature' | 'windSpeed' | 'biome' | 'annualPrecipitation' | 'vegetationState' | 'woodyCover' | 'woodyChange' | 'productivity' | 'precipitationAnomaly' | 'foragerCapacity' | 'leanSeason' | 'gameStock' | 'annualRunoff' | 'riverDischarge' | 'travelCost' | 'travelTime' | MoistureLayer;
export type ViewMode = 'flat' | 'globe';
/** Continuous layers can be interpolated between regions; categorical and sentinel layers cannot. */
const SMOOTH_LAYERS = new Set<string>(['surface', 'signal', 'area', 'latitude', 'crust', 'thickness', 'elevation', 'uplift',
  'depth', 'contributingArea', 'temperature', 'windSpeed', 'biome', 'annualPrecipitation', 'vegetationState', 'woodyCover', 'woodyChange', 'productivity', 'precipitationAnomaly', 'foragerCapacity', 'leanSeason', 'gameStock', 'annualRunoff', 'riverDischarge', 'travelCost', 'travelTime']);
const RELIEF_LAYERS: Partial<Record<string, number>> = { surface: 1, biome: 1, vegetationState: 1, woodyCover: 1, woodyChange: 1, productivity: 1, precipitationAnomaly: 1, foragerCapacity: 1, leanSeason: 1, gameStock: 1, annualRunoff: .4, riverDischarge: .45, travelCost: .85, travelTime: .3, elevation: .8 };
/** Land layers colored by a per-region tint over relief, with bathymetric water. */
const TINT_LAYERS = new Set<string>(['biome', 'vegetationState', 'woodyCover', 'woodyChange', 'productivity', 'precipitationAnomaly',
  'foragerCapacity', 'leanSeason', 'gameStock', 'annualRunoff', 'riverDischarge', 'travelCost', 'travelTime']);
/** Layers drawn as land and water with a shoreline, bathymetry and the globe water surface. */
const isMovement = (layer: Layer): boolean => layer === 'travelCost' || layer === 'travelTime';
const surfaceLike = (layer: string): boolean => layer === 'surface' || TINT_LAYERS.has(layer);
type Rgb = readonly [number, number, number];
const RUNOFF_COLORS: readonly Rgb[] = [[169, 100, 62], [214, 164, 102], [215, 202, 139],
  [100, 173, 136], [38, 118, 150], [46, 62, 117]];
const RUNOFF_STOPS = [0, 10, 50, 200, 600, 2_000].map(mm => Math.log1p(mm));

function runoffColor(mm: number): Rgb {
  const depth = Math.min(Math.log1p(mm), RUNOFF_STOPS.at(-1)!);
  const i = Math.min(RUNOFF_STOPS.length - 2, RUNOFF_STOPS.findLastIndex(stop => depth >= stop));
  return mixRgb(RUNOFF_COLORS[i], RUNOFF_COLORS[i + 1], (depth - RUNOFF_STOPS[i]) / (RUNOFF_STOPS[i + 1] - RUNOFF_STOPS[i]));
}
const mixRgb = (a: Rgb, b: Rgb, t: number): Rgb => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
/** Woody cover 0–1 from open ground to closed canopy. */
export function woodyColor(woody: number): Rgb {
  const t = Math.min(1, Math.max(0, woody));
  return t < 0.5 ? mixRgb([214, 204, 160], [132, 160, 88], t / 0.5) : mixRgb([132, 160, 88], [28, 88, 44], (t - 0.5) / 0.5);
}
/** Diverging woody-cover change: a loss or gain of 0.3 or more is fully colored. */
export function changeColor(change: number): Rgb {
  const t = Math.min(1, Math.max(-1, change / 0.3));
  return t < 0 ? mixRgb([238, 234, 220], [168, 64, 36], -t) : mixRgb([238, 234, 220], [30, 122, 64], t);
}
/** Net primary productivity, 0 to 2,500 g dry matter / m² / year or more. */
export function productivityColor(grams: number): Rgb {
  const t = Math.min(1, Math.max(0, grams / 2500));
  return t < 0.25 ? mixRgb([236, 228, 200], [214, 196, 92], t / 0.25)
    : t < 0.55 ? mixRgb([214, 196, 92], [110, 160, 60], (t - 0.25) / 0.3)
    : mixRgb([110, 160, 60], [12, 82, 78], (t - 0.55) / 0.45);
}
/** Forager capacity on a log scale from 0.1 to 100 people per 100 km². */
export function capacityColor(peoplePerSquareKilometer: number): Rgb {
  const t = Math.min(1, Math.max(0, (Math.log10(Math.max(peoplePerSquareKilometer, 1e-9)) + 3) / 3));
  return t < 0.4 ? mixRgb([240, 234, 216], [226, 176, 92], t / 0.4)
    : t < 0.75 ? mixRgb([226, 176, 92], [176, 74, 44], (t - 0.4) / 0.35)
    : mixRgb([176, 74, 44], [82, 28, 46], (t - 0.75) / 0.25);
}
/** Year-round capacity as a share of perfect-storage capacity: pale where seasons do not limit. */
export function leanColor(ratio: number): Rgb {
  return mixRgb([84, 76, 160], [238, 234, 220], Math.min(1, Math.max(0, ratio)));
}
/** Game stock relative to its capacity: depleted red, half amber, full green; gray without game. */
export function gameColor(stock: number, capacity: number): Rgb {
  if (!(capacity > 1e-6)) return [206, 202, 192];
  const t = Math.min(1, Math.max(0, stock / capacity));
  return t < 0.5 ? mixRgb([176, 64, 40], [228, 196, 112], t / 0.5) : mixRgb([228, 196, 112], [52, 118, 64], (t - 0.5) / 0.5);
}
/** Diverging precipitation factor: half of normal or less is fully dry, double or more fully wet. */
export function anomalyColor(factor: number): Rgb {
  const t = Math.min(1, Math.max(-1, Math.log2(factor)));
  return t < 0 ? mixRgb([238, 234, 220], [176, 92, 40], -t) : mixRgb([238, 234, 220], [40, 108, 182], t);
}
/** Logarithmic travel cost in hours per 100 km: 20 (fast flat) to 160+ (very slow). */
export function travelCostColor(hoursPer100Km: number): Rgb {
  if (!(hoursPer100Km > 0)) return UNREACHABLE_COLOR;
  const t = Math.min(1, Math.max(0, Math.log2(hoursPer100Km / 20) / 3));
  return t < 1 / 3 ? mixRgb([232, 224, 178], [216, 168, 84], t * 3)
    : t < 2 / 3 ? mixRgb([216, 168, 84], [168, 76, 56], (t - 1 / 3) * 3)
    : mixRgb([168, 76, 56], [76, 36, 78], (t - 2 / 3) * 3);
}
/** Child order inside each view group. Picking and visibility depend on these indices. */
const RIVERS = 5;
const ROUTE = 6;
const LOD = 7;
const RIVERS_DETAIL = 8;
const ROUTE_RENDER_ORDER = 1 + Math.max(DRAINAGE_LINE_WIDTHS.length, 2 * RIVER_LINE_WIDTHS.length);
// Screen-space strips keep their winding when the globe embedding is reflected.
const RIVER_MATERIAL = { transparent: true, depthWrite: false, side: DoubleSide, forceSinglePass: true };
export type { SurfaceStyle };
export interface NavigationView extends PlanetView { mode: ViewMode; radiusMeters: number }

export class SurfaceMap {
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  private readonly flatCamera = new OrthographicCamera(-1.2, 1.2, .6, -.6, .01, 100);
  private readonly globeCamera = new PerspectiveCamera(42, 1, .001, 200);
  private readonly controls: OrbitControls;
  private readonly planetCamera: PlanetCamera;
  private readonly space: SpaceBackdrop;
  private readonly resizeObserver: ResizeObserver;
  private readonly raycaster = new Raycaster();
  private readonly worldSun = new Vector3(.25, .28, .927).normalize();
  private readonly activeSun = new Vector3(-.4, .6, 1).normalize();
  private distantLighting = true;
  private readonly material = new ShaderMaterial({
    side: DoubleSide,
    uniforms: { field: { value: null }, textureWidth: { value: 1 }, textureHeight: { value: 1 },
      cornerField: { value: null }, cornerWidth: { value: 1 }, cornerHeight: { value: 1 },
      tintField: { value: null }, cornerTintField: { value: null },
      detailField: { value: null }, cornerDetailField: { value: null },
      lodMaskField: { value: null }, lodActive: { value: 0 }, lodPass: { value: 0 },
      lodExaggeration: { value: 0 }, meshFreq: { value: 53.1 }, waterLevelRadius: { value: 1.0 }, viewZoomDetail: { value: 0.0 },
      biomeMode: { value: 0 }, naturalMode: { value: 1 }, detailEnabled: { value: 1 },
      seedOffset: { value: new Vector3() }, precipitationMode: { value: 0 },
      smoothMode: { value: 0 }, reliefStrength: { value: 0 },
      selected: { value: -1 }, globe: { value: 0 }, categorical: { value: 0 }, muted: { value: 0 }, waterMode: { value: 0 },
      surfaceMode: { value: 0 }, waterSurface: { value: 0 }, missingMode: { value: 0 },
      basinMode: { value: 0 }, spillFrom: { value: -1 }, spillTo: { value: -1 }, temperatureMode: { value: 0 }, windMode: { value: 0 },
      travelTimeMode: { value: 0 }, movementMode: { value: 0 },
      isochroneLevels: { value: ISOCHRONE_DAYS.map(travelTimeCoordinate) },
      distantLightMix: { value: 0 }, sunDirection: { value: this.worldSun }, activeSunDirection: { value: this.activeSun } },
    // Field texels hold (value, wet mask, east slope, north slope); detail texels hold
    // (coast phi, strict wet/dry sign, effective relief m, lake factor) for regions and dual corners.
    vertexShader: `attribute float region; attribute vec2 cornerPair; attribute vec3 weight; attribute float radialOffset;
      uniform sampler2D field; uniform float textureWidth; uniform float textureHeight;
      uniform sampler2D cornerField; uniform float cornerWidth; uniform float cornerHeight;
      uniform sampler2D tintField; uniform sampler2D cornerTintField;
      uniform sampler2D detailField; uniform sampler2D cornerDetailField;
      uniform sampler2D lodMaskField; uniform float lodActive; uniform float lodPass; uniform float lodExaggeration;
      uniform float globe; uniform float surfaceMode; uniform float waterSurface; uniform float waterLevelRadius;
      varying float cell; varying vec4 blended; varying vec4 tint; varying vec4 detailVal;
      varying float centerWeight; varying vec3 modelDir; varying vec2 flatCoord;
      varying vec3 direction; varying vec3 planetDirection;
      vec4 texel(sampler2D map, float id, float width, float height) {
        return texture2D(map, vec2((mod(id,width)+0.5)/width,(floor(id/width)+0.5)/height));
      }
      void main() {
        cell=region;
        centerWeight=weight.x;
        if(globe>0.5 && lodActive>0.5 && lodPass<0.5 && texel(lodMaskField,region,textureWidth,textureHeight).r>0.5) {
          gl_Position=vec4(2.0,2.0,2.0,1.0);
          return;
        }
        vec4 b=vec4(0.0), t=vec4(0.0), d=vec4(0.0);
        if(weight.x>0.0) {
          b+=weight.x*texel(field,region,textureWidth,textureHeight);
          t+=weight.x*texel(tintField,region,textureWidth,textureHeight);
          d+=weight.x*texel(detailField,region,textureWidth,textureHeight);
        }
        if(weight.y>0.0) {
          b+=weight.y*texel(cornerField,cornerPair.x,cornerWidth,cornerHeight);
          t+=weight.y*texel(cornerTintField,cornerPair.x,cornerWidth,cornerHeight);
          d+=weight.y*texel(cornerDetailField,cornerPair.x,cornerWidth,cornerHeight);
        }
        if(weight.z>0.0) {
          b+=weight.z*texel(cornerField,cornerPair.y,cornerWidth,cornerHeight);
          t+=weight.z*texel(cornerTintField,cornerPair.y,cornerWidth,cornerHeight);
          d+=weight.z*texel(cornerDetailField,cornerPair.y,cornerWidth,cornerHeight);
        }
        blended=b; tint=t; detailVal=d;
        vec3 pos=position;
        if(lodPass>0.5) {
          pos=position*(1.0+lodExaggeration*radialOffset);
        } else if(globe>0.5 && surfaceMode>0.5 && waterSurface<0.5) {
          float r=length(pos);
          if(r>0.0 && r<waterLevelRadius) pos*=(waterLevelRadius/r);
        }
        flatCoord=position.xy;
        modelDir=globe>0.5 ? (length(position)>0.0 ? normalize(position) : vec3(1.0,0.0,0.0)) : vec3(1.0,0.0,0.0);
        direction=normalMatrix*(length(pos)>0.0 ? normalize(pos) : vec3(0.0,0.0,1.0));
        planetDirection=(modelMatrix*vec4(pos,1.0)).xyz;
        gl_Position=projectionMatrix*modelViewMatrix*vec4(pos,1.0); }`,
    fragmentShader: `uniform sampler2D field; uniform float textureWidth; uniform float textureHeight;
      uniform float smoothMode; uniform float reliefStrength;
      uniform sampler2D tintField; uniform float travelTimeMode; uniform float movementMode;
      uniform float isochroneLevels[${ISOCHRONE_DAYS.length}];
      uniform float selected; uniform float globe; uniform float categorical; uniform float muted; uniform float waterMode;
      uniform float surfaceMode; uniform float waterSurface; uniform float missingMode;
      uniform float basinMode; uniform float spillFrom; uniform float spillTo; uniform float temperatureMode; uniform float windMode;
      uniform float distantLightMix; uniform vec3 sunDirection; uniform vec3 activeSunDirection;
      uniform float biomeMode; uniform float naturalMode; uniform float detailEnabled;
      uniform float meshFreq; uniform float viewZoomDetail;
      uniform vec3 seedOffset; uniform float precipitationMode;
      varying float cell; varying vec4 blended; varying vec4 tint; varying vec4 detailVal;
      varying float centerWeight; varying vec3 modelDir; varying vec2 flatCoord;
      varying vec3 direction; varying vec3 planetDirection;
      ${REFINEMENT_GLSL}
      // Atlas-style hypsometric and bathymetric tints in meters (selectable Relief tint style).
      vec3 hypsometric(float h) {
        vec3 c=vec3(0.33,0.47,0.29);
        c=mix(c,vec3(0.48,0.58,0.34),smoothstep(0.0,300.0,h));
        c=mix(c,vec3(0.68,0.67,0.45),smoothstep(300.0,900.0,h));
        c=mix(c,vec3(0.63,0.51,0.36),smoothstep(900.0,1800.0,h));
        c=mix(c,vec3(0.53,0.45,0.39),smoothstep(1800.0,3000.0,h));
        return mix(c,vec3(0.88,0.88,0.88),smoothstep(3200.0,5200.0,h));
      }
      vec3 bathymetric(float d) {
        vec3 c=vec3(0.24,0.48,0.63);
        c=mix(c,vec3(0.14,0.36,0.55),smoothstep(0.0,250.0,d));
        c=mix(c,vec3(0.08,0.25,0.46),smoothstep(250.0,1500.0,d));
        c=mix(c,vec3(0.04,0.14,0.33),smoothstep(1500.0,4500.0,d));
        return mix(c,vec3(0.02,0.08,0.21),smoothstep(4500.0,8000.0,d));
      }
      void main() {
        float id=floor(cell+0.5);
        vec2 uv=vec2((mod(id,textureWidth)+0.5)/textureWidth,(floor(id/textureWidth)+0.5)/textureHeight);
        vec4 own=texture2D(field,uv);
        vec4 sampled=smoothMode>0.5 ? blended : own;
        float v=sampled.r, m=sampled.g;
        bool wet=false;
        float landMix=1.0;
        vec3 mDir;
        if(globe>0.5) {
          mDir=normalize(modelDir);
        } else {
          float lon=flatCoord.x*3.14159265359;
          float lat=clamp(flatCoord.y*3.14159265359,-1.5707963,1.5707963);
          float cl=cos(lat);
          mDir=vec3(cl*cos(lon),sin(lat),cl*sin(lon));
        }
        float pixelArc=length(fwidth(mDir));
        float fineWeight=smoothstep(0.0045,0.00045,pixelArc);
        float localRelief=max(0.0,detailVal.b);
        float rugged=smoothstep(95.0,1150.0,localRelief);
        float valley=travelTimeMode>0.5 ? 0.0 : clamp(tint.a,0.0,1.0);
        bool useDetail=smoothMode>0.5 && detailEnabled>0.5;
        float phiDetail=detailVal.r;

        vec3 color=mix(vec3(0.16,0.28,0.33),vec3(0.73,0.80,0.63),clamp(v,0.0,1.0));
        if(categorical>0.5) color=0.51+0.23*cos(6.2831853*(v*0.618033989+vec3(0.0,0.33,0.67)));
        if(waterMode>0.5 && waterMode<1.5) color=m>0.0 ? vec3(0.34,0.36,0.30) : mix(vec3(0.18,0.62,0.75),vec3(0.025,0.10,0.26),clamp(v,0.0,1.0));
        if(waterMode>1.5 && v<0.5) color=vec3(0.34,0.36,0.30);
        if(missingMode>0.5 && v<0.0) color=vec3(0.34,0.36,0.30);

        float detailSignal=0.0;
        vec2 detailGrad=vec2(0.0);
        if(surfaceMode>0.5) {
          bool frozenCap=movementMode>0.5 && waterSurface>0.5 && own.g>0.0;
          if(smoothMode>0.5 && movementMode<0.5) {
            // Zero contour of symmetric relaxed coastline field, perturbed strictly in transition zones.
            float coastBand=smoothstep(0.01,0.28,1.0-abs(detailVal.g));
            float centerGuard=1.0-smoothstep(0.68,0.93,centerWeight);
            float coastMask=coastBand*centerGuard;
            if(useDetail && coastMask>0.001) {
              float coastAmp=(0.38+0.16*rugged)*coastMask;
              phiDetail+=coastAmp*sampleCoastNoise(mDir,seedOffset,meshFreq,pixelArc);
            }
            if(waterSurface>0.5 && !frozenCap && phiDetail>0.0) discard;
            float fw=max(fwidth(phiDetail),0.0010);
            landMix=waterSurface>0.5 ? 0.0 : smoothstep(-fw*0.85,fw*0.85,phiDetail);
            wet=!frozenCap && (waterSurface>0.5 || phiDetail<0.0);
          } else {
            if(waterSurface>0.5 && !frozenCap && (smoothMode>0.5 || movementMode>0.5) && m>0.0) discard;
            wet=!frozenCap && (waterSurface>0.5 || m<0.0);
            landMix=wet ? 0.0 : 1.0;
          }

          float rawDepth=max(-v,0.0);
          float shoreDepthFade=smoothstep(0.0,0.36,max(-phiDetail,0.0));
          float waterDepth=(smoothMode>0.5 && movementMode<0.5)
            ? mix(14.0*max(-phiDetail,0.0)/0.36,max(rawDepth,120.0*max(-phiDetail,0.0)),shoreDepthFade)
            : rawDepth;
          vec3 waterColor=naturalMode>0.5 ? naturalWater(waterDepth,detailVal.a) : bathymetric(waterDepth);
          vec3 landColor=biomeMode>0.5 ? tint.rgb : hypsometric(max(v,0.0));

          // Single-pass band-limited analytical fractal micro-relief normals & close-up satellite surface detail.
          if(useDetail && landMix>0.01 && reliefStrength>0.0) {
            float coastTaper=smoothstep(0.0,0.28,max(0.0,phiDetail));
            if(coastTaper>0.004) {
              float mh=length(mDir.xz);
              vec3 mEast=mh<1e-6 ? vec3(1.0,0.0,0.0) : vec3(-mDir.z/mh,0.0,mDir.x/mh);
              vec3 mNorth=cross(mEast,mDir);
              vec4 dg=sampleDetailGrad(mDir,seedOffset,rugged,valley,meshFreq,pixelArc);
              detailSignal=dg.x;
              // Scale micro-relief sharply on mountain divides, softly on hills, calm on near-flat plains & valley floors.
              float valleyDamp=1.0-0.58*valley;
              float reliefCurve=pow(rugged,0.90);
              float zoomBoost=1.0+0.50*fineWeight;
              float ampSlope=(0.00085*(1.0+0.55*fineWeight)+0.0195*reliefCurve*zoomBoost)*valleyDamp*coastTaper;
              detailGrad=vec2(dot(dg.yzw,mEast),dot(dg.yzw,mNorth))*ampSlope;
              if(naturalMode>0.5 && biomeMode>0.5) {
                float microSlope=length(detailGrad);
                float highRock=smoothstep(1650.0,3600.0,v);
                float rockiness=clamp(max(highRock*0.85,rugged*0.75),0.0,1.0);
                // Steep alpine micro-slopes expose bare granite/slate; high cold ridges catch snow.
                float screeMix=smoothstep(0.28,0.98,microSlope)*smoothstep(0.14,0.60,rugged)*(0.22+0.48*rockiness);
                vec3 rockTint=mix(vec3(0.49,0.46,0.42),vec3(0.45,0.45,0.46),highRock);
                landColor=mix(landColor,rockTint,screeMix);
                float coldPeak=smoothstep(0.32,0.82,rockiness)*smoothstep(0.08,0.48,detailSignal)*smoothstep(1900.0,3500.0,v+detailSignal*450.0);
                landColor=mix(landColor,vec3(0.90,0.92,0.94),coldPeak*0.52);
                // Subtle littoral beach/cliff nuance right along the smoothed shoreline.
                float littoral=(1.0-smoothstep(0.0,0.14,phiDetail))*fineWeight;
                vec3 shoreTint=mix(vec3(0.62,0.58,0.46),vec3(0.39,0.38,0.36),smoothstep(0.12,0.52,rugged));
                landColor=mix(landColor,shoreTint,littoral*0.24*(1.0-smoothstep(0.75,0.90,landColor.r)));
                // Subtle riparian greening along incised drainage valleys in warmer/drier terrain.
                float dryLand=smoothstep(0.04,0.22,landColor.r-landColor.g);
                landColor=mix(landColor,vec3(0.24,0.34,0.20),valley*dryLand*0.20*coastTaper);
                // Multi-scale satellite surface mosaic breaks up smooth interpolation at close range.
                float texStrength=(0.045+0.048*fineWeight+0.032*rugged)*(1.0-smoothstep(0.80,0.92,landColor.r));
                landColor*=(1.0+detailSignal*texStrength);
              }
            }
          }
          color=mix(waterColor,landColor,landMix);

          if(travelTimeMode>0.5) {
            // Isochrones follow interpolated log-time. The owning cell decides
            // reachability, so contour lines never continue onto another landmass.
            float width=clamp(fwidth(tint.a),0.00001,0.09);
            float distance=100.0;
            for(int i=0;i<${ISOCHRONE_DAYS.length};i++) distance=min(distance,abs(tint.a-isochroneLevels[i]));
            float line=1.0-smoothstep(0.15*width,1.35*width,distance);
            if(!wet) {
              if(texture2D(tintField,uv).a<0.0 && own.g>0.0) color=vec3(${UNREACHABLE_COLOR.map(c => c / 255).join(',')});
              else if(tint.a>=0.0) {
                float lightness=dot(color,vec3(0.21,0.72,0.07));
                vec3 ink=mix(vec3(0.99,0.91,0.76),vec3(0.28,0.16,0.12),smoothstep(0.57,0.68,lightness));
                color=mix(color,ink,0.72*line);
              }
            }
          }
        }

        // Softly compress relief gradients so alpine ridges sculpt crisply without black walls.
        vec2 rawGrad=blended.ba+detailGrad;
        float gMag=length(rawGrad);
        vec2 gSoft=rawGrad/(1.0+0.34*gMag);
        float reliefWeight=reliefStrength*mix(0.24,1.0,landMix);
        float creviceAO=1.0-0.13*clamp(-detailSignal,0.0,1.0)*(0.25+0.75*rugged)*reliefWeight;

        if(globe<0.5 && reliefWeight>0.0) {
          vec3 n=normalize(vec3(-gSoft.x,-gSoft.y,1.0));
          vec3 light=normalize(vec3(-0.60,0.64,0.48));
          float delta=dot(n,light)-light.z;
          vec3 shadeTint=delta>=0.0
            ? mix(vec3(1.0),vec3(1.28,1.24,1.15),clamp(delta*1.75,0.0,1.0))
            : mix(vec3(1.0),vec3(0.45,0.52,0.64),clamp(-delta*1.55,0.0,1.0));
          color*=mix(vec3(1.0),shadeTint,reliefWeight)*creviceAO;
        }

        if(temperatureMode>0.5) {
          vec3 cold=vec3(0.10,0.20,0.48), cool=vec3(0.29,0.68,0.82);
          vec3 mild=vec3(0.89,0.88,0.64), hot=vec3(0.83,0.24,0.13);
          color=v<0.33 ? mix(cold,cool,clamp(v/0.33,0.0,1.0))
            : v<0.66 ? mix(cool,mild,clamp((v-0.33)/0.33,0.0,1.0))
            : mix(mild,hot,clamp((v-0.66)/0.34,0.0,1.0));
        }
        if(precipitationMode>0.5) {
          vec3 arid=vec3(0.58,0.44,0.26), steppe=vec3(0.82,0.78,0.48), humid=vec3(0.32,0.62,0.36), soaked=vec3(0.13,0.36,0.62);
          color=v<0.4 ? mix(arid,steppe,clamp(v/0.4,0.0,1.0))
            : v<0.7 ? mix(steppe,humid,clamp((v-0.4)/0.3,0.0,1.0))
            : mix(humid,soaked,clamp((v-0.7)/0.3,0.0,1.0));
        }
        if(windMode>0.5) {
          color=mix(vec3(0.10,0.18,0.23),vec3(0.30,0.77,0.83),clamp(v/0.6,0.0,1.0));
          if(v>0.6) color=mix(vec3(0.30,0.77,0.83),vec3(0.92,0.90,0.54),clamp((v-0.6)/0.4,0.0,1.0));
        }
        color*=1.0-muted*0.65;
        if(basinMode>0.5 && (abs(cell-spillFrom)<0.25 || abs(cell-spillTo)<0.25)) color=vec3(0.15,0.95,0.94);
        if(abs(cell-selected)<0.25) color=vec3(0.96,0.78,0.42);
        if(globe>0.5) {
          vec3 worldUp=normalize(planetDirection);
          float wh=length(worldUp.xz);
          vec3 wEast=wh<1e-6 ? vec3(1.0,0.0,0.0) : vec3(worldUp.z/wh,0.0,-worldUp.x/wh);
          vec3 wNorth=cross(worldUp,wEast);
          vec3 worldNormal=reliefWeight>0.0
            ? normalize(worldUp-wEast*(gSoft.x*reliefWeight)-wNorth*(gSoft.y*reliefWeight))
            : worldUp;
          float macroDot=dot(worldUp,activeSunDirection);
          float normalDot=dot(worldNormal,activeSunDirection);
          float hemi=0.54+0.46*smoothstep(-0.24,0.88,macroDot);
          float slopeDelta=normalDot-macroDot;
          vec3 reliefTint=slopeDelta>=0.0
            ? mix(vec3(1.0),vec3(1.34,1.29,1.19),clamp(slopeDelta*1.85,0.0,1.0))
            : mix(vec3(1.0),vec3(0.44,0.52,0.65),clamp(-slopeDelta*1.65,0.0,1.0));
          vec3 survey=color*hemi*reliefTint*creviceAO;
          // Subtle grazing-angle aerial perspective haze on tilted close-up views for atmospheric depth.
          if(surfaceMode>0.5 && viewZoomDetail>0.01) {
            vec3 viewDir=normalize(cameraPosition-planetDirection);
            float grazing=pow(1.0-clamp(dot(worldUp,viewDir),0.0,1.0),2.4);
            float haze=grazing*viewZoomDetail*0.28*(1.0-distantLightMix);
            survey=mix(survey,vec3(0.48,0.62,0.80),haze);
          }

          float solar=dot(worldUp,sunDirection);
          float solarSlope=dot(worldNormal,sunDirection);
          float day=smoothstep(-0.04,0.075,solar);
          float litSlope=clamp(0.18+0.88*max(0.0,mix(solar,solarSlope,0.88)),0.0,1.16);
          vec3 daylight=mix(vec3(1.0,0.76,0.54),vec3(1.0,0.99,0.97),smoothstep(0.0,0.26,solar))*litSlope;
          vec3 orbital=color*mix(vec3(0.014,0.020,0.034),daylight,day);
          color=mix(survey,orbital,distantLightMix);
        }
        gl_FragColor=vec4(color,1.0);
      }`,
  });
  private readonly waterMaterial = this.material.clone();
  private readonly lodMaterial = this.material.clone();
  private readonly lineMaterial = new LineBasicMaterial({ color: '#304849', transparent: true, opacity: .45 });
  private readonly tectonicMaterial = new LineBasicMaterial({ vertexColors: true });
  private views: Record<ViewMode, Group> | null = null;
  private texture: DataTexture | null = null;
  private cornerTexture: DataTexture | null = null;
  private detailTexture: DataTexture | null = null;
  private cornerDetailTexture: DataTexture | null = null;
  private lodMaskTexture: DataTexture | null = null;
  private values = new Float32Array(0);
  private cornerValues = new Float32Array(0);
  private detailValues = new Float32Array(0);
  private cornerDetailValues = new Float32Array(0);
  private lodMaskValues = new Float32Array(0);
  private corners: ViewCorners | null = null;
  private compactRing: CompactDualRing | null = null;
  private reliefMetrics: ReliefMetrics | null = null;
  private coastField: CoastField | null = null;
  private valleyFactor: Float32Array | null = null;
  private linesExaggerationDirty = false;
  private smoothShading = true;
  private reliefShading = true;
  private drainageLines = true;
  private surfaceStyle: SurfaceStyle = 'natural';
  private seasonalSnow = false;
  private displayDetail = true;
  private adaptiveExaggeration = true;
  private lodRequestId = 0;
  private lodTimer = 0;
  private lastLodFocus: [number, number, number] = [0, 0, 0];
  private lastLodDistance = Infinity;
  private lastLodTilt = 0;
  private lastLodSubdivision = 1;
  private lodTriangles = 0;
  private baseGlobeTriangles = 0;
  private baseFlatTriangles = 0;
  private readonly riverMaterials = [
    ...DRAINAGE_LINE_WIDTHS.map(linewidth => new LineMaterial({ ...RIVER_MATERIAL, color: 0x3b78a4, linewidth })),
    ...RIVER_LINE_WIDTHS.map((linewidth, i) => new LineMaterial({ ...RIVER_MATERIAL, color: PERENNIAL_LINE_COLORS[i], linewidth })),
    ...RIVER_LINE_WIDTHS.map((width, i) => new LineMaterial({ ...RIVER_MATERIAL, color: SEASONAL_LINE_COLORS[i], linewidth: width * .68 })),
  ];
  private readonly riverDetailMaterials = [
    ...DRAINAGE_LINE_WIDTHS.map(linewidth => new LineMaterial({ ...RIVER_MATERIAL, color: 0x3b78a4, linewidth: linewidth * 0.88 })),
    ...RIVER_LINE_WIDTHS.map((linewidth, i) => new LineMaterial({ ...RIVER_MATERIAL, color: PERENNIAL_LINE_COLORS[i], linewidth: linewidth * 0.88 })),
    ...RIVER_LINE_WIDTHS.map((width, i) => new LineMaterial({ ...RIVER_MATERIAL, color: SEASONAL_LINE_COLORS[i], linewidth: width * .62 })),
  ];
  private rivers: Rivers | null = null;
  private readonly routeCasingMaterial = new LineMaterial({ color: 0x0b1220, linewidth: 5.5, transparent: true, opacity: 0.85, side: DoubleSide, depthWrite: false });
  private readonly routeLineMaterial = new LineMaterial({ color: 0xf8fafc, linewidth: 3, transparent: true, opacity: 0.98, side: DoubleSide, depthWrite: false });
  private world: World | null = null;
  private waterFrame: WaterFrame | null = null;
  private regionalWater: RegionalWaterDisplay | null = null;
  private globeWater: Water | WaterFrame | RegionalWaterDisplay | null = null;
  private temperatureNormals: TemperatureNormals | null = null;
  private climateNormals: ClimateNormals | null = null;
  private vegetation: VegetationFrame | null = null;
  private wildFood: WildFoodFrame | null = null;
  private traversalCosts: TraversalCosts | null = null;
  private travelField: TravelField | null = null;
  private travelSeason: TravelSeason = 'summer';
  private readonly originMaterial = new ShaderMaterial({
    uniforms: { size: { value: 14 } },
    vertexShader: `uniform float size; void main() {
      gl_PointSize=size; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0);
    }`,
    fragmentShader: `void main() {
      float r=length(gl_PointCoord-0.5);
      if(r>0.49) discard;
      gl_FragColor=r>0.37 ? vec4(0.04,0.06,0.09,1.0) :
        r>0.24 ? vec4(1.0,0.98,0.88,1.0) : vec4(0.08,0.11,0.16,1.0);
    }`,
    transparent: true, depthWrite: false,
  });
  private tintTexture: DataTexture | null = null;
  private cornerTintTexture: DataTexture | null = null;
  private tintValues = new Float32Array(0);
  private cornerTintValues = new Float32Array(0);
  private windNormals: WindNormals | null = null;
  private moistureFrame: SeasonalDisplayFrame | null = null;
  private temperatureMonth = 3;
  private temperatureRange = { minimum: -40, maximum: 40 };
  private windMaximum = 10;
  private mode: ViewMode = 'flat';
  private layer: Layer = 'surface';
  private exaggeration = 3.5;
  private appliedExaggeration = NaN;
  private terrainRange = { minimumMeters: 0, maximumMeters: 1 };
  private maximumDepth = 1;
  private maximumContributingArea = 1;
  private spillRange = { minimum: 0, maximum: 0 };
  private boundaries = false;
  private worker: Worker | null = null;
  private abortPreparation: (() => void) | null = null;
  private frame = 0;
  private dampingFrame = 0;
  private dampingChanged = false;
  private lost = false;
  private pointerStart = [0, 0];
  private pointerMoved = false;
  private dragPointer: number | null = null;
  private readonly handleControlsChange = (): void => {
    this.draw(); this.notifyNavigation();
    if (this.dampingFrame) this.dampingChanged = true;
    else if (this.dragPointer !== null) this.scheduleDamping();
  };
  private readonly updateDamping = (): void => {
    this.dampingFrame = 0;
    this.dampingChanged = false;
    this.controls.update();
    if (this.dragPointer !== null || this.dampingChanged) this.scheduleDamping();
  };
  private scheduleDamping(): void {
    if (!this.dampingFrame) this.dampingFrame = requestAnimationFrame(this.updateDamping);
  }
  private stopDamping(): void {
    if (this.dampingFrame) cancelAnimationFrame(this.dampingFrame);
    this.dampingFrame = 0;
    this.dampingChanged = false;
  }
  private readonly beginMapDrag = (event: PointerEvent): void => {
    if (this.mode !== 'flat' || event.button !== 0 || this.dragPointer !== null) return;
    this.stopDamping();
    this.dragPointer = event.pointerId;
    this.canvas.classList.add('dragging');
    document.body.classList.add('map-dragging');
  };
  private readonly endMapDrag = (event: PointerEvent): void => {
    if (this.dragPointer !== event.pointerId) return;
    const moved = Math.hypot(event.clientX - this.pointerStart[0], event.clientY - this.pointerStart[1]) > 4;
    this.clearMapDrag();
    if (event.type === 'pointerup' && moved) this.scheduleDamping();
  };
  private readonly clearMapDrag = (): void => {
    this.dragPointer = null;
    this.canvas.classList.remove('dragging');
    document.body.classList.remove('map-dragging');
  };

  constructor(private readonly canvas: HTMLCanvasElement, private readonly select: (id: number) => void) {
    // Share field/selection uniforms, distinguishing raised water and refined LOD passes.
    this.waterMaterial.uniforms = { ...this.material.uniforms, waterSurface: { value: 1 }, lodPass: { value: 0 } };
    this.waterMaterial.polygonOffset = true;
    this.waterMaterial.polygonOffsetFactor = -1; this.waterMaterial.polygonOffsetUnits = -1;
    this.lodMaterial.uniforms = { ...this.material.uniforms, waterSurface: { value: 0 }, lodPass: { value: 1 } };
    this.renderer = new WebGLRenderer({ canvas, antialias: true, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
    this.renderer.setClearColor('#0c1721');
    this.flatCamera.position.set(0, 0, 3);
    this.globeCamera.position.set(3, 0, 0);
    this.controls = new OrbitControls(this.flatCamera, canvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = .08;
    this.controls.enableRotate = false;
    this.controls.screenSpacePanning = true;
    this.controls.zoomToCursor = true;
    this.controls.minZoom = .5; this.controls.maxZoom = 30;
    this.controls.minDistance = 1.3; this.controls.maxDistance = 8;
    this.controls.addEventListener('change', this.handleControlsChange);
    canvas.dataset.sky = 'loading';
    canvas.dataset.surfaceStyle = this.surfaceStyle;
    canvas.dataset.displayDetail = String(this.displayDetail);
    canvas.dataset.seasonalSnow = String(this.seasonalSnow);
    this.space = new SpaceBackdrop(this.renderer.getPixelRatio(), ready => {
      canvas.dataset.sky = ready ? 'ready' : 'fallback'; this.notifyNavigation(); this.draw();
    });
    this.scene.add(this.space);
    this.planetCamera = new PlanetCamera(this.globeCamera, canvas, () => {
      this.syncAdaptiveExaggeration();
      this.scheduleLodUpdate();
      this.draw();
      this.notifyNavigation();
    });
    canvas.addEventListener('pointerdown', (e) => {
      if (!e.isPrimary) return;
      this.pointerStart = [e.clientX, e.clientY]; this.pointerMoved = false;
      canvas.focus({ preventScroll: true });
    });
    canvas.addEventListener('pointermove', e => {
      if (e.isPrimary && Math.hypot(e.clientX - this.pointerStart[0], e.clientY - this.pointerStart[1]) > 4) this.pointerMoved = true;
    });
    canvas.addEventListener('pointerdown', this.beginMapDrag);
    window.addEventListener('pointerup', this.endMapDrag);
    window.addEventListener('pointercancel', this.endMapDrag);
    window.addEventListener('blur', this.clearMapDrag);
    canvas.addEventListener('pointerup', (e) => {
      if (!e.isPrimary || this.pointerMoved || this.mode === 'globe' && this.planetCamera.wasDragged || e.button !== 0 || !this.views) return;
      const rect = canvas.getBoundingClientRect();
      this.raycaster.setFromCamera(new Vector2((e.clientX - rect.left) / rect.width * 2 - 1, 1 - (e.clientY - rect.top) / rect.height * 2), this.camera);
      const view = this.views[this.mode];
      const hit = pickSurface(this.raycaster, view.children[0] as Mesh<BufferGeometry>, view.children[3] as Mesh<BufferGeometry>);
      if (!hit) return;
      const id = hit.id; this.canvas.dataset.pickedSurface = hit.surface;
      this.material.uniforms.selected.value = id; this.select(id); this.draw();
    });
    canvas.addEventListener('dblclick', event => {
      if (this.mode !== 'flat') return;
      event.preventDefault(); this.controls.zoomToCursor = true;
      // OrbitControls owns wheel anchoring. Center the clicked map point for a deliberate approach.
      const bounds = canvas.getBoundingClientRect();
      const point = new Vector2((event.clientX - bounds.left) / bounds.width * 2 - 1, 1 - (event.clientY - bounds.top) / bounds.height * 2);
      this.raycaster.setFromCamera(point, this.flatCamera);
      const distance = -this.raycaster.ray.origin.z / this.raycaster.ray.direction.z;
      const hit = this.raycaster.ray.at(distance, this.controls.target.clone());
      const shift = hit.sub(this.controls.target).multiplyScalar(.5);
      this.controls.target.add(shift); this.flatCamera.position.add(shift); this.zoomBy(2);
    });
    canvas.addEventListener('keydown', event => {
      if (this.mode !== 'flat' || event.ctrlKey || event.metaKey) return;
      const key = event.key.toLowerCase();
      if (key === 'r' || key === 'home') this.reset();
      else if (key === 'n' || key === 'u') { /* The flat map is north up. */ }
      else if (key === '+' || key === '=' || key === 'pageup') this.zoomBy(1.5);
      else if (key === '-' || key === 'pagedown') this.zoomBy(1 / 1.5);
      else if (key === ' ') this.stopCamera();
      else if (key.startsWith('arrow')) this.controls.pan(key === 'arrowleft' ? 32 : key === 'arrowright' ? -32 : 0, key === 'arrowup' ? 32 : key === 'arrowdown' ? -32 : 0);
      else return;
      event.preventDefault(); this.scheduleDamping();
    });
    canvas.addEventListener('webglcontextlost', (event) => { event.preventDefault(); this.lost = true; canvas.dataset.gpu = 'lost'; });
    canvas.addEventListener('webglcontextrestored', () => { this.lost = false; canvas.dataset.gpu = 'ready'; this.draw(); });
    canvas.dataset.gpu = 'ready';
    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(canvas.parentElement!);
    this.resize(); this.setMode('flat');
  }
  private get camera(): OrthographicCamera | PerspectiveCamera { return this.mode === 'flat' ? this.flatCamera : this.globeCamera; }
  private resize(): void {
    const width = this.canvas.clientWidth, height = this.canvas.clientHeight;
    if (!width || !height) return;
    this.renderer.setSize(width, height, false);
    for (const material of this.riverMaterials) material.resolution.set(width, height);
    for (const material of this.riverDetailMaterials) material.resolution.set(width, height);
    this.routeCasingMaterial.resolution.set(width, height);
    this.routeLineMaterial.resolution.set(width, height);
    this.originMaterial.uniforms.size.value = 14 * this.renderer.getPixelRatio();
    const aspect = width / height, halfHeight = Math.max(.6, 1.1 / aspect);
    Object.assign(this.flatCamera, { left: -halfHeight * aspect, right: halfHeight * aspect, top: halfHeight, bottom: -halfHeight });
    this.flatCamera.updateProjectionMatrix();
    this.globeCamera.aspect = aspect; this.globeCamera.updateProjectionMatrix(); this.planetCamera.resize(); this.draw();
  }
  private makeView(data: ViewGeometry): Group {
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new BufferAttribute(data.positions, 3));
    geometry.setAttribute('region', new BufferAttribute(data.regions, 1));
    geometry.setAttribute('cornerPair', new BufferAttribute(data.cornerPairs, 2));
    geometry.setAttribute('weight', new BufferAttribute(data.weights, 3));
    if (data.indices) geometry.setIndex(new BufferAttribute(data.indices, 1));
    if (data.landCoastIndexCount !== undefined) geometry.userData.landCoastIndexCount = data.landCoastIndexCount;
    const lines = new BufferGeometry(); lines.setAttribute('position', new BufferAttribute(data.lines, 3));
    const tectonicLines = new BufferGeometry();
    tectonicLines.setAttribute('position', new BufferAttribute(data.tectonicLines, 3));
    tectonicLines.setAttribute('color', new BufferAttribute(data.tectonicColors, 3));
    const water = new BufferGeometry();
    water.setAttribute('position', new BufferAttribute(data.waterPositions, 3));
    water.setAttribute('region', new BufferAttribute(data.waterRegions, 1));
    water.setAttribute('cornerPair', new BufferAttribute(data.waterCornerPairs, 2));
    water.setAttribute('weight', new BufferAttribute(data.waterWeights, 3));
    if (data.waterIndices) water.setIndex(new BufferAttribute(data.waterIndices, 1));
    const waterLines = new BufferGeometry(); waterLines.setAttribute('position', new BufferAttribute(data.waterLines, 3));
    for (const [g, offsets] of [[geometry, data.radialOffsets], [lines, data.lineOffsets], [tectonicLines, data.tectonicOffsets],
      [water, data.waterOffsets], [waterLines, data.waterLineOffsets]] as const) {
      if (offsets.length) { g.userData.base = (g.getAttribute('position').array as Float32Array).slice(); g.userData.offsets = offsets; }
    }
    const lodGeometry = new BufferGeometry();
    lodGeometry.setAttribute('position', new BufferAttribute(new Float32Array(0), 3));
    lodGeometry.setAttribute('region', new BufferAttribute(new Float32Array(0), 1));
    lodGeometry.setAttribute('cornerPair', new BufferAttribute(new Float32Array(0), 2));
    lodGeometry.setAttribute('weight', new BufferAttribute(new Float32Array(0), 3));
    lodGeometry.setAttribute('radialOffset', new BufferAttribute(new Float32Array(0), 1));
    const lodMesh = new Mesh(lodGeometry, this.lodMaterial);
    lodMesh.visible = false;
    lodMesh.frustumCulled = false;
    const group = new Group();
    group.add(new Mesh(geometry, this.material), new LineSegments(lines, this.lineMaterial), new LineSegments(tectonicLines, this.tectonicMaterial),
      new Mesh(water, this.waterMaterial), new LineSegments(waterLines, this.lineMaterial), new Group(), new Group(), lodMesh, new Group());
    return group;
  }
  cancelPreparation(): void {
    clearTimeout(this.lodTimer); this.lodTimer = 0; this.lodRequestId++;
    this.worker?.terminate(); this.worker = null; this.abortPreparation?.(); this.abortPreparation = null;
  }
  async setWorld(world: World): Promise<void> {
    this.cancelPreparation();
    const worker = new Worker(new URL('./geometry.worker.ts', import.meta.url), { type: 'module' });
    this.worker = worker;
    const pair = await new Promise<ViewPair>((resolve, reject) => {
      this.abortPreparation = () => reject(new Error('View preparation canceled.'));
      worker.onerror = (e) => reject(new Error(e.message));
      worker.onmessage = (e: MessageEvent<{ type?: string; pair?: ViewPair; error?: string }>) => {
        if (e.data.error) reject(new Error(e.data.error));
        else if (e.data.pair) resolve(e.data.pair);
      };
      worker.postMessage({ type: 'build', surface: world.surface, tectonics: world.tectonics, elevation: world.terrain.elevation, water: world.water });
    }).catch((error) => {
      worker.terminate();
      if (this.worker === worker) { this.worker = null; this.abortPreparation = null; }
      throw error;
    });
    if (this.worker === worker) this.abortPreparation = null;
    worker.onmessage = (e: MessageEvent<{ type?: string; requestId?: number; patch?: LodPatchData | null; error?: string }>) => {
      if (this.worker !== worker || e.data.type !== 'lod') return;
      if (e.data.requestId !== this.lodRequestId) return;
      this.applyLodPatch(e.data.patch ?? null);
    };
    const next = { flat: this.makeView(pair.flat), globe: this.makeView(pair.globe) };
    const flatTriCount = pair.flat.indices ? pair.flat.indices.length / 3 : pair.flat.regions.length / 3;
    const globeBedTriCount = pair.globe.indices ? pair.globe.indices.length / 3 : pair.globe.regions.length / 3;
    const globeWaterTriCount = pair.globe.waterIndices ? pair.globe.waterIndices.length / 3 : pair.globe.waterRegions.length / 3;
    this.baseFlatTriangles = flatTriCount;
    this.baseGlobeTriangles = globeBedTriCount + globeWaterTriCount;
    this.canvas.dataset.flatTriangles = String(this.baseFlatTriangles);
    this.canvas.dataset.globeBaseTriangles = String(this.baseGlobeTriangles);
    this.canvas.dataset.lodTriangles = '0';
    this.canvas.dataset.lodLevel = '1';
    this.lodTriangles = 0;
    this.lastLodDistance = Infinity;
    this.lastLodTilt = 0;
    this.lastLodSubdivision = 1;
    this.reliefMetrics = computeReliefMetrics(world.surface, world.terrain.elevation, world.water.bodyIds, world.water.levelMeters);
    this.coastField = computeCoastField(world.surface, world.terrain.elevation, world.water.bodyIds, world.water.depthMeters,
      world.water.levelMeters, world.water.bodyIds, world.water.mainOceanId);
    this.valleyFactor = computeValleyField(world.surface, world.drainage, world.water.bodyIds, null);
    const seedVec = seedOffsetFromString(world.recipe.seed);
    this.material.uniforms.seedOffset.value.set(seedVec[0], seedVec[1], seedVec[2]);
    this.material.uniforms.meshFreq.value = meshFrequencyForRegions(world.stats.regionCount);
    this.rivers = null; this.addDrainageLines(next, world);
    // Model longitude increases toward +Z. Reflect the display embedding so east
    // appears on the right in a north-up globe, matching the flat projection.
    next.globe.scale.z = -1;
    this.releaseViews(); this.views = next; this.compactRing = pair.compactRing ?? null; this.linesExaggerationDirty = false;
    this.scene.add(next.flat, next.globe); this.world = world; this.waterFrame = null; this.temperatureNormals = null; this.windNormals = null; this.climateNormals = null; this.vegetation = null; this.wildFood = null;
    this.traversalCosts = null; this.travelField = null;
    this.moistureFrame = null; this.regionalWater = null; this.globeWater = world.water;
    delete this.canvas.dataset.regionalSurface;
    delete this.canvas.dataset.moistureSeconds;
    delete this.canvas.dataset.seasonalModel;
    delete this.canvas.dataset.travelOrigin;
    delete this.canvas.dataset.travelRouteHops;
    this.canvas.dataset.travelSeason = this.travelSeason;
    this.appliedExaggeration = NaN;
    this.terrainRange = summarizeTerrain(world.surface, world.terrain);
    this.maximumDepth = world.water.depthMeters.reduce((max, d) => Math.max(max, d), 0);
    this.maximumContributingArea = world.drainage.contributingArea.reduce((max, a) => Math.max(max, a), 0);
    const basinStats = summarizeBasins(world.basins);
    this.spillRange = { minimum: basinStats.minimumSpillMeters ?? 0, maximum: basinStats.maximumSpillMeters ?? 0 };
    this.setExaggeration(this.adaptiveExaggeration ? 'auto' : this.exaggeration);
    const width = Math.min(1024, this.renderer.capabilities.maxTextureSize);
    const fieldTexture = (data: Float32Array<ArrayBuffer>): DataTexture => {
      const texture = new DataTexture(data, width, data.length / 4 / width, RGBAFormat, FloatType);
      texture.minFilter = texture.magFilter = NearestFilter;
      return texture;
    };
    const texels = (count: number): Float32Array<ArrayBuffer> => new Float32Array(width * Math.max(1, Math.ceil(count / width)) * 4);
    this.corners = pair.corners;
    this.values = texels(world.stats.regionCount); this.cornerValues = texels(pair.corners.offsets.length - 1);
    this.texture = fieldTexture(this.values); this.cornerTexture = fieldTexture(this.cornerValues);
    this.tintValues = texels(world.stats.regionCount); this.cornerTintValues = texels(pair.corners.offsets.length - 1);
    this.tintTexture = fieldTexture(this.tintValues); this.cornerTintTexture = fieldTexture(this.cornerTintValues);
    this.detailValues = texels(world.stats.regionCount); this.cornerDetailValues = texels(pair.corners.offsets.length - 1);
    this.detailTexture = fieldTexture(this.detailValues); this.cornerDetailTexture = fieldTexture(this.cornerDetailValues);
    this.lodMaskValues = texels(world.stats.regionCount);
    this.lodMaskTexture = fieldTexture(this.lodMaskValues);
    const { gradients } = reliefGradients(world.surface, world.terrain.elevation, world.water.bodyIds, world.water.levelMeters);
    for (let id = 0; id < world.stats.regionCount; id++) this.values.set(gradients.subarray(id * 2, id * 2 + 2), id * 4 + 2);
    this.blendCorners(2);
    const uniforms = this.material.uniforms;
    uniforms.field.value = this.texture; uniforms.cornerField.value = this.cornerTexture;
    uniforms.tintField.value = this.tintTexture; uniforms.cornerTintField.value = this.cornerTintTexture;
    uniforms.detailField.value = this.detailTexture; uniforms.cornerDetailField.value = this.cornerDetailTexture;
    uniforms.lodMaskField.value = this.lodMaskTexture; uniforms.lodActive.value = 0;
    uniforms.textureWidth.value = this.texture.image.width; uniforms.textureHeight.value = this.texture.image.height;
    uniforms.cornerWidth.value = this.cornerTexture.image.width; uniforms.cornerHeight.value = this.cornerTexture.image.height;
    this.material.uniforms.selected.value = -1;
    this.setBasinContact();
    delete this.canvas.dataset.pickedSurface;
    this.setLayer(this.layer); this.setMode(this.mode); this.setBoundaries(this.boundaries); this.reset();
  }
  setWaterFrame(frame: WaterFrame): void {
    if (!this.world || !this.views || frame.depthMeters.length !== this.world.stats.regionCount
      || frame.surfaceLevelsMeters.length !== this.world.stats.regionCount || frame.bodyIds.length !== this.world.stats.regionCount) {
      throw new Error('Prescribed-water display does not match this world.');
    }
    this.waterFrame = frame;
    this.maximumDepth = frame.depthMeters.reduce((maximum, depth) => Math.max(maximum, depth), 0);
    this.syncWaterSurface();
    this.refreshField();
  }
  private syncWaterSurface(): void {
    if (!this.world || !this.views || this.mode !== 'globe' || !surfaceLike(this.layer)) return;
    const frame = isMovement(this.layer) ? this.world.water : this.regionalWater ?? this.waterFrame ?? this.world.water;
    if (frame !== this.globeWater) this.updateWaterSurface(frame);
  }
  private syncBedDrawRange(): void {
    if (!this.views || !this.world) return;
    const bed = (this.views.globe.children[0] as Mesh<BufferGeometry>).geometry;
    const landCoastCount = bed.userData.landCoastIndexCount as number | undefined;
    if (landCoastCount === undefined) return;
    // Skip underwater interior-ocean bed triangles when the globe water surface covers them.
    const skipUnderwater = this.mode === 'globe' && surfaceLike(this.layer) && this.globeWater === this.world.water;
    bed.setDrawRange(0, skipUnderwater ? landCoastCount : Infinity);
  }
  private updateWaterSurface(frame: Water | WaterFrame | RegionalWaterDisplay): void {
    if (!this.world || !this.views) throw new Error('Water display requires a prepared world.');
    const globe = this.views.globe;
    const water = new BufferGeometry();
    const lines = new BufferGeometry();
    if (this.compactRing) {
      const data = buildIndexedWaterSurface(this.world.surface, this.compactRing, frame);
      water.setAttribute('position', new BufferAttribute(data.waterPositions, 3));
      water.setAttribute('region', new BufferAttribute(data.waterRegions, 1));
      water.setAttribute('cornerPair', new BufferAttribute(data.waterCornerPairs, 2));
      water.setAttribute('weight', new BufferAttribute(data.waterWeights, 3));
      water.setIndex(new BufferAttribute(data.waterIndices, 1));
      water.userData.base = data.waterPositions.slice(); water.userData.offsets = data.waterOffsets;
      lines.setAttribute('position', new BufferAttribute(data.waterLines, 3));
      lines.userData.base = data.waterLines.slice(); lines.userData.offsets = data.waterLineOffsets;
    } else {
      const bed = (globe.children[0] as Mesh<BufferGeometry>).geometry;
      const base = bed.userData.base as Float32Array;
      const regions = bed.getAttribute('region').array as Float32Array;
      const data = buildWaterSurface(base, regions, frame, this.world.recipe.radiusMeters, {
        cornerPairs: bed.getAttribute('cornerPair').array as Float32Array, weights: bed.getAttribute('weight').array as Float32Array });
      water.setAttribute('position', new BufferAttribute(data.waterPositions, 3));
      water.setAttribute('region', new BufferAttribute(data.waterRegions, 1));
      water.setAttribute('cornerPair', new BufferAttribute(data.waterCornerPairs, 2));
      water.setAttribute('weight', new BufferAttribute(data.waterWeights, 3));
      water.userData.base = data.waterPositions.slice(); water.userData.offsets = data.waterOffsets;
      lines.setAttribute('position', new BufferAttribute(data.waterLines, 3));
      lines.userData.base = data.waterLines.slice(); lines.userData.offsets = data.waterLineOffsets;
    }
    const oldWater = globe.children[3] as Mesh<BufferGeometry>;
    const oldLines = globe.children[4] as LineSegments<BufferGeometry>;
    oldWater.geometry.dispose(); oldLines.geometry.dispose();
    oldWater.geometry = water; oldLines.geometry = lines;
    this.appliedExaggeration = NaN;
    this.globeWater = frame;
    this.syncBedDrawRange();
    this.setExaggeration(this.adaptiveExaggeration ? 'auto' : this.exaggeration);
  }
  setTemperatureNormals(normals: TemperatureNormals | null): void {
    if (normals && (!this.world || normals.annualMeanCelsius.length !== this.world.stats.regionCount)) {
      throw new Error('Seasonal-temperature normals do not match this world.');
    }
    this.temperatureNormals = normals;
    if (normals) {
      this.temperatureRange = {
        minimum: normals.annualMinimumCelsius.reduce((low, value) => Math.min(low, value), Infinity),
        maximum: normals.annualMaximumCelsius.reduce((high, value) => Math.max(high, value), -Infinity),
      };
    }
    if (this.seasonalSnow && this.layer === 'surface' && this.surfaceStyle === 'natural') this.applyTint();
    else this.refreshField();
  }
  /** Climate classes for the biome layer. */
  setClimateNormals(normals: ClimateNormals | null): void {
    if (normals && (!this.world || normals.biome.length !== this.world.stats.regionCount)) {
      throw new Error('Climate normals do not match this world.');
    }
    this.climateNormals = normals;
    this.applyTint();
  }
  setRivers(frame: Rivers): void {
    if (!this.world || !this.views || frame.runoff.length !== this.world.stats.regionCount) {
      throw new Error('River normals do not match this world.');
    }
    this.rivers = frame;
    this.valleyFactor = computeValleyField(this.world.surface, this.world.drainage, this.world.water.bodyIds, frame);
    this.rebuildDrainageLines();
    this.applyTint();
  }
  private rebuildDrainageLines(): void {
    if (!this.world || !this.views) return;
    for (const view of Object.values(this.views)) {
      for (const index of [RIVERS, RIVERS_DETAIL]) {
        const lines = view.children[index];
        for (const line of lines.children) (line as LineSegments2).geometry.dispose();
        lines.clear();
      }
    }
    this.addDrainageLines(this.views, this.world);
    this.appliedExaggeration = NaN;
    this.setExaggeration(this.adaptiveExaggeration ? 'auto' : this.exaggeration);
  }
  /** Vegetation state for the vegetation layers; display only. */
  setVegetation(frame: VegetationFrame | null): void {
    if (frame && (!this.world || frame.label.length !== this.world.stats.regionCount)) {
      throw new Error('Vegetation state does not match this world.');
    }
    this.vegetation = frame;
    this.applyTint();
  }
  /** Wild food for the food layers; display only. */
  setWildFood(frame: WildFoodFrame | null): void {
    if (frame && (!this.world || frame.capacity.length !== this.world.stats.regionCount)) {
      throw new Error('Wild food does not match this world.');
    }
    this.wildFood = frame;
    this.applyTint();
  }
  /** Foot-traversal costs for the Travel cost layer; display only. */
  setTraversalCosts(costs: TraversalCosts | null): void {
    if (costs && (!this.world || costs.flags.length !== this.world.stats.regionCount)) {
      throw new Error('Traversal costs do not match this world.');
    }
    this.traversalCosts = costs;
    this.applyTint();
  }
  /** Least-time Dijkstra field for the Travel time layer; display only. */
  setTravelField(field: TravelField | null): void {
    if (field && (!this.world || field.days.length !== this.world.stats.regionCount)) {
      throw new Error('Travel field does not match this world.');
    }
    this.travelField = field;
    if (field && field.sources.length) this.canvas.dataset.travelOrigin = String(field.sources[0]);
    else delete this.canvas.dataset.travelOrigin;
    this.applyTint();
  }
  frameTravelRoute(route: readonly number[]): void {
    if (this.mode !== 'globe' || !this.world || !route.length) return;
    const centers = this.world.surface.centers;
    const center = new Vector3(), point = new Vector3();
    for (const id of route) center.add(point.set(centers[id * 3], centers[id * 3 + 1], -centers[id * 3 + 2]));
    if (center.lengthSq() > 1e-8) this.planetCamera.frameDirection(center);
  }
  setTravelSeason(season: TravelSeason): void {
    this.travelSeason = season;
    this.canvas.dataset.travelSeason = season;
    this.applyTint();
  }
  /** Draw or clear the least-time route polyline on both flat map and globe; display only. */
  setTravelRoute(path: readonly number[]): void {
    this.canvas.dataset.travelRouteHops = String(Math.max(0, path.length - 1));
    if (!this.views || !this.world) return;
    for (const view of Object.values(this.views)) {
      const group = view.children[ROUTE];
      for (const child of [...group.children]) {
        if (child instanceof LineSegments2 || child instanceof Points) child.geometry.dispose();
        group.remove(child);
      }
    }
    const origin = this.travelField?.sources[0];
    if (origin !== undefined) {
      const point = routeOrigin(this.world.surface, this.world.terrain.elevation, this.world.water, origin);
      const applied = Number.isFinite(this.appliedExaggeration) ? this.appliedExaggeration : 0;
      for (const mode of ['flat', 'globe'] as const) {
        const geometry = new BufferGeometry();
        const positions = mode === 'flat' ? point.flat : displaceDirections(point.globe, point.offset, applied);
        geometry.setAttribute('position', new BufferAttribute(positions, 3));
        if (mode === 'globe') geometry.userData = { base: point.globe, offsets: point.offset };
        const marker = new Points(geometry, this.originMaterial);
        marker.renderOrder = ROUTE_RENDER_ORDER + 2;
        this.views[mode].children[ROUTE].add(marker);
      }
    }
    if (path.length >= 2) {
      const overlay = buildRouteOverlay(this.world.surface, this.world.terrain.elevation, this.world.water.bodyIds, this.world.water.levelMeters, path);
      if (overlay.flat.length) {
        const casing = new LineSegments2(new LineSegmentsGeometry().setPositions(overlay.flat), this.routeCasingMaterial);
        const core = new LineSegments2(new LineSegmentsGeometry().setPositions(overlay.flat), this.routeLineMaterial);
        casing.renderOrder = ROUTE_RENDER_ORDER; core.renderOrder = ROUTE_RENDER_ORDER + 1;
        this.views.flat.children[ROUTE].add(casing, core);
      }
      if (overlay.globe.positions.length) {
        const applied = Number.isFinite(this.appliedExaggeration) ? this.appliedExaggeration : 0;
        const displaced = displaceDirections(overlay.globe.positions, overlay.globe.offsets, applied);
        [this.routeCasingMaterial, this.routeLineMaterial].forEach((material, index) => {
          const geometry = new LineSegmentsGeometry().setPositions(displaced);
          geometry.userData.routeBase = overlay.globe.positions;
          geometry.userData.routeOffsets = overlay.globe.offsets;
          const line = new LineSegments2(geometry, material);
          line.renderOrder = ROUTE_RENDER_ORDER + index;
          this.views!.globe.children[ROUTE].add(line);
        });
      }
    }
    this.applyShadingModes();
    this.draw();
  }
  /** Per-region land colors of the active tint layer; shared corners average only dry regions. */
  private applyTint(): void {
    const climate = this.climateNormals, vegetation = this.vegetation, food = this.wildFood, rivers = this.rivers, costs = this.traversalCosts, field = this.travelField, layer = this.layer;
    const costField = costs ? (this.travelSeason === 'summer' ? costs.hoursPer100KmSummer : costs.hoursPer100KmWinter) : null;
    const timeDays = layer === 'travelTime' ? field?.days : undefined;
    const naturalSurface = layer === 'surface' && this.surfaceStyle === 'natural';
    this.material.uniforms.travelTimeMode.value = timeDays ? 1 : 0;
    this.material.uniforms.movementMode.value = isMovement(layer) ? 1 : 0;
    this.material.uniforms.naturalMode.value = naturalSurface ? 1 : 0;
    const color: ((id: number) => Rgb) | null = layer === 'biome' && climate ? (id) => BIOMES[climate.biome[id]].color
      : layer === 'vegetationState' && vegetation ? (id) => BIOMES[vegetation.label[id]].color
      : layer === 'woodyCover' && vegetation ? (id) => woodyColor(vegetation.woody[id])
      : layer === 'woodyChange' && vegetation ? (id) => changeColor(vegetation.woodyChange[id])
      : layer === 'productivity' && vegetation ? (id) => productivityColor(vegetation.productivity[id])
      : layer === 'precipitationAnomaly' && vegetation ? (id) => anomalyColor(vegetation.precipitationFactor[id])
      : layer === 'foragerCapacity' && food ? (id) => capacityColor(food.capacity[id])
      : layer === 'leanSeason' && food ? (id) => food.storedCapacity[id] > 0 ? leanColor(food.capacity[id] / food.storedCapacity[id]) : [206, 202, 192]
      : layer === 'gameStock' && food ? (id) => gameColor(food.game[id], food.gameCapacity[id])
      : layer === 'annualRunoff' && rivers ? (id) => runoffColor(rivers.runoff[id])
      : layer === 'riverDischarge' ? () => [194, 187, 165]
      : layer === 'travelCost' && costField ? (id) => travelCostColor(costField[id])
      : timeDays ? (id) => travelTimeColor(timeDays[id]) : null;
    const hasTint = naturalSurface || color !== null;
    this.material.uniforms.biomeMode.value = hasTint ? 1 : 0;
    if ((hasTint || surfaceLike(layer)) && this.world && this.corners && this.tintTexture && this.cornerTintTexture) {
      const tint = this.tintValues, out = this.cornerTintValues, surface = this.world.surface;
      const traversalLayer = isMovement(layer) && costs !== null;
      const activeBodyIds = traversalLayer ? this.world.water.bodyIds : this.waterFrame?.bodyIds ?? this.world.water.bodyIds;
      const activeWetMask: ArrayLike<number> = traversalLayer ? this.world.water.bodyIds : this.regionalWater?.wetMask ?? activeBodyIds;
      const wet = (id: number): boolean => traversalLayer ? !isRegionPassable(costs, id, this.travelSeason) : activeWetMask[id] !== 0;
      const count = this.world.stats.regionCount;
      const valley = this.valleyFactor ?? computeValleyField(surface, this.world.drainage, activeWetMask, this.rivers);

      if (naturalSurface) {
        const relief = this.reliefMetrics ?? computeReliefMetrics(surface, this.world.terrain.elevation, activeWetMask, this.world.water.levelMeters);
        const natural = computeNaturalSurface(this.world, climate, vegetation, this.temperatureNormals, relief, {
          month: this.temperatureMonth,
          seasonalSnow: this.seasonalSnow,
          wetMask: activeWetMask,
          bodyIds: activeBodyIds,
          mainOceanId: this.waterFrame?.mainOceanId ?? this.world.water.mainOceanId,
        });
        tint.set(natural.colors.subarray(0, count * 4));
        // Gentle display softening (28%) blends adjacent regions organically without erasing alpine snow or forest edges.
        const own = tint.slice(0, count * 4);
        for (let id = 0; id < count; id++) {
          if (wet(id)) continue;
          let r = 0, g = 0, b = 0, n = 0;
          for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
            const j = surface.neighbors[k];
            if (wet(j)) continue;
            r += own[j * 4]; g += own[j * 4 + 1]; b += own[j * 4 + 2]; n++;
          }
          if (n) {
            tint[id * 4] = .72 * own[id * 4] + .28 * r / n;
            tint[id * 4 + 1] = .72 * own[id * 4 + 1] + .28 * g / n;
            tint[id * 4 + 2] = .72 * own[id * 4 + 2] + .28 * b / n;
          }
          tint[id * 4 + 3] = valley[id];
        }
      } else if (color) {
        for (let id = 0; id < count; id++) {
          const rgb = color(id);
          for (let c = 0; c < 3; c++) tint[id * 4 + c] = rgb[c] / 255;
          const days = timeDays?.[id] ?? -1;
          tint[id * 4 + 3] = timeDays ? (days < 0 ? -1 : travelTimeCoordinate(days)) : valley[id];
        }
        if (layer === 'biome' || layer === 'vegetationState') {
          // Display-only softening: each dry region mixes half of its dry neighbors' mean
          // color so 110 km region edges read as transitions. Classes are unchanged.
          const own = tint.slice(0, count * 4);
          for (let id = 0; id < count; id++) {
            if (wet(id)) continue;
            let r = 0, g = 0, b = 0, n = 0;
            for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
              const j = surface.neighbors[k];
              if (wet(j)) continue;
              r += own[j * 4]; g += own[j * 4 + 1]; b += own[j * 4 + 2]; n++;
            }
            if (n) { tint[id * 4] = .5 * own[id * 4] + .5 * r / n; tint[id * 4 + 1] = .5 * own[id * 4 + 1] + .5 * g / n; tint[id * 4 + 2] = .5 * own[id * 4 + 2] + .5 * b / n; }
          }
        }
      } else {
        // Relief tint (hypsometric) mode still carries drainage valley factor in tint.a.
        for (let id = 0; id < count; id++) tint[id * 4 + 3] = valley[id];
      }

      // Water regions borrow their dry neighbors' mean so interpolated shores do not darken.
      for (let id = 0; id < count; id++) {
        if (!wet(id)) continue;
        let r = 0, g = 0, b = 0, time = 0, n = 0;
        for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
          const j = surface.neighbors[k];
          if (wet(j) || (timeDays && timeDays[j] < 0)) continue;
          r += tint[j * 4]; g += tint[j * 4 + 1]; b += tint[j * 4 + 2]; time += tint[j * 4 + 3]; n++;
        }
        if (n) {
          tint[id * 4] = r / n; tint[id * 4 + 1] = g / n; tint[id * 4 + 2] = b / n; tint[id * 4 + 3] = time / n;
        }
      }
      const { offsets, regions } = this.corners;
      for (let k = 0; k + 1 < offsets.length; k++) {
        let r = 0, g = 0, b = 0, time = 0, n = 0;
        for (const dryOnly of [true, false]) {
          for (let j = offsets[k]; j < offsets[k + 1]; j++) {
            const id = regions[j];
            if (dryOnly && (wet(id) || (timeDays && timeDays[id] < 0))) continue;
            r += tint[id * 4]; g += tint[id * 4 + 1]; b += tint[id * 4 + 2]; time += tint[id * 4 + 3]; n++;
          }
          if (n) break;
        }
        out[k * 4] = r / n; out[k * 4 + 1] = g / n; out[k * 4 + 2] = b / n; out[k * 4 + 3] = time / n;
      }
      this.tintTexture.needsUpdate = true; this.cornerTintTexture.needsUpdate = true;
    }
    this.syncRiverPalette();
    this.refreshField();
  }
  setWindNormals(normals: WindNormals | null): void {
    if (normals && (!this.world || normals.monthlyEastMetersPerSecond.some((values) => values.length !== this.world!.stats.regionCount)
      || normals.monthlyNorthMetersPerSecond.some((values) => values.length !== this.world!.stats.regionCount))) {
      throw new Error('Seasonal-wind normals do not match this world.');
    }
    this.windNormals = normals;
    if (normals) {
      let maximum = 0;
      for (let month = 0; month < 12; month++) {
        const east = normals.monthlyEastMetersPerSecond[month], north = normals.monthlyNorthMetersPerSecond[month];
        for (let id = 0; id < east.length; id++) maximum = Math.max(maximum, Math.hypot(east[id], north[id]));
      }
      this.windMaximum = Math.max(maximum, 1);
    }
    this.refreshField();
  }
  setMoistureFrame(frame: SeasonalDisplayFrame | null): void {
    const stocks = frame ? isSoilMoistureFrame(frame)
      ? Object.values(frame.stocks).flatMap(p => [p.high, p.low]) : Object.values(frame.stocks) : [];
    if (frame && (!this.world || stocks.some(field => field.length !== this.world!.stats.regionCount))) {
      throw new Error('Seasonal-water display does not match this world.');
    }
    this.moistureFrame = frame;
    if (frame && hasRegionalSurface(frame)) {
      this.regionalWater = regionalWaterDisplay(this.world!, frame);
      this.maximumDepth = this.regionalWater.depthMeters.reduce((maximum, depth) => Math.max(maximum, depth), 0);
      this.canvas.dataset.regionalSurface = 'active';
    } else {
      this.regionalWater = null;
      delete this.canvas.dataset.regionalSurface;
    }
    this.syncWaterSurface();
    if (frame) this.canvas.dataset.moistureSeconds = String(frame.elapsedSeconds);
    else delete this.canvas.dataset.moistureSeconds;
    if (frame) this.canvas.dataset.seasonalModel = frame.modelVersion;
    else delete this.canvas.dataset.seasonalModel;
    // Legacy modes update only textures. Regional caps rebuild lazily only for
    // the visible physical globe; flat/analytical playback never rebuilds them.
    if (isMoistureLayer(this.layer) || (hasRegionalSurface(frame) && ['surface', 'biome', 'depth'].includes(this.layer))) this.refreshField();
  }
  setTemperatureMonth(month: number): void {
    if (!Number.isInteger(month) || month < 0 || month >= 12) throw new Error('Invalid seasonal month.');
    this.temperatureMonth = month; this.canvas.dataset.temperatureMonth = String(month); this.canvas.dataset.climateMonth = String(month);
    if (this.seasonalSnow && this.layer === 'surface' && this.surfaceStyle === 'natural') this.applyTint();
    else this.refreshField();
  }
  setMode(mode: ViewMode): void {
    this.stopCamera();
    this.clearMapDrag();
    this.mode = mode; this.canvas.dataset.view = mode;
    this.syncWaterSurface();
    this.material.uniforms.globe.value = mode === 'globe' ? 1 : 0;
    const faceSide = mode === 'globe' ? FrontSide : DoubleSide;
    this.material.side = faceSide; this.waterMaterial.side = faceSide; this.lodMaterial.side = faceSide;
    this.space.visible = mode === 'globe';
    this.renderer.setClearColor(mode === 'globe' ? '#040507' : '#0c1721');
    if (this.views) { this.views.flat.visible = mode === 'flat'; this.views.globe.visible = mode === 'globe'; }
    this.controls.enabled = mode === 'flat';
    this.controls.mouseButtons.LEFT = MOUSE.PAN;
    this.planetCamera.setEnabled(mode === 'globe');
    this.syncBedDrawRange();
    if (mode === 'globe') { this.syncAdaptiveExaggeration(); this.scheduleLodUpdate(true); }
    else this.clearLodPatch();
    this.notifyNavigation(); this.draw();
  }
  setLayer(layer: Layer): void {
    this.layer = layer;
    this.space.setSurfaceMode(surfaceLike(layer));
    this.syncWaterSurface();
    this.syncBedDrawRange();
    this.canvas.dataset.activeLayer = layer;
    this.notifyNavigation();
    this.material.uniforms.surfaceMode.value = surfaceLike(layer) ? 1 : 0;
    this.material.uniforms.precipitationMode.value = layer === 'annualPrecipitation' ? 1 : 0;
    this.material.uniforms.temperatureMode.value = layer === 'temperature' ? 1 : 0;
    this.material.uniforms.windMode.value = layer === 'windSpeed' ? 1 : 0;
    this.material.uniforms.categorical.value = layer === 'plates' || layer === 'boundaries' || layer === 'waterBodies' || layer === 'catchments' || layer === 'basins' ? 1 : 0;
    this.material.uniforms.missingMode.value = layer === 'spill' ? 1 : 0;
    this.material.uniforms.basinMode.value = layer === 'basins' || layer === 'spill' ? 1 : 0;
    this.material.uniforms.waterMode.value = layer === 'depth' ? 1 : layer === 'waterBodies' ? 2 : 0;
    this.material.uniforms.muted.value = layer === 'boundaries' ? 1 : 0;
    this.applyShadingModes();
    if (this.views) for (const view of Object.values(this.views)) {
      view.children[2].visible = layer === 'plates' || layer === 'boundaries';
      view.children[3].visible = surfaceLike(layer);
      view.children[4].visible = surfaceLike(layer) && this.boundaries;
    }
    this.applyTint();
    this.scheduleLodUpdate(true);
  }
  refreshField(): void {
    if (!this.world || !this.texture) return;
    const w = this.world;
    const staticWater = isMovement(this.layer);
    const depth = staticWater ? w.water.depthMeters : this.regionalWater?.depthMeters ?? this.waterFrame?.depthMeters ?? w.water.depthMeters;
    const bodyIds = staticWater ? w.water.bodyIds : this.waterFrame?.bodyIds ?? w.water.bodyIds;
    const mainOceanId = staticWater ? w.water.mainOceanId : this.waterFrame?.mainOceanId ?? w.water.mainOceanId;
    const surfaceLevels = staticWater ? undefined : this.regionalWater?.surfaceLevelsMeters ?? this.waterFrame?.surfaceLevelsMeters;
    let surfaceMask: ArrayLike<number> = staticWater ? bodyIds : this.regionalWater?.wetMask ?? bodyIds;
    if (isMovement(this.layer) && this.traversalCosts) {
      const passableMask = new Uint8Array(w.stats.regionCount);
      for (let id = 0; id < w.stats.regionCount; id++) passableMask[id] = isRegionPassable(this.traversalCosts, id, this.travelSeason) ? 0 : 1;
      surfaceMask = passableMask;
    }
    const values = this.values, masked = surfaceLike(this.layer) || this.layer === 'depth';
    const mask = masked ? relaxedMask(w.surface, surfaceMask) : null;
    for (let id = 0; id < w.stats.regionCount; id++) {
      const wet = surfaceMask[id] ? 1 : 0;
      values[id * 4 + 1] = mask ? mask[id] : 1;
      values[id * 4] = isMoistureLayer(this.layer) ? (this.moistureFrame && supportsMoistureLayer(this.moistureFrame, this.layer)
        ? moistureLayerColor(moistureLayerValue(this.moistureFrame, this.layer, id, w.surface.areasSquareMeters[id]), this.layer) : 0)
        : this.layer === 'plates' || this.layer === 'boundaries' ? w.tectonics.owners[id]
        : this.layer === 'temperature' ? (this.temperatureNormals
          ? (this.temperatureNormals.monthlyTemperatureCelsius[this.temperatureMonth][id] - this.temperatureRange.minimum)
            / Math.max(1, this.temperatureRange.maximum - this.temperatureRange.minimum) : 0.5)
        : this.layer === 'windSpeed' ? (this.windNormals
          ? Math.hypot(this.windNormals.monthlyEastMetersPerSecond[this.temperatureMonth][id],
            this.windNormals.monthlyNorthMetersPerSecond[this.temperatureMonth][id]) / this.windMaximum : 0)
        : this.layer === 'basins' ? w.basins.regionNodes[id]
        : this.layer === 'spill' ? (w.basins.parents[w.basins.regionNodes[id]] === w.basins.regionNodes[id] ? -1
          : (w.basins.spillLevels[w.basins.regionNodes[id]] - this.spillRange.minimum) / Math.max(1, this.spillRange.maximum - this.spillRange.minimum))
        : this.layer === 'catchments' ? w.drainage.outlets[id]
        : this.layer === 'contributingArea' ? Math.log1p(w.drainage.contributingArea[id] / 1e6) / Math.max(1e-30, Math.log1p(this.maximumContributingArea / 1e6))
        : surfaceLike(this.layer) ? (wet ? -depth[id] : w.terrain.elevation[id] - w.water.levelMeters)
        : this.layer === 'annualPrecipitation' ? (this.climateNormals
          ? Math.min(1, Math.log1p(this.climateNormals.annualPrecipitationMm[id] / 50) / Math.log1p(4000 / 50)) : 0)
        : this.layer === 'depth' ? (wet ? depth[id] / Math.max(1e-30, this.maximumDepth) : 0)
        : this.layer === 'waterBodies' ? bodyIds[id]
        : this.layer === 'elevation' ? (w.terrain.elevation[id] - this.terrainRange.minimumMeters) / Math.max(1, this.terrainRange.maximumMeters - this.terrainRange.minimumMeters)
        : this.layer === 'uplift' ? w.terrain.convergence[id] / 12000
        : this.layer === 'crust' ? w.crust.continentality[id]
        : this.layer === 'thickness' ? (w.crust.thicknessMeters[id] - 7000) / 28000
        : this.layer === 'speed' ? speedCmPerYear(w.surface, w.tectonics, id) / Math.max(1e-30, w.recipe.maxPlateSpeedCmPerYear)
        : this.layer === 'signal' ? (w.diagnosticField[id] + 1) / 2
        : this.layer === 'latitude' ? 1 - Math.abs(Math.asin(w.surface.centers[id * 3 + 1])) / (Math.PI / 2)
          : (w.surface.areasSquareMeters[id] - w.stats.minimumAreaSquareMeters)
            / Math.max(1, w.stats.maximumAreaSquareMeters - w.stats.minimumAreaSquareMeters);
    }
    if (SMOOTH_LAYERS.has(this.layer) || isMoistureLayer(this.layer)) this.blendCorners(0);
    if (surfaceLike(this.layer) && this.detailTexture && this.cornerDetailTexture && this.corners) {
      this.reliefMetrics = computeReliefMetrics(w.surface, w.terrain.elevation, surfaceMask, w.water.levelMeters);
      this.coastField = computeCoastField(w.surface, w.terrain.elevation, surfaceMask, depth, w.water.levelMeters, bodyIds, mainOceanId, surfaceLevels);
      this.valleyFactor = computeValleyField(w.surface, w.drainage, surfaceMask, this.rivers);
      const count = w.stats.regionCount;
      const dVals = this.detailValues, dOut = this.cornerDetailValues;
      const { phi, strictMask, lakeMask, localWaterLevel } = this.coastField;
      const rel = this.reliefMetrics.effectiveReliefMeters;
      for (let id = 0; id < count; id++) {
        dVals[id * 4] = phi[id];
        dVals[id * 4 + 1] = strictMask[id];
        dVals[id * 4 + 2] = rel[id];
        dVals[id * 4 + 3] = lakeMask[id];
      }
      const { offsets, regions } = this.corners;
      for (let k = 0; k + 1 < offsets.length; k++) {
        let p = 0, s = 0, r = 0, lk = 0, wetN = 0;
        const nInc = offsets[k + 1] - offsets[k];
        for (let j = offsets[k]; j < offsets[k + 1]; j++) {
          const rid = regions[j];
          p += phi[rid]; s += strictMask[rid]; r += rel[rid];
          if (strictMask[rid] < 0) { lk += lakeMask[rid]; wetN++; }
        }
        dOut[k * 4] = p / nInc;
        dOut[k * 4 + 1] = s / nInc;
        dOut[k * 4 + 2] = r / nInc;
        dOut[k * 4 + 3] = wetN > 0 ? lk / wetN : 0;
      }
      this.detailTexture.needsUpdate = true;
      this.cornerDetailTexture.needsUpdate = true;
      if (this.worker && !isMovement(this.layer)) {
        const wetBytes = new Uint8Array(count);
        for (let id = 0; id < count; id++) wetBytes[id] = surfaceMask[id] ? 1 : 0;
        const phiCopy = phi.slice();
        const strictCopy = strictMask.slice();
        const wlCopy = localWaterLevel.slice();
        const rawRelCopy = this.reliefMetrics.localReliefMeters.slice();
        const effRelCopy = rel.slice();
        const valCopy = this.valleyFactor.slice();
        this.worker.postMessage({
          type: 'lod-fields',
          waterLevelMeters: w.water.levelMeters,
          wetMask: wetBytes,
          phi: phiCopy,
          strictMask: strictCopy,
          localWaterLevel: wlCopy,
          localReliefMeters: rawRelCopy,
          effectiveReliefMeters: effRelCopy,
          valleyFactor: valCopy,
        }, [wetBytes.buffer, phiCopy.buffer, strictCopy.buffer, wlCopy.buffer, rawRelCopy.buffer, effRelCopy.buffer, valCopy.buffer]);
      }
    }
    this.texture.needsUpdate = true; this.draw();
  }
  /** Shared corners take the mean of their incident regions for two channels starting at `channel`. */
  private blendCorners(channel: number): void {
    if (!this.corners || !this.cornerTexture) return;
    const { offsets, regions } = this.corners, values = this.values, out = this.cornerValues;
    for (let k = 0; k + 1 < offsets.length; k++) {
      let a = 0, b = 0;
      for (let j = offsets[k]; j < offsets[k + 1]; j++) { const i = regions[j] * 4 + channel; a += values[i]; b += values[i + 1]; }
      const count = offsets[k + 1] - offsets[k];
      out[k * 4 + channel] = a / count; out[k * 4 + channel + 1] = b / count;
    }
    this.cornerTexture.needsUpdate = true;
  }
  private applyShadingModes(): void {
    const smooth = this.smoothShading && (SMOOTH_LAYERS.has(this.layer) || isMoistureLayer(this.layer));
    this.material.uniforms.smoothMode.value = smooth ? 1 : 0;
    this.material.uniforms.detailEnabled.value = this.displayDetail ? 1 : 0;
    this.material.uniforms.reliefStrength.value = this.reliefShading ? RELIEF_LAYERS[this.layer] ?? 0 : 0;
    this.canvas.dataset.smoothShading = String(smooth);
    this.canvas.dataset.displayDetail = String(this.displayDetail);
    const showRivers = this.drainageLines && this.layer !== 'annualRunoff' && (surfaceLike(this.layer) || this.layer === 'elevation');
    if (this.views) for (const view of Object.values(this.views)) {
      view.children[RIVERS].visible = showRivers;
      view.children[RIVERS_DETAIL].visible = showRivers;
      view.children[ROUTE].visible = this.layer === 'travelTime';
    }
    this.syncLodVisibility();
  }
  setSmoothShading(enabled: boolean): void { this.smoothShading = enabled; this.applyShadingModes(); this.refreshField(); this.scheduleLodUpdate(true); }
  setReliefShading(enabled: boolean): void { this.reliefShading = enabled; this.applyShadingModes(); this.draw(); }
  setDrainageLines(visible: boolean): void { this.drainageLines = visible; this.applyShadingModes(); this.draw(); }
  setSurfaceStyle(style: SurfaceStyle): void {
    this.surfaceStyle = style;
    this.canvas.dataset.surfaceStyle = style;
    this.applyTint();
    this.notifyNavigation();
  }
  getSurfaceStyle(): SurfaceStyle { return this.surfaceStyle; }
  setSeasonalSnow(enabled: boolean): void {
    this.seasonalSnow = enabled;
    this.canvas.dataset.seasonalSnow = String(enabled);
    if (this.layer === 'surface' && this.surfaceStyle === 'natural') this.applyTint();
  }
  setDisplayDetail(enabled: boolean): void {
    this.displayDetail = enabled;
    this.canvas.dataset.displayDetail = String(enabled);
    this.applyShadingModes();
    this.rebuildDrainageLines();
    this.scheduleLodUpdate(true);
    this.notifyNavigation();
    this.draw();
  }
  getDisplayDetail(): boolean { return this.displayDetail; }
  private syncRiverPalette(): void {
    const useNatural = this.layer === 'surface' && this.surfaceStyle === 'natural';
    const baseOffset = DRAINAGE_LINE_WIDTHS.length;
    const bins = RIVER_LINE_WIDTHS.length;
    for (let i = 0; i < bins; i++) {
      const pColor = useNatural ? NATURAL_PERENNIAL_COLORS[i] : PERENNIAL_LINE_COLORS[i];
      const sColor = useNatural ? NATURAL_SEASONAL_COLORS[i] : SEASONAL_LINE_COLORS[i];
      const pWidth = useNatural ? NATURAL_RIVER_WIDTHS[i] : RIVER_LINE_WIDTHS[i];
      const sWidth = pWidth * 0.68;
      this.riverMaterials[baseOffset + i].color.setHex(pColor);
      this.riverMaterials[baseOffset + i].linewidth = pWidth;
      this.riverMaterials[baseOffset + bins + i].color.setHex(sColor);
      this.riverMaterials[baseOffset + bins + i].linewidth = sWidth;
      this.riverDetailMaterials[baseOffset + i].color.setHex(pColor);
      this.riverDetailMaterials[baseOffset + i].linewidth = pWidth * 0.88;
      this.riverDetailMaterials[baseOffset + bins + i].color.setHex(sColor);
      this.riverDetailMaterials[baseOffset + bins + i].linewidth = sWidth * 0.88;
    }
  }
  private addDrainageLines(views: Record<ViewMode, Group>, world: World): void {
    const lines = buildDrainageLines(world.surface, world.drainage, world.terrain.elevation, world.water.bodyIds, this.rivers ?? undefined, {
      seed: world.recipe.seed,
      waterLevelMeters: world.water.levelMeters,
      coastField: this.coastField ?? undefined,
      reliefMetrics: this.reliefMetrics ?? undefined,
      valleyFactor: this.valleyFactor ?? undefined,
      detailEnabled: this.displayDetail,
    });
    const offset = this.rivers ? DRAINAGE_LINE_WIDTHS.length : 0;
    this.canvas.dataset.riverLineMode = this.rivers ? 'discharge' : 'area';
    const primarySegs = lines.flat.reduce((sum, v) => sum + v.length / 6, 0);
    const detailSegs = lines.flatDetail.reduce((sum, v) => sum + v.length / 6, 0);
    this.canvas.dataset.drainageLineSegments = String(primarySegs + detailSegs);
    const attach = (
      flatArr: Float32Array[],
      globeArr: { positions: Float32Array; offsets: Float32Array }[],
      mats: LineMaterial[],
      childIdx: number,
    ): void => {
      flatArr.forEach((positions, width) => {
        if (!positions.length) return;
        const geometry = new LineSegmentsGeometry().setPositions(positions);
        const line = new LineSegments2(geometry, mats[offset + width]);
        line.renderOrder = this.riverRenderOrder(width);
        views.flat.children[childIdx].add(line);
      });
      globeArr.forEach(({ positions, offsets }, width) => {
        if (!positions.length) return;
        const geometry = new LineSegmentsGeometry().setPositions(positions);
        geometry.userData.riverBase = positions; geometry.userData.riverOffsets = offsets;
        const line = new LineSegments2(geometry, mats[offset + width]);
        line.renderOrder = this.riverRenderOrder(width);
        views.globe.children[childIdx].add(line);
      });
    };
    attach(lines.flat, lines.globe, this.riverMaterials, RIVERS);
    attach(lines.flatDetail, lines.globeDetail, this.riverDetailMaterials, RIVERS_DETAIL);
    this.syncRiverPalette();
  }
  private riverRenderOrder(width: number): number {
    const bins = RIVER_LINE_WIDTHS.length;
    return 1 + (this.rivers ? 2 * (width % bins) + (width < bins ? 1 : 0) : width);
  }
  setBoundaries(visible: boolean): void {
    this.boundaries = visible;
    if (visible && this.linesExaggerationDirty && this.views && Number.isFinite(this.appliedExaggeration)) {
      this.syncBoundaryLineExaggeration(this.appliedExaggeration);
    }
    if (this.views) for (const group of Object.values(this.views)) {
      group.children[1].visible = visible; group.children[4].visible = visible && surfaceLike(this.layer);
    }
    this.draw();
  }
  private syncBoundaryLineExaggeration(applied: number): void {
    if (!this.views) return;
    for (const idx of [1, 4]) {
      const lineObj = this.views.globe.children[idx] as LineSegments<BufferGeometry>;
      const { base, offsets } = lineObj.geometry.userData as { base?: Float32Array; offsets?: Float32Array };
      if (!base || !offsets) continue;
      const attribute = lineObj.geometry.getAttribute('position') as BufferAttribute;
      displaceDirections(base, offsets, applied, attribute.array as Float32Array);
      attribute.needsUpdate = true;
      lineObj.geometry.computeBoundingSphere();
    }
    this.linesExaggerationDirty = false;
  }
  setBasinContact(from = -1, to = -1): void {
    this.material.uniforms.spillFrom.value = from; this.material.uniforms.spillTo.value = to;
    this.canvas.dataset.spillFrom = String(from); this.canvas.dataset.spillTo = String(to);
    this.draw();
  }
  private adaptiveExaggerationForDistance(distance: number): number {
    // 3.5× at planet overview (calm orbital scale), rising smoothly to 6.5× when zoomed in close.
    const t = Math.max(0, Math.min(1, (2.0 - distance) / 1.65));
    const smooth = t * t * (3 - 2 * t);
    return Math.round((3.5 + 3.0 * smooth) * 4) / 4;
  }
  private syncAdaptiveExaggeration(): void {
    if (!this.adaptiveExaggeration || !this.world || this.mode !== 'globe') return;
    const target = this.adaptiveExaggerationForDistance(this.planetCamera.view.distance);
    if (Math.abs(target - this.exaggeration) >= 0.25) this.applyExaggeration(target);
  }
  setExaggeration(requested: number | 'auto'): number {
    if (requested === 'auto') {
      this.adaptiveExaggeration = true;
      const dist = this.mode === 'globe' ? this.planetCamera.view.distance : 2.25;
      return this.applyExaggeration(this.adaptiveExaggerationForDistance(dist));
    }
    this.adaptiveExaggeration = false;
    return this.applyExaggeration(requested);
  }
  private applyExaggeration(requested: number): number {
    const liveWater = isMovement(this.layer) ? null : this.regionalWater ?? this.waterFrame;
    const waterExtent = liveWater
      ? liveWater.surfaceLevelsMeters.reduce((maximum, level) => Math.max(maximum, Math.abs(level)), 0)
      : this.world?.water.mainOceanId ? this.world.water.levelMeters : undefined;
    const applied = this.world ? effectiveExaggeration(requested, this.world.recipe.radiusMeters, this.world.terrain.elevation,
      waterExtent) : requested;
    this.exaggeration = requested;
    this.material.uniforms.lodExaggeration.value = applied;
    if (this.appliedExaggeration === applied) return applied;
    this.appliedExaggeration = applied;
    if (this.views) for (const idx of [RIVERS, RIVERS_DETAIL]) {
      for (const river of this.views.globe.children[idx].children as LineSegments2[]) {
        const { riverBase, riverOffsets } = river.geometry.userData as { riverBase: Float32Array; riverOffsets: Float32Array };
        river.geometry.setPositions(displaceDirections(riverBase, riverOffsets, applied));
      }
    }
    if (this.views) for (const object of this.views.globe.children[ROUTE].children) {
      if (object instanceof LineSegments2) {
        const { routeBase, routeOffsets } = object.geometry.userData;
        object.geometry.setPositions(displaceDirections(routeBase, routeOffsets, applied));
      } else if (object instanceof Points) {
        const { base, offsets } = object.geometry.userData;
        object.geometry.setAttribute('position', new BufferAttribute(displaceDirections(base, offsets, applied), 3));
        object.geometry.computeBoundingSphere();
      }
    }
    if (this.views) {
      const children = this.views.globe.children;
      for (let idx = 0; idx < children.length; idx++) {
        const object = children[idx];
        if (!(object instanceof Mesh || object instanceof LineSegments)) continue;
        // Skip hidden region boundary lines during zoom; lazily sync if boundaries are toggled on.
        if ((idx === 1 || idx === 4) && !this.boundaries) { this.linesExaggerationDirty = true; continue; }
        const geometry = (object as Mesh<BufferGeometry>).geometry;
        const { base, offsets } = geometry.userData as { base?: Float32Array; offsets?: Float32Array };
        if (!base || !offsets) continue;
        const attribute = geometry.getAttribute('position') as BufferAttribute;
        displaceDirections(base, offsets, applied, attribute.array as Float32Array); attribute.needsUpdate = true;
        geometry.computeBoundingSphere();
      }
      if (this.boundaries) this.linesExaggerationDirty = false;
    }
    const outerRadius = this.world ? Math.max(1, 1 + applied * Math.max(0, this.terrainRange.maximumMeters, waterExtent ?? 0) / this.world.recipe.radiusMeters) : 1;
    const atmosphericReference = this.world?.water.mainOceanId
      ? 1 + applied * this.world.water.levelMeters / this.world.recipe.radiusMeters : 1;
    this.material.uniforms.waterLevelRadius.value = atmosphericReference;
    this.planetCamera.setRadius(outerRadius); this.space.setRadius(outerRadius, atmosphericReference);
    this.canvas.dataset.exaggeration = String(applied); this.draw(); return applied;
  }
  private scheduleLodUpdate(force = false): void {
    if (!this.world || !this.views || !this.worker) return;
    const eligible = this.mode === 'globe' && surfaceLike(this.layer) && this.smoothShading && !isMovement(this.layer);
    const view = this.planetCamera.view;
    if (!eligible || view.distance >= 1.65) {
      this.clearLodPatch();
      return;
    }
    const lat = view.latitude * Math.PI / 180;
    const lon = view.longitude * Math.PI / 180;
    const headingRad = view.heading * Math.PI / 180;
    const tiltRad = view.tilt * Math.PI / 180;
    const cosLat = Math.cos(lat), sinLat = Math.sin(lat), cosLon = Math.cos(lon), sinLon = Math.sin(lon);
    const focus: [number, number, number] = [cosLat * cosLon, sinLat, cosLat * sinLon];
    // Model-space local east/north tangent frame matching globe.scale.z = -1 reflection.
    const mEast: [number, number, number] = [-sinLon, 0, cosLon];
    const mNorth: [number, number, number] = [-sinLat * cosLon, cosLat, -sinLat * sinLon];
    const forward: [number, number, number] = [
      mNorth[0] * Math.cos(headingRad) + mEast[0] * Math.sin(headingRad),
      mNorth[1] * Math.cos(headingRad) + mEast[1] * Math.sin(headingRad),
      mNorth[2] * Math.cos(headingRad) + mEast[2] * Math.sin(headingRad),
    ];
    const targetSub = view.distance < 0.52 ? 4 : view.distance < 1.05 ? 3 : 2;
    const dotFocus = focus[0] * this.lastLodFocus[0] + focus[1] * this.lastLodFocus[1] + focus[2] * this.lastLodFocus[2];
    const movedAngle = Math.acos(Math.max(-1, Math.min(1, dotFocus)));
    const distRatio = Number.isFinite(this.lastLodDistance) ? Math.abs(view.distance - this.lastLodDistance) / Math.max(0.05, view.distance) : 1;
    const tiltChanged = Math.abs(view.tilt - this.lastLodTilt) > 12;
    if (!force && this.lodTriangles > 0 && targetSub === this.lastLodSubdivision && movedAngle < 0.06 && distRatio < 0.20 && !tiltChanged) return;

    clearTimeout(this.lodTimer);
    this.lodTimer = window.setTimeout(() => {
      this.lodTimer = 0;
      if (!this.worker || !this.world || this.mode !== 'globe') return;
      const reqId = ++this.lodRequestId;
      this.lastLodFocus = focus;
      this.lastLodDistance = view.distance;
      this.lastLodTilt = view.tilt;
      this.lastLodSubdivision = targetSub;
      const seedVec = seedOffsetFromString(this.world.recipe.seed);
      this.worker.postMessage({
        type: 'lod',
        requestId: reqId,
        focus,
        distance: view.distance,
        tiltRad,
        forward,
        seedOffset: seedVec,
        detailEnabled: this.displayDetail,
      });
    }, force ? 0 : 40);
  }
  private clearLodPatch(): void {
    clearTimeout(this.lodTimer); this.lodTimer = 0; this.lodRequestId++;
    if (this.lodTriangles === 0 && this.material.uniforms.lodActive.value === 0) return;
    this.lodTriangles = 0;
    this.lastLodDistance = Infinity;
    this.lastLodSubdivision = 1;
    if (this.lodMaskTexture && this.lodMaskValues.length) {
      this.lodMaskValues.fill(0);
      this.lodMaskTexture.needsUpdate = true;
    }
    this.material.uniforms.lodActive.value = 0;
    if (this.views) {
      const lodMesh = this.views.globe.children[LOD] as Mesh<BufferGeometry>;
      lodMesh.visible = false;
    }
    this.canvas.dataset.lodTriangles = '0';
    this.canvas.dataset.lodLevel = '1';
    this.draw();
  }
  private applyLodPatch(patch: LodPatchData | null): void {
    if (!this.views || !this.lodMaskTexture) return;
    if (!patch || patch.triangleCount === 0) { this.clearLodPatch(); return; }
    const lodMesh = this.views.globe.children[LOD] as Mesh<BufferGeometry>;
    const nextGeom = new BufferGeometry();
    nextGeom.setAttribute('position', new BufferAttribute(patch.positions, 3));
    nextGeom.setAttribute('region', new BufferAttribute(patch.regions, 1));
    nextGeom.setAttribute('cornerPair', new BufferAttribute(patch.cornerPairs, 2));
    nextGeom.setAttribute('weight', new BufferAttribute(patch.weights, 3));
    nextGeom.setAttribute('radialOffset', new BufferAttribute(patch.radialOffsets, 1));
    lodMesh.geometry.dispose();
    lodMesh.geometry = nextGeom;
    this.lodMaskValues.fill(0);
    for (let i = 0; i < patch.replacedRegions.length; i++) {
      this.lodMaskValues[patch.replacedRegions[i] * 4] = 1;
    }
    this.lodMaskTexture.needsUpdate = true;
    this.lodTriangles = patch.triangleCount;
    this.canvas.dataset.lodTriangles = String(patch.triangleCount);
    this.canvas.dataset.lodLevel = String(patch.subdivision);
    this.syncLodVisibility();
    this.draw();
  }
  private syncLodVisibility(): void {
    const show = this.mode === 'globe' && surfaceLike(this.layer) && this.smoothShading && !isMovement(this.layer) && this.lodTriangles > 0;
    this.material.uniforms.lodActive.value = show ? 1 : 0;
    if (this.views) (this.views.globe.children[LOD] as Mesh).visible = show;
  }
  reset(): void {
    this.stopCamera();
    if (this.mode === 'globe') { this.planetCamera.reset(); return; }
    this.controls.target.set(0, 0, 0);
    this.flatCamera.position.set(0, 0, 3); this.flatCamera.zoom = 1; this.flatCamera.updateProjectionMatrix();
    this.controls.update(); this.notifyNavigation(); this.draw();
  }
  zoomBy(factor: number): void {
    if (this.mode === 'globe') { this.planetCamera.zoom(factor); return; }
    if (this.mode === 'flat') { this.flatCamera.zoom = Math.max(.5, Math.min(30, this.flatCamera.zoom * factor)); this.flatCamera.updateProjectionMatrix(); }
    this.controls.update(); this.notifyNavigation(); this.draw();
  }
  get navigation(): NavigationView {
    return { ...(this.mode === 'globe' ? this.planetCamera.view : { heading: 0, tilt: 0, latitude: Math.max(-90, Math.min(90, this.controls.target.y * 180)),
      longitude: ((this.controls.target.x * 180 + 180) % 360 + 360) % 360 - 180, distance: 0 }), mode: this.mode, radiusMeters: this.world?.recipe.radiusMeters ?? 0 };
  }
  private notifyNavigation(): void {
    if (!this.planetCamera) return;
    const view = this.navigation;
    this.canvas.dataset.heading = view.heading.toFixed(2); this.canvas.dataset.tilt = view.tilt.toFixed(2);
    this.canvas.dataset.viewDistance = view.distance.toFixed(5);
    this.canvas.dispatchEvent(new CustomEvent<NavigationView>('viewchange', { detail: view }));
  }
  northUp(): void { if (this.mode === 'globe') this.planetCamera.north(); }
  rotateView(degrees: number): void { if (this.mode === 'globe') this.planetCamera.rotate(degrees); }
  toggleTilt(): void { if (this.mode === 'globe') this.planetCamera.toggleTilt(); }
  stopCamera(): void {
    this.planetCamera.stop(); this.stopDamping();
    this.controls.enableDamping = false; this.controls.update(); this.controls.enableDamping = true;
  }
  setStars(visible: boolean): void {
    this.space.setStars(visible); this.canvas.dataset.skyVisible = String(visible); this.notifyNavigation(); this.draw();
  }
  setAtmosphere(visible: boolean): void { this.space.setAtmosphere(visible); this.draw(); }
  setDistantLighting(enabled: boolean): void { this.distantLighting = enabled; this.draw(); }
  private draw(): void {
    if (this.frame || this.lost) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      if (!this.lost) {
        const distance = this.planetCamera.view.distance;
        const fade = Math.max(0, Math.min(1, (distance - 3) / 2));
        const nightBlend = this.mode === 'globe' && surfaceLike(this.layer) && this.distantLighting ? fade * fade * (3 - 2 * fade) : 0;
        this.material.uniforms.distantLightMix.value = nightBlend;
        this.canvas.dataset.dayNight = nightBlend.toFixed(3);
        this.lineMaterial.opacity = .45 * (1 - nightBlend * .8);

        // Keep globe survey sun at an oblique NW angle in the local surface tangent frame
        // (independent of camera tilt, lowering from ~48° at orbit to ~32° close up so relief pops).
        if (this.mode === 'globe') {
          const view = this.planetCamera.view;
          const zoomT = Math.max(0, Math.min(1, (1.35 - distance) / 1.15));
          const zoomSmooth = zoomT * zoomT * (3 - 2 * zoomT);
          this.material.uniforms.viewZoomDetail.value = zoomSmooth;
          const lat = view.latitude * Math.PI / 180;
          const worldLon = -view.longitude * Math.PI / 180;
          const head = view.heading * Math.PI / 180;
          const cosLat = Math.cos(lat), sinLat = Math.sin(lat);
          const cosL = Math.cos(worldLon), sinL = Math.sin(worldLon);
          const fx = cosLat * cosL, fy = sinLat, fz = cosLat * sinL;
          const ex = sinL, ey = 0, ez = -cosL;
          const nx = fy * ez - fz * ey, ny = fz * ex - fx * ez, nz = fx * ey - fy * ex;
          const nLen = Math.hypot(nx, ny, nz) || 1;
          const nnx = nx / nLen, nny = ny / nLen, nnz = nz / nLen;
          const ch = Math.cos(head), sh = Math.sin(head);
          const fwdX = nnx * ch + ex * sh, fwdY = nny * ch + ey * sh, fwdZ = nnz * ch + ez * sh;
          const rgtX = ex * ch - nnx * sh, rgtY = ey * ch - nny * sh, rgtZ = ez * ch - nnz * sh;
          // Screen upper-left oblique azimuth, elevation 48° at orbit -> 32° at close-up.
          const sunElev = (48 - 16 * zoomSmooth) * Math.PI / 180;
          const sinE = Math.sin(sunElev), cosE = Math.cos(sunElev);
          const azX = -0.68 * rgtX + 0.73 * fwdX;
          const azY = -0.68 * rgtY + 0.73 * fwdY;
          const azZ = -0.68 * rgtZ + 0.73 * fwdZ;
          this.activeSun.set(fx * sinE + azX * cosE, fy * sinE + azY * cosE, fz * sinE + azZ * cosE).normalize()
            .lerp(this.worldSun, nightBlend).normalize();
          this.space.updateView(this.camera, distance, nightBlend, this.worldSun, this.activeSun);
        } else {
          this.material.uniforms.viewZoomDetail.value = 0;
        }

        // Hide tiny coastal stubs at planet scale on Land and water; fade them in smoothly when zoomed in.
        const detailZoom = this.layer === 'riverDischarge'
          ? 1
          : this.mode === 'flat'
            ? Math.max(0, Math.min(1, (this.flatCamera.zoom - 1.7) / 1.5))
            : Math.max(0, Math.min(1, (1.42 - distance) / 0.62));
        const naturalSurface = this.layer === 'surface' && this.surfaceStyle === 'natural';
        const riverOpacity = this.layer === 'travelTime' ? .28 : naturalSurface ? .84 : 1;
        for (const [i, material] of this.riverMaterials.entries()) {
          const bin = i - DRAINAGE_LINE_WIDTHS.length;
          const seasonal = bin >= RIVER_LINE_WIDTHS.length;
          const strength = bin < 0 ? .7 : Math.min(1, (seasonal ? .33 : .55) + .19 * (bin % RIVER_LINE_WIDTHS.length));
          const op = strength * riverOpacity * (1 - nightBlend * .75);
          material.opacity = op;
          this.riverDetailMaterials[i].opacity = op * detailZoom;
        }
        if (this.views) {
          const showRivers = this.drainageLines && this.layer !== 'annualRunoff' && (surfaceLike(this.layer) || this.layer === 'elevation');
          this.views[this.mode].children[RIVERS_DETAIL].visible = showRivers && detailZoom > 0.02;
        }
        this.renderer.render(this.scene, this.camera);
      }
    });
  }
  private releaseViews(): void {
    clearTimeout(this.lodTimer); this.lodTimer = 0; this.lodRequestId++;
    if (this.views) for (const group of Object.values(this.views)) {
      this.scene.remove(group);
      group.traverse((object: Object3D) => { if ('geometry' in object) (object as Mesh<BufferGeometry>).geometry.dispose(); });
    }
    this.texture?.dispose(); this.texture = null; this.cornerTexture?.dispose(); this.cornerTexture = null;
    this.tintTexture?.dispose(); this.tintTexture = null; this.cornerTintTexture?.dispose(); this.cornerTintTexture = null;
    this.detailTexture?.dispose(); this.detailTexture = null; this.cornerDetailTexture?.dispose(); this.cornerDetailTexture = null;
    this.lodMaskTexture?.dispose(); this.lodMaskTexture = null;
    this.corners = null; this.compactRing = null; this.views = null; this.lodTriangles = 0;
  }
  dispose(): void {
    this.cancelPreparation(); cancelAnimationFrame(this.frame); this.stopDamping(); this.resizeObserver.disconnect();
    this.clearMapDrag();
    this.canvas.removeEventListener('pointerdown', this.beginMapDrag);
    window.removeEventListener('pointerup', this.endMapDrag);
    window.removeEventListener('pointercancel', this.endMapDrag);
    window.removeEventListener('blur', this.clearMapDrag);
    this.controls.removeEventListener('change', this.handleControlsChange);
    this.controls.dispose(); this.planetCamera.dispose(); this.space.dispose(); this.releaseViews(); this.material.dispose(); this.waterMaterial.dispose(); this.lodMaterial.dispose(); this.lineMaterial.dispose(); for (const material of this.riverMaterials) material.dispose(); for (const material of this.riverDetailMaterials) material.dispose(); this.routeCasingMaterial.dispose(); this.routeLineMaterial.dispose(); this.originMaterial.dispose(); this.tectonicMaterial.dispose(); this.renderer.dispose();
  }
}
