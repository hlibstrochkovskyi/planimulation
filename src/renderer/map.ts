import { BufferAttribute, BufferGeometry, DataTexture, DoubleSide, FloatType, Group, LineBasicMaterial,
  LineSegments, Mesh, MOUSE, NearestFilter, OrthographicCamera, PerspectiveCamera, Raycaster, RedFormat,
  Scene, ShaderMaterial, Vector2, WebGLRenderer } from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import type { World } from '../core/world';
import { speedCmPerYear } from '../core/tectonics';
import type { ViewGeometry, ViewPair } from './view-geometry';
import { displaceDirections, effectiveExaggeration } from './relief';
import { summarizeTerrain } from '../core/terrain';
import { pickSurface } from './water-surface';

export type Layer = 'surface' | 'signal' | 'area' | 'latitude' | 'plates' | 'boundaries' | 'speed' | 'crust' | 'thickness' | 'elevation' | 'uplift' | 'depth' | 'waterBodies';
export type ViewMode = 'flat' | 'globe';

export class SurfaceMap {
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  private readonly flatCamera = new OrthographicCamera(-1.2, 1.2, .6, -.6, .01, 100);
  private readonly globeCamera = new PerspectiveCamera(42, 1, .01, 100);
  private readonly controls: OrbitControls;
  private readonly resizeObserver: ResizeObserver;
  private readonly raycaster = new Raycaster();
  private readonly material = new ShaderMaterial({
    side: DoubleSide,
    uniforms: { field: { value: null }, textureWidth: { value: 1 }, textureHeight: { value: 1 },
      selected: { value: -1 }, globe: { value: 0 }, categorical: { value: 0 }, muted: { value: 0 }, waterMode: { value: 0 },
      surfaceMode: { value: 0 }, waterSurface: { value: 0 } },
    vertexShader: `attribute float region; varying float cell; varying vec3 direction;
      void main() { cell=region; direction=normalMatrix*normal;
        gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }`,
    fragmentShader: `uniform sampler2D field; uniform float textureWidth; uniform float textureHeight;
      uniform float selected; uniform float globe; uniform float categorical; uniform float muted; uniform float waterMode;
      uniform float surfaceMode; uniform float waterSurface;
      varying float cell; varying vec3 direction;
      void main() {
        float id=floor(cell+0.5);
        vec2 uv=vec2((mod(id,textureWidth)+0.5)/textureWidth,(floor(id/textureWidth)+0.5)/textureHeight);
        float v=texture2D(field,uv).r;
        vec3 color=mix(vec3(0.16,0.28,0.33),vec3(0.73,0.80,0.63),clamp(v,0.0,1.0));
        if(categorical>0.5) color=0.51+0.23*cos(6.2831853*(v*0.618033989+vec3(0.0,0.33,0.67)));
        if(waterMode>0.5 && waterMode<1.5) color=v<0.0 ? vec3(0.34,0.36,0.30) : mix(vec3(0.18,0.62,0.75),vec3(0.025,0.10,0.26),clamp(v,0.0,1.0));
        if(waterMode>1.5 && v<0.5) color=vec3(0.34,0.36,0.30);
        if(surfaceMode>0.5) {
          color=mix(vec3(0.34,0.40,0.28),vec3(0.72,0.69,0.61),clamp(v,0.0,1.0));
          if(waterSurface>0.5 || (globe<0.5 && v<0.0)) color=mix(vec3(0.18,0.62,0.75),vec3(0.025,0.10,0.26),clamp(-v,0.0,1.0));
        }
        color*=1.0-muted*0.65;
        if(abs(cell-selected)<0.25) color=vec3(0.96,0.78,0.42);
        if(globe>0.5) color*=0.4+0.6*max(0.0,dot(normalize(direction),normalize(vec3(-0.4,0.6,1.0))));
        gl_FragColor=vec4(color,1.0);
      }`,
  });
  private readonly waterMaterial = this.material.clone();
  private readonly lineMaterial = new LineBasicMaterial({ color: '#304849', transparent: true, opacity: .45 });
  private readonly tectonicMaterial = new LineBasicMaterial({ vertexColors: true });
  private views: Record<ViewMode, Group> | null = null;
  private texture: DataTexture | null = null;
  private values = new Float32Array(0);
  private world: World | null = null;
  private mode: ViewMode = 'flat';
  private layer: Layer = 'surface';
  private exaggeration = 10;
  private appliedExaggeration = NaN;
  private terrainRange = { minimumMeters: 0, maximumMeters: 1 };
  private maximumDepth = 1;
  private boundaries = false;
  private worker: Worker | null = null;
  private abortPreparation: (() => void) | null = null;
  private frame = 0;
  private lost = false;
  private pointerStart = [0, 0];

  constructor(private readonly canvas: HTMLCanvasElement, private readonly select: (id: number) => void) {
    // Share field/selection uniforms, but distinguish the raised water mesh.
    this.waterMaterial.uniforms = { ...this.material.uniforms, waterSurface: { value: 1 } };
    this.waterMaterial.polygonOffset = true;
    this.waterMaterial.polygonOffsetFactor = -1; this.waterMaterial.polygonOffsetUnits = -1;
    this.renderer = new WebGLRenderer({ canvas, antialias: true, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
    this.renderer.setClearColor('#0c1417');
    this.flatCamera.position.set(0, 0, 3);
    this.globeCamera.position.set(3, 0, 0);
    this.controls = new OrbitControls(this.flatCamera, canvas);
    this.controls.enableRotate = false;
    this.controls.screenSpacePanning = true;
    this.controls.minZoom = .5; this.controls.maxZoom = 30;
    this.controls.minDistance = 1.3; this.controls.maxDistance = 8;
    this.controls.addEventListener('change', () => this.draw());
    canvas.addEventListener('pointerdown', (e) => { this.pointerStart = [e.clientX, e.clientY]; });
    canvas.addEventListener('pointerup', (e) => {
      if (Math.hypot(e.clientX - this.pointerStart[0], e.clientY - this.pointerStart[1]) > 4 || e.button !== 0 || !this.views) return;
      const rect = canvas.getBoundingClientRect();
      this.raycaster.setFromCamera(new Vector2((e.clientX - rect.left) / rect.width * 2 - 1, 1 - (e.clientY - rect.top) / rect.height * 2), this.camera);
      const view = this.views[this.mode];
      const hit = pickSurface(this.raycaster, view.children[0] as Mesh<BufferGeometry>, view.children[3] as Mesh<BufferGeometry>);
      if (!hit) return;
      const id = hit.id; this.canvas.dataset.pickedSurface = hit.surface;
      this.material.uniforms.selected.value = id; this.select(id); this.draw();
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
    const aspect = width / height, halfHeight = Math.max(.6, 1.1 / aspect);
    Object.assign(this.flatCamera, { left: -halfHeight * aspect, right: halfHeight * aspect, top: halfHeight, bottom: -halfHeight });
    this.flatCamera.updateProjectionMatrix();
    this.globeCamera.aspect = aspect; this.globeCamera.updateProjectionMatrix(); this.draw();
  }
  private makeView(data: ViewGeometry): Group {
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new BufferAttribute(data.positions, 3));
    geometry.setAttribute('region', new BufferAttribute(data.regions, 1));
    geometry.computeVertexNormals();
    const lines = new BufferGeometry(); lines.setAttribute('position', new BufferAttribute(data.lines, 3));
    const tectonicLines = new BufferGeometry();
    tectonicLines.setAttribute('position', new BufferAttribute(data.tectonicLines, 3));
    tectonicLines.setAttribute('color', new BufferAttribute(data.tectonicColors, 3));
    const water = new BufferGeometry();
    water.setAttribute('position', new BufferAttribute(data.waterPositions, 3));
    water.setAttribute('region', new BufferAttribute(data.waterRegions, 1));
    water.computeVertexNormals();
    const waterLines = new BufferGeometry(); waterLines.setAttribute('position', new BufferAttribute(data.waterLines, 3));
    for (const [g, offsets] of [[geometry, data.radialOffsets], [lines, data.lineOffsets], [tectonicLines, data.tectonicOffsets],
      [water, data.waterOffsets], [waterLines, data.waterLineOffsets]] as const) {
      if (offsets.length) { g.userData.base = (g.getAttribute('position').array as Float32Array).slice(); g.userData.offsets = offsets; }
    }
    const group = new Group();
    group.add(new Mesh(geometry, this.material), new LineSegments(lines, this.lineMaterial), new LineSegments(tectonicLines, this.tectonicMaterial),
      new Mesh(water, this.waterMaterial), new LineSegments(waterLines, this.lineMaterial));
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
    this.releaseViews(); this.views = next;
    this.scene.add(next.flat, next.globe); this.world = world;
    this.appliedExaggeration = NaN;
    this.terrainRange = summarizeTerrain(world.surface, world.terrain);
    this.maximumDepth = world.water.depthMeters.reduce((max, d) => Math.max(max, d), 0);
    this.setExaggeration(this.exaggeration);
    const width = Math.min(1024, this.renderer.capabilities.maxTextureSize);
    const height = Math.ceil(world.stats.regionCount / width);
    this.values = new Float32Array(width * height);
    this.texture = new DataTexture(this.values, width, height, RedFormat, FloatType);
    this.texture.minFilter = this.texture.magFilter = NearestFilter;
    this.material.uniforms.field.value = this.texture;
    this.material.uniforms.textureWidth.value = width; this.material.uniforms.textureHeight.value = height;
    this.material.uniforms.selected.value = -1;
    delete this.canvas.dataset.pickedSurface;
    this.setLayer(this.layer); this.setMode(this.mode); this.setBoundaries(this.boundaries);
  }
  setMode(mode: ViewMode): void {
    this.mode = mode; this.canvas.dataset.view = mode;
    this.material.uniforms.globe.value = mode === 'globe' ? 1 : 0;
    if (this.views) { this.views.flat.visible = mode === 'flat'; this.views.globe.visible = mode === 'globe'; }
    this.controls.object = this.camera; this.controls.enableRotate = mode === 'globe';
    this.controls.enablePan = mode === 'flat';
    this.controls.mouseButtons.LEFT = mode === 'flat' ? MOUSE.PAN : MOUSE.ROTATE;
    this.reset();
  }
  setLayer(layer: Layer): void {
    this.layer = layer;
    this.canvas.dataset.activeLayer = layer;
    this.material.uniforms.surfaceMode.value = layer === 'surface' ? 1 : 0;
    this.material.uniforms.categorical.value = layer === 'plates' || layer === 'boundaries' || layer === 'waterBodies' ? 1 : 0;
    this.material.uniforms.waterMode.value = layer === 'depth' ? 1 : layer === 'waterBodies' ? 2 : 0;
    this.material.uniforms.muted.value = layer === 'boundaries' ? 1 : 0;
    if (this.views) for (const view of Object.values(this.views)) {
      view.children[2].visible = layer === 'plates' || layer === 'boundaries';
      view.children[3].visible = layer === 'surface';
      view.children[4].visible = layer === 'surface' && this.boundaries;
    }
    this.refreshField();
  }
  refreshField(): void {
    if (!this.world || !this.texture) return;
    const w = this.world;
    for (let id = 0; id < w.stats.regionCount; id++) {
      this.values[id] = this.layer === 'plates' || this.layer === 'boundaries' ? w.tectonics.owners[id]
        : this.layer === 'surface' ? (w.water.bodyIds[id] ? -Math.max(1e-6, w.water.depthMeters[id] / Math.max(1e-30, this.maximumDepth))
          : Math.max(0, w.terrain.elevation[id] - w.water.levelMeters) / Math.max(1, this.terrainRange.maximumMeters - w.water.levelMeters))
        : this.layer === 'depth' ? (w.water.bodyIds[id] ? w.water.depthMeters[id] / Math.max(1e-30, this.maximumDepth) : -1)
        : this.layer === 'waterBodies' ? w.water.bodyIds[id]
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
    this.texture.needsUpdate = true; this.draw();
  }
  setBoundaries(visible: boolean): void {
    this.boundaries = visible;
    if (this.views) for (const group of Object.values(this.views)) {
      group.children[1].visible = visible; group.children[4].visible = visible && this.layer === 'surface';
    }
    this.draw();
  }
  setExaggeration(requested: number): number {
    const applied = this.world ? effectiveExaggeration(requested, this.world.recipe.radiusMeters, this.world.terrain.elevation,
      this.world.water.mainOceanId ? this.world.water.levelMeters : undefined) : requested;
    this.exaggeration = requested;
    if (this.appliedExaggeration === applied) return applied;
    this.appliedExaggeration = applied;
    if (this.views) for (const object of this.views.globe.children) {
      const geometry = (object as Mesh<BufferGeometry>).geometry;
      const { base, offsets } = geometry.userData as { base?: Float32Array; offsets?: Float32Array };
      if (!base || !offsets) continue;
      const attribute = geometry.getAttribute('position') as BufferAttribute;
      displaceDirections(base, offsets, applied, attribute.array as Float32Array); attribute.needsUpdate = true;
      if (object instanceof Mesh) geometry.computeVertexNormals();
      geometry.computeBoundingSphere(); geometry.computeBoundingBox();
    }
    this.canvas.dataset.exaggeration = String(applied); this.draw(); return applied;
  }
  reset(): void {
    this.controls.target.set(0, 0, 0);
    if (this.mode === 'flat') { this.flatCamera.position.set(0, 0, 3); this.flatCamera.zoom = 1; this.flatCamera.updateProjectionMatrix(); }
    else this.globeCamera.position.set(3, 0, 0);
    this.controls.update(); this.draw();
  }
  zoomBy(factor: number): void {
    if (this.mode === 'flat') { this.flatCamera.zoom = Math.max(.5, Math.min(30, this.flatCamera.zoom * factor)); this.flatCamera.updateProjectionMatrix(); }
    else this.globeCamera.position.multiplyScalar(Math.max(1.3, Math.min(8, this.globeCamera.position.length() / factor)) / this.globeCamera.position.length());
    this.controls.update(); this.draw();
  }
  private draw(): void {
    if (this.frame || this.lost) return;
    this.frame = requestAnimationFrame(() => { this.frame = 0; if (!this.lost) this.renderer.render(this.scene, this.camera); });
  }
  private releaseViews(): void {
    if (this.views) for (const group of Object.values(this.views)) {
      this.scene.remove(group);
      for (const object of group.children) (object as Mesh<BufferGeometry>).geometry.dispose();
    }
    this.texture?.dispose(); this.texture = null; this.views = null;
  }
  dispose(): void {
    this.cancelPreparation(); cancelAnimationFrame(this.frame); this.resizeObserver.disconnect();
    this.controls.dispose(); this.releaseViews(); this.material.dispose(); this.waterMaterial.dispose(); this.lineMaterial.dispose(); this.tectonicMaterial.dispose(); this.renderer.dispose();
  }
}
