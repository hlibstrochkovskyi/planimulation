import './style.css';
import { DEFAULT_RECIPE, parseRecipe } from '../core/recipe';
import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import type { DesktopAPI } from '../shared/desktop-api';
import { SurfaceMap } from './map';
import type { Layer, ViewMode } from './map';

function element<T extends HTMLElement>(id: string): T {
  const result = document.getElementById(id);
  if (!result) throw new Error(`Missing interface element: ${id}`);
  return result as T;
}

const api: DesktopAPI = window.desktop;
const seedInput = element<HTMLInputElement>('seed');
const resolutionInput = element<HTMLSelectElement>('subdivision');
const radiusInput = element<HTMLInputElement>('radius');
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
let currentLayer: Layer = 'signal';

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
  element('selection-title').textContent = `Region ${id.toLocaleString('en')}`;
  element('selection-note').textContent = `${Math.abs(lat).toFixed(2)}° ${lat >= 0 ? 'N' : 'S'} · ${Math.abs(lon).toFixed(2)}° ${lon >= 0 ? 'E' : 'W'}`;
  const details = element('selection-details');
  details.replaceChildren();
  for (const [label, value] of [
    ['Surface area', `${number.format(s.areasSquareMeters[id] / 1e6)} km²`],
    ['Connected neighbors', String(neighbors.length)],
    ['Mean neighbor distance', `${number.format(s.neighborDistancesMeters.subarray(s.neighborOffsets[id], s.neighborOffsets[id + 1]).reduce((sum, v) => sum + v, 0) / neighbors.length / 1000)} km`],
    ['Diagnostic signal', world.diagnosticField[id].toFixed(5)],
  ]) {
    const dt = document.createElement('dt'), dd = document.createElement('dd');
    dt.textContent = label; dd.textContent = value; details.append(dt, dd);
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
    signal: ['Seed field · dimensionless diagnostic', '−1', '+1'],
    area: ['Region area · true spherical area', world ? `${number.format(world.stats.minimumAreaSquareMeters / 1e6)} km²` : 'min', world ? `${number.format(world.stats.maximumAreaSquareMeters / 1e6)} km²` : 'max'],
    latitude: ['Latitude · distance from the equator', '90°', '0°'],
  };
  const [title, low, high] = legends[currentLayer];
  element('legend-title').textContent = title;
  element('legend-low').textContent = low;
  element('legend-high').textContent = high;
}

function setInputs(recipe: Recipe): void {
  seedInput.value = recipe.seed;
  resolutionInput.value = String(recipe.subdivision);
  radiusInput.value = String(recipe.radiusMeters / 1000);
}

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
    element('diagnostic-tick').textContent = 'Step 0';
    updateLegend(); cancel.hidden = true; save.disabled = false;
    element<HTMLButtonElement>('play').disabled = false;
    document.body.dataset.state = 'ready';
    showStatus(`Surface ready · ${(world.stats.arrayBytes / 2 ** 20).toFixed(1)} MiB of model arrays · Area error ${world.stats.relativeAreaError.toExponential(1)}`);
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
    element('projection-label').textContent = mode === 'flat' ? 'EQUIRECTANGULAR PROJECTION' : 'GLOBE · NO ELEVATION MODEL YET';
  });
}

element('recipe-form').addEventListener('submit', (event) => {
  event.preventDefault();
  try {
    void generate(parseRecipe({ ...DEFAULT_RECIPE, seed: seedInput.value, subdivision: Number(resolutionInput.value), radiusMeters: Number(radiusInput.value) * 1000 }));
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
    for (const other of document.querySelectorAll('[data-layer]')) {
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
