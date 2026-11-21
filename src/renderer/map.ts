import { BufferAttribute, BufferGeometry, DataTexture, DoubleSide, FloatType, Group, LineBasicMaterial,
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
import type { ViewCorners, ViewGeometry, ViewPair } from './view-geometry';
import { relaxedMask, reliefGradients } from './shading';
import { buildDrainageLines, DRAINAGE_LINE_WIDTHS } from './rivers';
import { displaceDirections, effectiveExaggeration } from './relief';
import { summarizeTerrain } from '../core/terrain';
import { buildWaterSurface, pickSurface } from './water-surface';
import { summarizeBasins } from '../core/basins';
import type { TemperatureNormals } from '../core/seasonal-temperature';
import { BIOMES } from '../core/climate-normals';
import type { ClimateNormals } from '../core/climate-normals';
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

export type Layer = 'surface' | 'signal' | 'area' | 'latitude' | 'plates' | 'boundaries' | 'speed' | 'crust' | 'thickness' | 'elevation' | 'uplift' | 'depth' | 'waterBodies' | 'catchments' | 'contributingArea' | 'basins' | 'spill' | 'temperature' | 'windSpeed' | 'biome' | 'annualPrecipitation' | 'vegetationState' | 'woodyCover' | 'woodyChange' | 'productivity' | 'precipitationAnomaly' | 'foragerCapacity' | 'leanSeason' | 'gameStock' | 'travelCost' | 'travelTime' | MoistureLayer;
export type ViewMode = 'flat' | 'globe';
/** Continuous layers can be interpolated between regions; categorical and sentinel layers cannot. */
const SMOOTH_LAYERS = new Set<string>(['surface', 'signal', 'area', 'latitude', 'crust', 'thickness', 'elevation', 'uplift',
  'depth', 'contributingArea', 'temperature', 'windSpeed', 'biome', 'annualPrecipitation', 'vegetationState', 'woodyCover', 'woodyChange', 'productivity', 'precipitationAnomaly', 'foragerCapacity', 'leanSeason', 'gameStock', 'travelCost', 'travelTime']);
const RELIEF_LAYERS: Partial<Record<string, number>> = { surface: 1, biome: 1, vegetationState: 1, woodyCover: 1, woodyChange: 1, productivity: 1, precipitationAnomaly: 1, foragerCapacity: 1, leanSeason: 1, gameStock: 1, travelCost: .85, travelTime: .3, elevation: .8 };
/** Land layers colored by a per-region tint over relief, with bathymetric water. */
const TINT_LAYERS = new Set<string>(['biome', 'vegetationState', 'woodyCover', 'woodyChange', 'productivity', 'precipitationAnomaly',
  'foragerCapacity', 'leanSeason', 'gameStock', 'travelCost', 'travelTime']);
/** Layers drawn as land and water with a shoreline, bathymetry and the globe water surface. */
const isMovement = (layer: Layer): boolean => layer === 'travelCost' || layer === 'travelTime';
const surfaceLike = (layer: string): boolean => layer === 'surface' || TINT_LAYERS.has(layer);
type Rgb = readonly [number, number, number];
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
  private distantLighting = true;
  private readonly material = new ShaderMaterial({
    side: DoubleSide,
    uniforms: { field: { value: null }, textureWidth: { value: 1 }, textureHeight: { value: 1 },
      cornerField: { value: null }, cornerWidth: { value: 1 }, cornerHeight: { value: 1 },
      tintField: { value: null }, cornerTintField: { value: null }, biomeMode: { value: 0 }, precipitationMode: { value: 0 },
      smoothMode: { value: 0 }, reliefStrength: { value: 0 },
      selected: { value: -1 }, globe: { value: 0 }, categorical: { value: 0 }, muted: { value: 0 }, waterMode: { value: 0 },
      surfaceMode: { value: 0 }, waterSurface: { value: 0 }, missingMode: { value: 0 },
      basinMode: { value: 0 }, spillFrom: { value: -1 }, spillTo: { value: -1 }, temperatureMode: { value: 0 }, windMode: { value: 0 },
      travelTimeMode: { value: 0 }, movementMode: { value: 0 },
      isochroneLevels: { value: ISOCHRONE_DAYS.map(travelTimeCoordinate) },
      distantLightMix: { value: 0 }, sunDirection: { value: this.worldSun } },
    // Field texels hold (value, wet mask, east slope, north slope) for regions and for
    // shared dual corners. Vertices blend a region center with two corners.
    vertexShader: `attribute float region; attribute vec2 cornerPair; attribute vec3 weight;
      uniform sampler2D field; uniform float textureWidth; uniform float textureHeight;
      uniform sampler2D cornerField; uniform float cornerWidth; uniform float cornerHeight;
      uniform sampler2D tintField; uniform sampler2D cornerTintField;
      varying float cell; varying vec4 blended; varying vec4 tint; varying vec3 direction; varying vec3 planetDirection;
      vec4 texel(sampler2D map, float id, float width, float height) {
        return texture2D(map, vec2((mod(id,width)+0.5)/width,(floor(id/width)+0.5)/height));
      }
      void main() { cell=region;
        blended=weight.x*texel(field,region,textureWidth,textureHeight)
          +weight.y*texel(cornerField,cornerPair.x,cornerWidth,cornerHeight)
          +weight.z*texel(cornerField,cornerPair.y,cornerWidth,cornerHeight);
        tint=weight.x*texel(tintField,region,textureWidth,textureHeight)
          +weight.y*texel(cornerTintField,cornerPair.x,cornerWidth,cornerHeight)
          +weight.z*texel(cornerTintField,cornerPair.y,cornerWidth,cornerHeight);
        direction=normalMatrix*(length(position)>0.0 ? normalize(position) : vec3(0.0,0.0,1.0));
        planetDirection=(modelMatrix*vec4(position,1.0)).xyz;
        gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }`,
    fragmentShader: `uniform sampler2D field; uniform float textureWidth; uniform float textureHeight;
      uniform float smoothMode; uniform float reliefStrength;
      uniform sampler2D tintField; uniform float travelTimeMode; uniform float movementMode;
      uniform float isochroneLevels[${ISOCHRONE_DAYS.length}];
      uniform float selected; uniform float globe; uniform float categorical; uniform float muted; uniform float waterMode;
      uniform float surfaceMode; uniform float waterSurface; uniform float missingMode;
      uniform float basinMode; uniform float spillFrom; uniform float spillTo; uniform float temperatureMode; uniform float windMode;
      uniform float distantLightMix; uniform vec3 sunDirection; uniform float biomeMode; uniform float precipitationMode;
      varying float cell; varying vec4 blended; varying vec4 tint; varying vec3 direction; varying vec3 planetDirection;
      // Atlas-style hypsometric and bathymetric tints in meters. They are not biomes.
      vec3 hypsometric(float h) {
        vec3 c=vec3(0.33,0.47,0.29);
        c=mix(c,vec3(0.48,0.58,0.34),smoothstep(0.0,300.0,h));
        c=mix(c,vec3(0.68,0.67,0.45),smoothstep(300.0,900.0,h));
        c=mix(c,vec3(0.63,0.51,0.36),smoothstep(900.0,1800.0,h));
        c=mix(c,vec3(0.53,0.45,0.39),smoothstep(1800.0,3000.0,h));
        return mix(c,vec3(0.88,0.88,0.88),smoothstep(3200.0,5200.0,h));
      }
      vec3 bathymetric(float d) {
        vec3 c=vec3(0.38,0.68,0.78);
        c=mix(c,vec3(0.18,0.46,0.64),smoothstep(0.0,250.0,d));
        c=mix(c,vec3(0.09,0.29,0.51),smoothstep(250.0,1500.0,d));
        c=mix(c,vec3(0.04,0.15,0.35),smoothstep(1500.0,4500.0,d));
        return mix(c,vec3(0.02,0.08,0.22),smoothstep(4500.0,8000.0,d));
      }
      void main() {
        float id=floor(cell+0.5);
        vec2 uv=vec2((mod(id,textureWidth)+0.5)/textureWidth,(floor(id/textureWidth)+0.5)/textureHeight);
        vec4 own=texture2D(field,uv);
        vec4 sampled=smoothMode>0.5 ? blended : own;
        float v=sampled.r, m=sampled.g;
        bool wet=false;
        vec3 color=mix(vec3(0.16,0.28,0.33),vec3(0.73,0.80,0.63),clamp(v,0.0,1.0));
        if(categorical>0.5) color=0.51+0.23*cos(6.2831853*(v*0.618033989+vec3(0.0,0.33,0.67)));
        if(waterMode>0.5 && waterMode<1.5) color=m>0.0 ? vec3(0.34,0.36,0.30) : mix(vec3(0.18,0.62,0.75),vec3(0.025,0.10,0.26),clamp(v,0.0,1.0));
        if(waterMode>1.5 && v<0.5) color=vec3(0.34,0.36,0.30);
        if(missingMode>0.5 && v<0.0) color=vec3(0.34,0.36,0.30);
        if(surfaceMode>0.5) {
          // Interpolated masks give a display shoreline midway between wet and dry centers.
          bool frozenCap=movementMode>0.5 && waterSurface>0.5 && own.g>0.0;
          if(waterSurface>0.5 && !frozenCap && (smoothMode>0.5 || movementMode>0.5) && m>0.0) discard;
          wet=!frozenCap && (waterSurface>0.5 || m<0.0);
          color=wet ? bathymetric(max(-v,0.0)) : biomeMode>0.5 ? tint.rgb : hypsometric(max(v,0.0));
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
        if(reliefStrength>0.0) {
          vec3 n=normalize(vec3(-blended.b,-blended.a,1.0)), light=normalize(vec3(-0.55,0.65,0.75));
          float shade=clamp(dot(n,light)/light.z,0.3,1.45);
          color*=mix(1.0,shade,reliefStrength*(wet ? 0.35 : 1.0));
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
          vec3 normal=normalize(direction), light=normalize(vec3(-0.4,0.6,1.0));
          vec3 survey=color*(0.4+0.6*max(0.0,dot(normal,light)));
          float solar=dot(normalize(planetDirection),sunDirection);
          float day=smoothstep(-0.035,0.065,solar);
          vec3 daylight=mix(vec3(1.0,0.78,0.58),vec3(1.0),smoothstep(0.0,0.25,solar))*(0.16+0.84*max(0.0,solar));
          vec3 orbital=color*mix(vec3(0.012,0.018,0.03),daylight,day);
          color=mix(survey,orbital,distantLightMix);
        }
        gl_FragColor=vec4(color,1.0);
      }`,
  });
  private readonly waterMaterial = this.material.clone();
  private readonly lineMaterial = new LineBasicMaterial({ color: '#304849', transparent: true, opacity: .45 });
  private readonly tectonicMaterial = new LineBasicMaterial({ vertexColors: true });
  private views: Record<ViewMode, Group> | null = null;
  private texture: DataTexture | null = null;
  private cornerTexture: DataTexture | null = null;
  private values = new Float32Array(0);
  private cornerValues = new Float32Array(0);
  private corners: ViewCorners | null = null;
  private smoothShading = true;
  private reliefShading = true;
  private drainageLines = true;
  private readonly riverMaterials = DRAINAGE_LINE_WIDTHS.map(linewidth => new LineMaterial({ color: 0x4d9cc9, linewidth, transparent: true }));
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
  private exaggeration = 10;
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
    // Share field/selection uniforms, but distinguish the raised water mesh.
    this.waterMaterial.uniforms = { ...this.material.uniforms, waterSurface: { value: 1 } };
    this.waterMaterial.polygonOffset = true;
    this.waterMaterial.polygonOffsetFactor = -1; this.waterMaterial.polygonOffsetUnits = -1;
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
    this.space = new SpaceBackdrop(this.renderer.getPixelRatio(), ready => {
      canvas.dataset.sky = ready ? 'ready' : 'fallback'; this.notifyNavigation(); this.draw();
    });
    this.scene.add(this.space);
    this.planetCamera = new PlanetCamera(this.globeCamera, canvas, () => { this.draw(); this.notifyNavigation(); });
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
    const lines = new BufferGeometry(); lines.setAttribute('position', new BufferAttribute(data.lines, 3));
    const tectonicLines = new BufferGeometry();
    tectonicLines.setAttribute('position', new BufferAttribute(data.tectonicLines, 3));
    tectonicLines.setAttribute('color', new BufferAttribute(data.tectonicColors, 3));
    const water = new BufferGeometry();
    water.setAttribute('position', new BufferAttribute(data.waterPositions, 3));
    water.setAttribute('region', new BufferAttribute(data.waterRegions, 1));
    water.setAttribute('cornerPair', new BufferAttribute(data.waterCornerPairs, 2));
    water.setAttribute('weight', new BufferAttribute(data.waterWeights, 3));
    const waterLines = new BufferGeometry(); waterLines.setAttribute('position', new BufferAttribute(data.waterLines, 3));
    for (const [g, offsets] of [[geometry, data.radialOffsets], [lines, data.lineOffsets], [tectonicLines, data.tectonicOffsets],
      [water, data.waterOffsets], [waterLines, data.waterLineOffsets]] as const) {
      if (offsets.length) { g.userData.base = (g.getAttribute('position').array as Float32Array).slice(); g.userData.offsets = offsets; }
    }
    const group = new Group();
    group.add(new Mesh(geometry, this.material), new LineSegments(lines, this.lineMaterial), new LineSegments(tectonicLines, this.tectonicMaterial),
      new Mesh(water, this.waterMaterial), new LineSegments(waterLines, this.lineMaterial), new Group(), new Group());
    return group;
  }
  cancelPreparation(): void {
    this.worker?.terminate(); this.worker = null; this.abortPreparation?.(); this.abortPreparation = null;
  }
  async setWorld(world: World): Promise<void> {
    this.cancelPreparation();
    const worker = new Worker(new URL('./geometry.worker.ts', import.meta.url), { type: 'module' });
    this.worker = worker;
    const pair = await new Promise<ViewPair>((resolve, reject) => {
      this.abortPreparation = () => reject(new Error('View preparation canceled.'));
      worker.onerror = (e) => reject(new Error(e.message));
      worker.onmessage = (e: MessageEvent<{ pair: ViewPair; error?: string }>) => {
        if (e.data.error) reject(new Error(e.data.error)); else resolve(e.data.pair);
      };
      worker.postMessage({ surface: world.surface, tectonics: world.tectonics, elevation: world.terrain.elevation, water: world.water });
    }).finally(() => { worker.terminate(); if (this.worker === worker) { this.worker = null; this.abortPreparation = null; } });
    const next = { flat: this.makeView(pair.flat), globe: this.makeView(pair.globe) };
    this.addDrainageLines(next, world);
    // Model longitude increases toward +Z. Reflect the display embedding so east
    // appears on the right in a north-up globe, matching the flat projection.
    next.globe.scale.z = -1;
    this.releaseViews(); this.views = next;
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
    this.setExaggeration(this.exaggeration);
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
    const { gradients } = reliefGradients(world.surface, world.terrain.elevation, world.water.bodyIds, world.water.levelMeters);
    for (let id = 0; id < world.stats.regionCount; id++) this.values.set(gradients.subarray(id * 2, id * 2 + 2), id * 4 + 2);
    this.blendCorners(2);
    const uniforms = this.material.uniforms;
    uniforms.field.value = this.texture; uniforms.cornerField.value = this.cornerTexture;
    uniforms.tintField.value = this.tintTexture; uniforms.cornerTintField.value = this.cornerTintTexture;
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
  private updateWaterSurface(frame: Water | WaterFrame | RegionalWaterDisplay): void {
    if (!this.world || !this.views) throw new Error('Water display requires a prepared world.');
    const globe = this.views.globe;
    const bed = (globe.children[0] as Mesh<BufferGeometry>).geometry;
    const base = bed.userData.base as Float32Array;
    const regions = bed.getAttribute('region').array as Float32Array;
    const data = buildWaterSurface(base, regions, frame, this.world.recipe.radiusMeters, {
      cornerPairs: bed.getAttribute('cornerPair').array as Float32Array, weights: bed.getAttribute('weight').array as Float32Array });
    const water = new BufferGeometry();
    water.setAttribute('position', new BufferAttribute(data.waterPositions, 3));
    water.setAttribute('region', new BufferAttribute(data.waterRegions, 1));
    water.setAttribute('cornerPair', new BufferAttribute(data.waterCornerPairs, 2));
    water.setAttribute('weight', new BufferAttribute(data.waterWeights, 3));
    water.userData.base = data.waterPositions.slice(); water.userData.offsets = data.waterOffsets;
    const lines = new BufferGeometry();
    lines.setAttribute('position', new BufferAttribute(data.waterLines, 3));
    lines.userData.base = data.waterLines.slice(); lines.userData.offsets = data.waterLineOffsets;
    const oldWater = globe.children[3] as Mesh<BufferGeometry>;
    const oldLines = globe.children[4] as LineSegments<BufferGeometry>;
    oldWater.geometry.dispose(); oldLines.geometry.dispose();
    oldWater.geometry = water; oldLines.geometry = lines;
    this.appliedExaggeration = NaN;
    this.setExaggeration(this.exaggeration);
    this.globeWater = frame;
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
    this.refreshField();
  }
  /** Climate classes for the biome layer. */
  setClimateNormals(normals: ClimateNormals | null): void {
    if (normals && (!this.world || normals.biome.length !== this.world.stats.regionCount)) {
      throw new Error('Climate normals do not match this world.');
    }
    this.climateNormals = normals;
    this.applyTint();
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
        marker.renderOrder = 3;
        this.views[mode].children[ROUTE].add(marker);
      }
    }
    if (path.length >= 2) {
      const overlay = buildRouteOverlay(this.world.surface, this.world.terrain.elevation, this.world.water.bodyIds, this.world.water.levelMeters, path);
      if (overlay.flat.length) {
        const casing = new LineSegments2(new LineSegmentsGeometry().setPositions(overlay.flat), this.routeCasingMaterial);
        const core = new LineSegments2(new LineSegmentsGeometry().setPositions(overlay.flat), this.routeLineMaterial);
        casing.renderOrder = 1; core.renderOrder = 2;
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
          line.renderOrder = index + 1;
          this.views!.globe.children[ROUTE].add(line);
        });
      }
    }
    this.applyShadingModes();
    this.draw();
  }
  /** Per-region land colors of the active tint layer; shared corners average only dry regions. */
  private applyTint(): void {
    const climate = this.climateNormals, vegetation = this.vegetation, food = this.wildFood, costs = this.traversalCosts, field = this.travelField, layer = this.layer;
    const costField = costs ? (this.travelSeason === 'summer' ? costs.hoursPer100KmSummer : costs.hoursPer100KmWinter) : null;
    const timeDays = layer === 'travelTime' ? field?.days : undefined;
    this.material.uniforms.travelTimeMode.value = timeDays ? 1 : 0;
    this.material.uniforms.movementMode.value = isMovement(layer) ? 1 : 0;
    const color: ((id: number) => Rgb) | null = layer === 'biome' && climate ? (id) => BIOMES[climate.biome[id]].color
      : layer === 'vegetationState' && vegetation ? (id) => BIOMES[vegetation.label[id]].color
      : layer === 'woodyCover' && vegetation ? (id) => woodyColor(vegetation.woody[id])
      : layer === 'woodyChange' && vegetation ? (id) => changeColor(vegetation.woodyChange[id])
      : layer === 'productivity' && vegetation ? (id) => productivityColor(vegetation.productivity[id])
      : layer === 'precipitationAnomaly' && vegetation ? (id) => anomalyColor(vegetation.precipitationFactor[id])
      : layer === 'foragerCapacity' && food ? (id) => capacityColor(food.capacity[id])
      : layer === 'leanSeason' && food ? (id) => food.storedCapacity[id] > 0 ? leanColor(food.capacity[id] / food.storedCapacity[id]) : [206, 202, 192]
      : layer === 'gameStock' && food ? (id) => gameColor(food.game[id], food.gameCapacity[id])
      : layer === 'travelCost' && costField ? (id) => travelCostColor(costField[id])
      : timeDays ? (id) => travelTimeColor(timeDays[id]) : null;
    this.material.uniforms.biomeMode.value = color ? 1 : 0;
    if (color && this.world && this.corners && this.tintTexture && this.cornerTintTexture) {
      const tint = this.tintValues, out = this.cornerTintValues, surface = this.world.surface;
      const traversalLayer = isMovement(layer) && costs !== null;
      const wet = (id: number): boolean => traversalLayer ? !isRegionPassable(costs, id, this.travelSeason) : this.world!.water.bodyIds[id] !== 0;
      const count = this.world.stats.regionCount;
      for (let id = 0; id < count; id++) {
        const rgb = color(id);
        for (let c = 0; c < 3; c++) tint[id * 4 + c] = rgb[c] / 255;
        const days = timeDays?.[id] ?? -1;
        tint[id * 4 + 3] = days < 0 ? -1 : travelTimeCoordinate(days);
      }
      if (layer === 'biome' || layer === 'vegetationState') {
        // Display-only softening: each dry region mixes half of its dry neighbors' mean
        // color so 110 km region edges read as transitions. Classes are unchanged.
        const own = tint.slice();
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
    this.refreshField();
  }
  setMode(mode: ViewMode): void {
    this.stopCamera();
    this.clearMapDrag();
    this.mode = mode; this.canvas.dataset.view = mode;
    this.syncWaterSurface();
    this.material.uniforms.globe.value = mode === 'globe' ? 1 : 0;
    this.space.visible = mode === 'globe';
    this.renderer.setClearColor(mode === 'globe' ? '#040507' : '#0c1721');
    if (this.views) { this.views.flat.visible = mode === 'flat'; this.views.globe.visible = mode === 'globe'; }
    this.controls.enabled = mode === 'flat';
    this.controls.mouseButtons.LEFT = MOUSE.PAN;
    this.planetCamera.setEnabled(mode === 'globe');
    this.notifyNavigation(); this.draw();
  }
  setLayer(layer: Layer): void {
    this.layer = layer;
    this.space.setSurfaceMode(surfaceLike(layer));
    this.syncWaterSurface();
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
  }
  refreshField(): void {
    if (!this.world || !this.texture) return;
    const w = this.world;
    const staticWater = isMovement(this.layer);
    const depth = staticWater ? w.water.depthMeters : this.regionalWater?.depthMeters ?? this.waterFrame?.depthMeters ?? w.water.depthMeters;
    const bodyIds = staticWater ? w.water.bodyIds : this.waterFrame?.bodyIds ?? w.water.bodyIds;
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
    this.material.uniforms.reliefStrength.value = this.reliefShading ? RELIEF_LAYERS[this.layer] ?? 0 : 0;
    this.canvas.dataset.smoothShading = String(smooth);
    if (this.views) for (const view of Object.values(this.views)) {
      view.children[RIVERS].visible = this.drainageLines && (surfaceLike(this.layer) || this.layer === 'elevation');
      view.children[ROUTE].visible = this.layer === 'travelTime';
    }
  }
  setSmoothShading(enabled: boolean): void { this.smoothShading = enabled; this.applyShadingModes(); this.refreshField(); }
  setReliefShading(enabled: boolean): void { this.reliefShading = enabled; this.applyShadingModes(); this.draw(); }
  setDrainageLines(visible: boolean): void { this.drainageLines = visible; this.applyShadingModes(); this.draw(); }
  private addDrainageLines(views: Record<ViewMode, Group>, world: World): void {
    const lines = buildDrainageLines(world.surface, world.drainage, world.terrain.elevation, world.water.bodyIds);
    this.canvas.dataset.drainageLineSegments = String(lines.flat.reduce((sum, v) => sum + v.length / 6, 0));
    lines.flat.forEach((positions, width) => {
      if (!positions.length) return;
      const geometry = new LineSegmentsGeometry().setPositions(positions);
      views.flat.children[RIVERS].add(new LineSegments2(geometry, this.riverMaterials[width]));
    });
    lines.globe.forEach(({ positions, offsets }, width) => {
      if (!positions.length) return;
      const geometry = new LineSegmentsGeometry().setPositions(positions);
      geometry.userData.riverBase = positions; geometry.userData.riverOffsets = offsets;
      views.globe.children[RIVERS].add(new LineSegments2(geometry, this.riverMaterials[width]));
    });
  }
  setBoundaries(visible: boolean): void {
    this.boundaries = visible;
    if (this.views) for (const group of Object.values(this.views)) {
      group.children[1].visible = visible; group.children[4].visible = visible && surfaceLike(this.layer);
    }
    this.draw();
  }
  setBasinContact(from = -1, to = -1): void {
    this.material.uniforms.spillFrom.value = from; this.material.uniforms.spillTo.value = to;
    this.canvas.dataset.spillFrom = String(from); this.canvas.dataset.spillTo = String(to);
    this.draw();
  }
  setExaggeration(requested: number): number {
    const liveWater = isMovement(this.layer) ? null : this.regionalWater ?? this.waterFrame;
    const waterExtent = liveWater
      ? liveWater.surfaceLevelsMeters.reduce((maximum, level) => Math.max(maximum, Math.abs(level)), 0)
      : this.world?.water.mainOceanId ? this.world.water.levelMeters : undefined;
    const applied = this.world ? effectiveExaggeration(requested, this.world.recipe.radiusMeters, this.world.terrain.elevation,
      waterExtent) : requested;
    this.exaggeration = requested;
    if (this.appliedExaggeration === applied) return applied;
    this.appliedExaggeration = applied;
    if (this.views) for (const river of this.views.globe.children[RIVERS].children as LineSegments2[]) {
      const { riverBase, riverOffsets } = river.geometry.userData as { riverBase: Float32Array; riverOffsets: Float32Array };
      river.geometry.setPositions(displaceDirections(riverBase, riverOffsets, applied));
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
    if (this.views) for (const object of this.views.globe.children) {
      if (!(object instanceof Mesh || object instanceof LineSegments)) continue;
      const geometry = (object as Mesh<BufferGeometry>).geometry;
      const { base, offsets } = geometry.userData as { base?: Float32Array; offsets?: Float32Array };
      if (!base || !offsets) continue;
      const attribute = geometry.getAttribute('position') as BufferAttribute;
      displaceDirections(base, offsets, applied, attribute.array as Float32Array); attribute.needsUpdate = true;
      geometry.computeBoundingSphere(); geometry.computeBoundingBox();
    }
    const outerRadius = this.world ? Math.max(1, 1 + applied * Math.max(0, this.terrainRange.maximumMeters, waterExtent ?? 0) / this.world.recipe.radiusMeters) : 1;
    const atmosphericReference = this.world?.water.mainOceanId
      ? 1 + applied * this.world.water.levelMeters / this.world.recipe.radiusMeters : 1;
    this.planetCamera.setRadius(outerRadius); this.space.setRadius(outerRadius, atmosphericReference);
    this.canvas.dataset.exaggeration = String(applied); this.draw(); return applied;
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
        const riverOpacity = this.layer === 'travelTime' ? .25 : .9;
        for (const material of this.riverMaterials) material.opacity = riverOpacity * (1 - nightBlend * .85);
        if (this.mode === 'globe') this.space.updateView(this.camera, distance, nightBlend, this.worldSun);
        this.renderer.render(this.scene, this.camera);
      }
    });
  }
  private releaseViews(): void {
    if (this.views) for (const group of Object.values(this.views)) {
      this.scene.remove(group);
      group.traverse((object: Object3D) => { if ('geometry' in object) (object as Mesh<BufferGeometry>).geometry.dispose(); });
    }
    this.texture?.dispose(); this.texture = null; this.cornerTexture?.dispose(); this.cornerTexture = null;
    this.tintTexture?.dispose(); this.tintTexture = null; this.cornerTintTexture?.dispose(); this.cornerTintTexture = null;
    this.corners = null; this.views = null;
  }
  dispose(): void {
    this.cancelPreparation(); cancelAnimationFrame(this.frame); this.stopDamping(); this.resizeObserver.disconnect();
    this.clearMapDrag();
    this.canvas.removeEventListener('pointerdown', this.beginMapDrag);
    window.removeEventListener('pointerup', this.endMapDrag);
    window.removeEventListener('pointercancel', this.endMapDrag);
    window.removeEventListener('blur', this.clearMapDrag);
    this.controls.removeEventListener('change', this.handleControlsChange);
    this.controls.dispose(); this.planetCamera.dispose(); this.space.dispose(); this.releaseViews(); this.material.dispose(); this.waterMaterial.dispose(); this.lineMaterial.dispose(); for (const material of this.riverMaterials) material.dispose(); this.routeCasingMaterial.dispose(); this.routeLineMaterial.dispose(); this.originMaterial.dispose(); this.tectonicMaterial.dispose(); this.renderer.dispose();
  }
}
