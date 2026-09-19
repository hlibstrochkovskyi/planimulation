import './style.css';
import { DEFAULT_RECIPE, parseRecipe } from '../core/recipe';
import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import type { DesktopAPI } from '../shared/desktop-api';
import { SurfaceMap } from './map';
import type { Layer } from './map';

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
let worker: Worker | null = null;
let generationId = 0;
let currentLayer: Layer = 'signal';

function showStatus(message: string, error = false): void {
  status.textContent = message;
  status.classList.toggle('error', error);
}

function inspect(id: number): void {
  if (!world) return;
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

const map = new SurfaceMap(element<HTMLCanvasElement>('map'), inspect);

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

function generate(recipe: Recipe): void {
  worker?.terminate();
  const request = ++generationId;
  const start = performance.now();
  cancel.hidden = false; save.disabled = true;
  showStatus('Preparing a new surface…');
  document.body.dataset.state = 'generating';
  const task = new Worker(new URL('./world.worker.ts', import.meta.url), { type: 'module' });
  worker = task;
  const stop = (): void => {
    task.terminate();
    if (worker === task) worker = null;
    cancel.hidden = true; save.disabled = world === null;
  };
  task.onerror = (event) => {
    if (request !== generationId) return;
    stop(); showStatus(event.message || 'Generation worker failed.', true);
    document.body.dataset.state = 'error';
  };
  task.onmessage = (event: MessageEvent<{ type: string; message: string; world: World }>) => {
    if (request !== generationId) return;
    if (event.data.type === 'progress') { showStatus(event.data.message); return; }
    if (event.data.type === 'error') { stop(); showStatus(event.data.message, true); document.body.dataset.state = 'error'; return; }
    try {
      world = event.data.world;
      const calculationMs = performance.now() - start;
      map.setWorld(world);
      map.setLayer(currentLayer);
      element('world-name').textContent = world.recipe.seed;
      element('region-count').textContent = number.format(world.stats.regionCount);
      element('surface-area').textContent = `${number.format(world.stats.totalAreaSquareMeters / 1e12)} M km²`;
      element('generation-time').textContent = `${number.format(calculationMs)} ms`;
      element('fingerprint').textContent = world.checksum;
      element('map-empty').hidden = true;
      element('selection-title').textContent = 'Explore the surface';
      element('selection-note').textContent = 'Select a region to see its geometry and connections.';
      element('selection-details').replaceChildren();
      updateLegend(); stop();
      document.body.dataset.state = 'ready';
      showStatus(`Surface ready · ${(world.stats.arrayBytes / 2 ** 20).toFixed(1)} MiB of model arrays · Area error ${world.stats.relativeAreaError.toExponential(1)}`);
    } catch (error) {
      stop(); showStatus(error instanceof Error ? error.message : String(error), true);
      document.body.dataset.state = 'error';
    }
  };
  task.postMessage(recipe);
}

element('recipe-form').addEventListener('submit', (event) => {
  event.preventDefault();
  try {
    generate(parseRecipe({ ...DEFAULT_RECIPE, seed: seedInput.value, subdivision: Number(resolutionInput.value), radiusMeters: Number(radiusInput.value) * 1000 }));
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
});
cancel.addEventListener('click', () => {
  generationId++; worker?.terminate(); worker = null; cancel.hidden = true; save.disabled = world === null;
  document.body.dataset.state = world ? 'ready' : 'idle';
  showStatus(world ? 'Generation canceled. The previous surface is still displayed.' : 'Generation canceled.');
});
element('new-seed').addEventListener('click', () => { seedInput.value = crypto.randomUUID().slice(0, 8); });
element('open-recipe').addEventListener('click', async () => {
  try {
    const recipe = await api.openRecipe();
    if (recipe) { const validated = parseRecipe(recipe); setInputs(validated); generate(validated); }
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
window.addEventListener('beforeunload', () => worker?.terminate());
setInputs(DEFAULT_RECIPE);
generate({ ...DEFAULT_RECIPE });
