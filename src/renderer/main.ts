import './style.css';
import { DEFAULT_RECIPE, parseRecipe } from '../core/recipe';
import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import type { DesktopAPI } from '../shared/desktop-api';
import { SurfaceMap } from './map';
import type { Layer, ViewMode } from './map';
import { BOUNDARY_NAMES, speedCmPerYear } from '../core/tectonics';
import { summarizeCrust } from '../core/crust';
import { summarizeTerrain } from '../core/terrain';
import { summarizeWater } from '../core/water';
import { summarizeDrainage } from '../core/drainage';

function element<T extends HTMLElement>(id: string): T {
  const result = document.getElementById(id);
  if (!result) throw new Error(`Missing interface element: ${id}`);
  return result as T;
}

const api: DesktopAPI = window.desktop;
const seedInput = element<HTMLInputElement>('seed');
const resolutionInput = element<HTMLSelectElement>('subdivision');
const radiusInput = element<HTMLInputElement>('radius');
const plateCountInput = element<HTMLInputElement>('plate-count');
const plateSpeedInput = element<HTMLInputElement>('plate-speed');
const continentalFractionInput = element<HTMLInputElement>('continental-fraction');
const continentalScaleInput = element<HTMLInputElement>('continental-scale');
const reliefScaleInput = element<HTMLInputElement>('relief-scale');
const boundaryWidthInput = element<HTMLInputElement>('boundary-width');
const detailAmplitudeInput = element<HTMLInputElement>('detail-amplitude');
const waterModeInput = element<HTMLSelectElement>('water-mode');
const waterCoverageInput = element<HTMLInputElement>('water-coverage');
const waterVolumeInput = element<HTMLInputElement>('water-volume');
const status = element('status');
const cancel = element<HTMLButtonElement>('cancel');
const save = element<HTMLButtonElement>('save-recipe');
const number = new Intl.NumberFormat('en', { maximumFractionDigits: 1 });
let world: World | null = null;
let generationId = 0;
let epoch = 0;
let selected: number | null = null;
let playing = false;
let advancing = false;
let playbackTimer = 0;
let currentLayer: Layer = 'surface';
let terrainStats = { minimumMeters: 0, maximumMeters: 0, meanMeters: 0 };
let maximumWaterDepth = 0;
let maximumContributingArea = 0;
let plateAreas = new Float64Array(0);
let boundarySegments = new Map<number, number[]>();

function showStatus(message: string, error = false): void {
  status.textContent = message;
  status.classList.toggle('error', error);
}

function inspect(id: number): void {
  if (!world) return;
  selected = id;
  const s = world.surface, x = s.centers[id * 3], y = s.centers[id * 3 + 1], z = s.centers[id * 3 + 2];
  const lat = Math.asin(y) * 180 / Math.PI, lon = Math.atan2(z, x) * 180 / Math.PI;
  const neighbors = s.neighbors.subarray(s.neighborOffsets[id], s.neighborOffsets[id + 1]);
  const plate = world.tectonics.owners[id];
  element('selection-title').textContent = `Region ${id.toLocaleString('en')}`;
  element('selection-note').textContent = `${Math.abs(lat).toFixed(2)}° ${lat >= 0 ? 'N' : 'S'} · ${Math.abs(lon).toFixed(2)}° ${lon >= 0 ? 'E' : 'W'}`;
  const details = element('selection-details');
  details.replaceChildren();
  for (const [label, value] of [
    ['Reference surface area', `${number.format(s.areasSquareMeters[id] / 1e6)} km²`],
    ['Tectonic plate', `Plate ${plate + 1} · seed region ${world.tectonics.seeds[plate]}`],
    ['Plate area', `${number.format(plateAreas[plate] / 1e12)} M km²`],
    ['Speed (model frame)', `${speedCmPerYear(s, world.tectonics, id).toFixed(3)} cm/year`],
    ['Continentality', `${world.crust.continentality[id].toFixed(4)} · ${world.crust.continentality[id] > 0.5 ? 'continental-dominant' : 'oceanic-dominant'}`],
    ['Crust thickness', `${number.format(world.crust.thicknessMeters[id] / 1000)} km`],
    ['Crust density', `${number.format(world.crust.densityKgPerCubicMeter[id])} kg/m³`],
    ['Elevation (reference datum)', `${number.format(world.terrain.elevation[id])} m`],
    ['Initial water depth', `${number.format(world.water.depthMeters[id])} m`],
    ['Initial water body', world.water.bodyIds[id] === 0 ? 'Dry land' : `Body ${world.water.bodyIds[id]} · ${world.water.bodyIds[id] === world.water.mainOceanId ? 'main ocean' : 'inland basin'}`],
    ['Drainage receiver', world.drainage.receivers[id] === id ? 'Terminal' : `Region ${world.drainage.receivers[id]}`],
    ['Catchment outlet', `Region ${world.drainage.outlets[id]} · ${world.water.bodyIds[world.drainage.outlets[id]] ? `water body ${world.water.bodyIds[world.drainage.outlets[id]]}` : 'closed dry sink'}`],
    ['Contributing land area', `${number.format(world.drainage.contributingArea[id] / 1e6)} km² · not discharge`],
    ['Connected neighbors', String(neighbors.length)],
    ['Reference neighbor distance', `${number.format(s.neighborDistancesMeters.subarray(s.neighborOffsets[id], s.neighborOffsets[id + 1]).reduce((sum, v) => sum + v, 0) / neighbors.length / 1000)} km`],
    ['Diagnostic signal', world.diagnosticField[id].toFixed(5)],
  ]) {
    const dt = document.createElement('dt'), dd = document.createElement('dd');
    dt.textContent = label; dd.textContent = value; details.append(dt, dd);
  }
  const boundaryDetails = element('boundary-details');
  const elevationDetails = element('elevation-details'); elevationDetails.replaceChildren();
  for (const [label, value] of [['Crust baseline', world.terrain.baseline[id]], ['Convergence uplift', world.terrain.convergence[id]],
    ['Divergence (ridge − rift)', world.terrain.divergence[id]], ['Bounded detail', world.terrain.detail[id]], ['Total elevation', world.terrain.elevation[id]]] as const) {
    const row = document.createElement('li'); row.textContent = `${label}: ${value.toFixed(2)} m`; elevationDetails.append(row);
  }
  element('crust-note').textContent = `Seeded spherical potential ${world.crust.potential[id].toFixed(6)}; fitted threshold ${world.crust.threshold.toFixed(6)}; smooth transition width 0.12. Continentality blends the 7–35 km thickness and 3,000–2,800 kg/m³ density endmembers. These are initial model approximations, not elevation or water depth.`;
  element('water-note').textContent = `Initial level ${world.water.levelMeters.toFixed(2)} m − bed ${world.terrain.elevation[id].toFixed(2)} m → depth max(0, level − bed) = ${world.water.depthMeters[id].toFixed(2)} m. Regional stock: ${(world.water.depthMeters[id] * s.areasSquareMeters[id] / 1e9).toFixed(3)} km³ using reference-sphere area. Positive-depth neighbors form water bodies; disconnected bodies share only this initial level, not a permanent connection.`;
  const receiver = world.drainage.receivers[id], steps = world.drainage.flatSteps[id];
  element('drainage-note').textContent = receiver === id
    ? world.water.bodyIds[id] ? 'Existing water is a terminal receiver. Incoming land area is counted here; water-body area itself is excluded. No underwater routing or overflow is inferred.'
      : 'Closed dry sink: no lower exit from this equal-height component. A closed flat uses its smallest region ID as the analysis outlet. The bed is not filled or raised; no lake storage or spill level is computed yet.'
    : steps ? `Equal-height routing: ${steps} graph hops to a downhill exit or closed-flat sink. The receiver has one fewer hop. This deterministic tie-break is not a measured hydraulic gradient and changes no bed heights.`
      : `Steepest bed descent to region ${receiver}: ${(world.terrain.elevation[id] - world.terrain.elevation[receiver]).toFixed(2)} m drop over ${number.format(s.neighborDistancesMeters[s.neighborOffsets[id] + neighbors.indexOf(receiver)] / 1000)} km. Gradient ties prefer the smaller region ID. Area accumulation assumes connectivity only, not rain, travel time, or discharge.`;
  boundaryDetails.replaceChildren();
  const segments = boundarySegments.get(id) ?? [];
  element('boundary-note').textContent = segments.length
    ? 'Actual shared-boundary segments. Positive opening = divergence; negative = convergence. Shear retains its sign.'
    : 'Plate interior: this region has no inter-plate boundary. Nearby boundaries can still contribute to elevation through distance decay.';
  for (const segment of segments) {
    const t = world.tectonics, a = t.boundaryCells[segment * 2], b = t.boundaryCells[segment * 2 + 1];
    const other = t.owners[a === id ? b : a];
    const row = document.createElement('li');
    row.dataset.kind = String(t.boundaryTypes[segment]);
    row.textContent = `Plate ${other + 1} · ${BOUNDARY_NAMES[t.boundaryTypes[segment]]} · opening ${(t.boundaryMotion[segment * 2] * 100).toFixed(3)} cm/year · shear ${(t.boundaryMotion[segment * 2 + 1] * 100).toFixed(3)} cm/year`;
    boundaryDetails.append(row);
  }
}

function createMap(): SurfaceMap {
  try { return new SurfaceMap(element<HTMLCanvasElement>('map'), inspect); }
  catch (error) {
    const message = 'GPU viewer unavailable. This build requires WebGL 2; check graphics support and restart.';
    showStatus(message, true); element('map-empty').textContent = message;
    document.body.dataset.state = 'error';
    element<HTMLButtonElement>('generate').disabled = true;
    throw error;
  }
}
const map = createMap();

function updateLegend(): void {
  const legends: Record<Layer, [string, string, string]> = {
    surface: ['Land and water surface · globe shoreline is a display approximation · colors are not biomes', '', ''],
    signal: ['Seed field · dimensionless diagnostic', '−1', '+1'],
    area: ['Region area · true spherical area', world ? `${number.format(world.stats.minimumAreaSquareMeters / 1e6)} km²` : 'min', world ? `${number.format(world.stats.maximumAreaSquareMeters / 1e6)} km²` : 'max'],
    latitude: ['Latitude · distance from the equator', '90°', '0°'],
    plates: [`${world?.recipe.plateCount ?? '—'} connected plates · colors identify plates, not continents`, '', ''],
    boundaries: ['Boundary motion · dominant component; oblique motion retained in inspector', '', ''],
    speed: ['Plate speed · model reference frame', '0', `${world?.recipe.maxPlateSpeedCmPerYear ?? '—'} cm/year`],
    crust: ['Continentality · initial crust, not land or ocean coverage', '0 · oceanic', '1 · continental'],
    thickness: ['Crust thickness · initial approximation, not elevation', '7 km', '35 km'],
    elevation: ['Elevation · reference datum, not sea level · world-relative color scale', `${number.format(terrainStats.minimumMeters)} m`, `${number.format(terrainStats.maximumMeters)} m`],
    uplift: ['Convergence uplift · strongest attenuated source, not accumulated history', '0 m', '12,000 m'],
    depth: ['Initial water depth · gray = dry · globe shows the bed, not a water-surface mesh', '0 m', `${number.format(maximumWaterDepth)} m`],
    waterBodies: [`Connected water bodies · gray = dry · main ocean = body ${world?.water.mainOceanId || 'none'}`, '', ''],
    catchments: ['Drainage catchments · colors identify terminal outlets, not states or rivers', '', ''],
    contributingArea: ['Contributing dry-land area · logarithmic color scale · not river discharge', '0 km²', `${number.format(maximumContributingArea / 1e6)} km²`],
  };
  const [title, low, high] = legends[currentLayer];
  element('legend-title').textContent = title;
  element('legend-low').textContent = low;
  element('legend-high').textContent = high;
  const plateLayer = currentLayer === 'plates' || currentLayer === 'boundaries';
  element('legend-scale').hidden = plateLayer || currentLayer === 'waterBodies' || currentLayer === 'surface' || currentLayer === 'catchments';
  element('boundary-legend').hidden = !plateLayer;
  element('legend-gradient').classList.toggle('water-gradient', currentLayer === 'depth');
}

function setInputs(recipe: Recipe): void {
  seedInput.value = recipe.seed;
  resolutionInput.value = String(recipe.subdivision);
  radiusInput.value = String(recipe.radiusMeters / 1000);
  plateCountInput.value = String(recipe.plateCount);
  plateCountInput.max = String(Math.min(32, 10 * 4 ** recipe.subdivision + 2));
  plateSpeedInput.value = String(recipe.maxPlateSpeedCmPerYear);
  continentalFractionInput.value = String(recipe.continentalFraction * 100);
  continentalScaleInput.value = String(recipe.continentalScale);
  reliefScaleInput.value = String(recipe.reliefScale);
  boundaryWidthInput.value = String(recipe.boundaryWidthKm);
  detailAmplitudeInput.value = String(recipe.detailAmplitudeMeters);
  waterModeInput.value = recipe.water.mode;
  if (recipe.water.mode === 'coverage') waterCoverageInput.value = String(recipe.water.fraction * 100);
  else waterVolumeInput.value = String(recipe.water.volumeCubicMeters / 1e9);
  updateWaterInputs();
}
function updateWaterInputs(): void {
  const coverage = waterModeInput.value === 'coverage';
  element('water-coverage-control').hidden = !coverage; waterCoverageInput.disabled = !coverage;
  element('water-volume-control').hidden = coverage; waterVolumeInput.disabled = coverage;
}
waterModeInput.addEventListener('change', updateWaterInputs);
function updateExaggeration(): void {
  const requested = Number(element<HTMLSelectElement>('exaggeration').value);
  const applied = map.setExaggeration(requested);
  element('exaggeration-note').textContent = `Globe relief and water: ${number.format(applied)}× applied${applied < requested ? ` (${requested}× requested; 20% radius display limit)` : ''} · display only; flat map stays flat`;
}
element('exaggeration').addEventListener('change', updateExaggeration);
resolutionInput.addEventListener('change', () => { plateCountInput.max = String(Math.min(32, 10 * 4 ** Number(resolutionInput.value) + 2)); });

async function generate(recipe: Recipe): Promise<void> {
  pause(); map.cancelPreparation();
  const request = ++generationId;
  const start = performance.now();
  cancel.hidden = false; save.disabled = true;
  showStatus('Building the surface in the native core…');
  document.body.dataset.state = 'generating';
  element<HTMLButtonElement>('play').disabled = true;
  try {
    const result = await api.generate(recipe);
    if (request !== generationId) return;
    const calculationMs = performance.now() - start;
    showStatus('Preparing GPU geometry for both views…');
    await map.setWorld(result.world);
    if (request !== generationId) return;
    // Queue acceptance before any subsequent UI action; no await between view
    // publication and this request. The old native world survives preparation.
    const accepted = api.acceptWorld(result.epoch);
    world = result.world; epoch = result.epoch; selected = null;
    terrainStats = summarizeTerrain(world.surface, world.terrain);
    const waterStats = summarizeWater(world.surface, world.water);
    maximumWaterDepth = waterStats.maximumDepthMeters;
    const drainageStats = summarizeDrainage(world.surface, world.water, world.drainage);
    maximumContributingArea = drainageStats.maximumContributingAreaSquareMeters;
    plateAreas = new Float64Array(world.recipe.plateCount);
    boundarySegments = new Map();
    for (let id = 0; id < world.stats.regionCount; id++) plateAreas[world.tectonics.owners[id]] += world.surface.areasSquareMeters[id];
    for (let segment = 0; segment < world.tectonics.boundaryTypes.length; segment++) {
      for (const cell of world.tectonics.boundaryCells.subarray(segment * 2, segment * 2 + 2)) {
        const list = boundarySegments.get(cell) ?? []; list.push(segment); boundarySegments.set(cell, list);
      }
    }
    map.setLayer(currentLayer);
    element('world-name').textContent = world.recipe.seed;
    element('region-count').textContent = number.format(world.stats.regionCount);
    element('surface-area').textContent = `${number.format(world.stats.totalAreaSquareMeters / 1e12)} M km²`;
    element('generation-time').textContent = `${number.format(performance.now() - start)} ms`;
    element('generation-time').title = `Native generation + IPC: ${calculationMs.toFixed(1)} ms; remaining time prepares GPU views.`;
    element('fingerprint').textContent = world.checksum;
    element('map-empty').hidden = true;
    element('selection-title').textContent = 'Explore the surface';
    element('selection-note').textContent = 'Select a region to see its geometry and connections.';
    element('selection-details').replaceChildren();
    element('boundary-details').replaceChildren();
    element('elevation-details').replaceChildren();
    element('height-summary').textContent = `Bed elevation: ${number.format(terrainStats.minimumMeters)} to ${number.format(terrainStats.maximumMeters)} m · area-weighted mean ${number.format(terrainStats.meanMeters)} m · no erosion yet`;
    const waterRequest = world.recipe.water.mode === 'coverage' ? `${(world.recipe.water.fraction * 100).toFixed(2)}% target` : `${number.format(world.recipe.water.volumeCubicMeters / 1e9)} km³ requested`;
    element('water-summary').textContent = `Water: ${(waterStats.waterAreaFraction * 100).toFixed(2)}% actual / ${waterRequest} · main ocean ${(waterStats.mainOceanAreaFraction * 100).toFixed(2)}% · inland ${(waterStats.inlandWaterAreaFraction * 100).toFixed(2)}% · ${waterStats.bodyCount} bodies · level ${number.format(waterStats.levelMeters)} m · resolved stock ${number.format(waterStats.resolvedVolumeCubicMeters / 1e9)} km³`;
    element('water-note').textContent = 'Select a region to inspect its initial water depth and stored volume. Coverage fitting never splits equal-elevation plateaus; actual coverage can differ from the target. No runoff, evaporation, or dynamic basin exchange is modeled yet.';
    element('drainage-summary').textContent = `Drainage: ${drainageStats.catchmentCount} terminal catchments · ${drainageStats.closedSinkCount} closed dry sinks · ${(drainageStats.closedDrainageLandFraction * 100).toFixed(2)}% of dry land ends in closed sinks · topology only, no flowing water`;
    element('drainage-note').textContent = 'Select a region to inspect its receiver, flat-routing rule, and contributing land area. Existing water bodies stop routing; closed sinks are preserved. Spill hierarchy and lake dynamics are not implemented.';
    updateExaggeration();
    element('crust-note').textContent = 'Select a region to inspect its crust potential, fitted threshold, and material approximations.';
    const crustSummary = summarizeCrust(world.surface, world.crust);
    element('crust-summary').textContent = `Continental-dominant crust: ${(crustSummary.continentalAreaFraction * 100).toFixed(2)}% actual / ${(world.recipe.continentalFraction * 100).toFixed(2)}% target · ${crustSummary.continentalPatchCount} connected patches · largest ${number.format(crustSummary.largestContinentalPatchAreaSquareMeters / 1e12)} M km² · not emerged land`;
    element('boundary-note').textContent = 'Select a region to inspect its plate and any inter-plate boundary segments.';
    element('diagnostic-tick').textContent = 'Step 0';
    updateLegend(); cancel.hidden = true; save.disabled = false;
    element<HTMLButtonElement>('play').disabled = false;
    document.body.dataset.state = 'ready';
    showStatus(`${world.recipe.plateCount} connected plates · ${world.tectonics.boundaryTypes.length} boundary segments · ${(world.stats.arrayBytes / 2 ** 20).toFixed(1)} MiB of model arrays · Static kinematics, no geological time integration`);
    await accepted;
  } catch (error) {
    if (request === generationId) {
      void api.cancelGeneration();
      cancel.hidden = true; save.disabled = world === null;
      element<HTMLButtonElement>('play').disabled = world === null;
      showStatus(error instanceof Error ? error.message : String(error), true);
      document.body.dataset.state = 'error';
    }
  }
}

function pause(): void {
  playing = false; clearTimeout(playbackTimer);
  element('play').textContent = 'Run diagnostic';
}
async function advance(): Promise<void> {
  if (!playing || advancing || !world) return;
  advancing = true;
  const request = generationId, activeEpoch = epoch;
  try {
    const frame = await api.advance(activeEpoch, 4);
    if (request !== generationId || frame.epoch !== epoch || !world) return;
    world.diagnosticField = frame.field; map.refreshField();
    if (selected !== null) inspect(selected);
    element('diagnostic-tick').textContent = `Step ${frame.tick}`;
    showStatus(`Diagnostic diffusion · relative mass error ${frame.relativeMassError.toExponential(2)} · Recipe saves the initial state, not this diagnostic step.`);
  } catch (e) { if (request === generationId) { pause(); showStatus(String(e), true); } }
  finally {
    advancing = false;
    if (playing) playbackTimer = window.setTimeout(() => void advance(), 33);
  }
}
element('play').addEventListener('click', () => {
  if (playing) pause();
  else { playing = true; element('play').textContent = 'Pause diagnostic'; void advance(); }
});
for (const button of document.querySelectorAll<HTMLButtonElement>('button[data-view]')) {
  button.addEventListener('click', () => {
    const mode = button.dataset.view as ViewMode;
    map.setMode(mode);
    for (const other of document.querySelectorAll('button[data-view]')) {
      other.classList.toggle('active', other === button); other.setAttribute('aria-pressed', String(other === button));
    }
    element('projection-label').textContent = mode === 'flat' ? 'EQUIRECTANGULAR PROJECTION' : 'GLOBE · COMPUTED RELIEF';
  });
}

element('recipe-form').addEventListener('submit', (event) => {
  event.preventDefault();
  try {
    void generate(parseRecipe({ ...DEFAULT_RECIPE, seed: seedInput.value, subdivision: Number(resolutionInput.value), radiusMeters: Number(radiusInput.value) * 1000,
      plateCount: Number(plateCountInput.value), maxPlateSpeedCmPerYear: Number(plateSpeedInput.value),
      continentalFraction: Number(continentalFractionInput.value) / 100, continentalScale: Number(continentalScaleInput.value),
      reliefScale: Number(reliefScaleInput.value), boundaryWidthKm: Number(boundaryWidthInput.value), detailAmplitudeMeters: Number(detailAmplitudeInput.value),
      water: waterModeInput.value === 'coverage' ? { mode: 'coverage', fraction: Number(waterCoverageInput.value) / 100 }
        : { mode: 'volume', volumeCubicMeters: Number(waterVolumeInput.value) * 1e9 } }));
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
});
cancel.addEventListener('click', () => {
  generationId++; map.cancelPreparation(); void api.cancelGeneration(); cancel.hidden = true; save.disabled = world === null;
  element<HTMLButtonElement>('play').disabled = world === null;
  document.body.dataset.state = world ? 'ready' : 'idle';
  showStatus(world ? 'Generation canceled. The previous surface is still displayed.' : 'Generation canceled.');
});
element('new-seed').addEventListener('click', () => { seedInput.value = crypto.randomUUID().slice(0, 8); });
element('open-recipe').addEventListener('click', async () => {
  try {
    const recipe = await api.openRecipe();
    if (recipe) { const validated = parseRecipe(recipe); setInputs(validated); void generate(validated); }
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
});
save.addEventListener('click', async () => {
  if (!world) return;
  try { if (await api.saveRecipe(world.recipe)) showStatus('Recipe saved. Reopen it to reproduce this surface.'); }
  catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
});
for (const button of document.querySelectorAll<HTMLButtonElement>('[data-layer]')) {
  button.addEventListener('click', () => {
    currentLayer = button.dataset.layer as Layer;
    for (const other of document.querySelectorAll('button[data-layer]')) {
      other.classList.toggle('active', other === button);
      other.setAttribute('aria-pressed', String(other === button));
    }
    map.setLayer(currentLayer); updateLegend();
  });
}
element<HTMLInputElement>('boundaries').addEventListener('change', (event) => map.setBoundaries((event.target as HTMLInputElement).checked));
element('reset-view').addEventListener('click', () => map.reset());
element('zoom-in').addEventListener('click', () => map.zoomBy(1.5));
element('zoom-out').addEventListener('click', () => map.zoomBy(1 / 1.5));
window.addEventListener('beforeunload', () => { pause(); map.dispose(); });
setInputs(DEFAULT_RECIPE);
void generate({ ...DEFAULT_RECIPE });
