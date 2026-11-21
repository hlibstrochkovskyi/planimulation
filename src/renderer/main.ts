import './style.css';
import { Workspace } from './workspace';
import { MapNavigation } from './map-navigation';
import { DEFAULT_RECIPE, LEGACY_MODEL_VERSION, MULTISCALE_GEOGRAPHY_MODEL_VERSION, BELTS_MAX_FILL_METERS, COASTAL_LOWLAND, MULTISCALE_BELTS_MODEL_VERSION, MULTISCALE_ERODED_MODEL_VERSION, MULTISCALE_LOWLANDS_MODEL_VERSION, MULTISCALE_UPLANDS_MODEL_VERSION, NEW_WORLD_RECIPE, maxSubdivision, parseRecipe } from '../core/recipe';
import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import type { DesktopAPI, PreparedMoistureWorld, PreparedSoilWorld, PreparedWaterWorld, PrescribedWaterMode, WaterBudget, WaterFrame } from '../shared/desktop-api';
import { SurfaceMap } from './map';
import type { Layer, ViewMode } from './map';
import { BOUNDARY_NAMES, speedCmPerYear } from '../core/tectonics';
import { summarizeCrust } from '../core/crust';
import { summarizeTerrain } from '../core/terrain';
import { summarizeWater } from '../core/water';
import { summarizeDrainage } from '../core/drainage';
import { basinTree, summarizeBasins } from '../core/basins';
import { approximateCubicKilometers, runoffDestination } from './water-budget';
import type { TemperatureNormals } from '../core/seasonal-temperature';
import type { WindNormals } from '../core/seasonal-wind';
import { MOISTURE_MAX_SECONDS, MOISTURE_STOCK_FIELDS, MOISTURE_MODES, SURFACE_TRANSFER_FIELDS, RUNOFF_TRANSFER_FIELDS } from '../shared/seasonal-moisture';
import type { SeasonalDisplayFrame } from '../shared/seasonal-display';
import { hasRegionalSurface, isSoilMoistureFrame } from '../shared/seasonal-display';
import { MOISTURE_LAYERS, isMoistureLayer, supportsMoistureLayer, moistureLayerTitle, moistureLayerValue, moistureCalendar } from './seasonal-moisture';
import { soilWaterInspection } from './soil-water-inspector';
import { BIOMES, aridityThreshold } from '../core/climate-normals';
import type { ClimateNormals } from '../core/climate-normals';
import { RIVER_CLASSES } from '../core/rivers';
import type { Rivers } from '../core/rivers';
import { CHANGE_YEARS, COVER_ARIDITY, DEFAULT_VEGETATION_SETTINGS, FOREST_COVER, MINIMUM_REFERENCE_COVER, OPEN_COVER, VARIABILITY_PRECIPITATION_MM, WOODY_ARIDITY, miami } from '../core/vegetation';
import type { VegetationFrame, VegetationSeries } from '../core/vegetation';
import { COLD_NEED_CELSIUS, DEFAULT_WILD_FOOD_SETTINGS } from '../core/wild-food';
import type { ForageExperiment, WildFoodFrame, WildFoodSeries } from '../core/wild-food';
import { DEFAULT_TRAVERSAL_SETTINGS, freezingMonths, isFrozenLakeWinter, isFrozenRiverWinter, isRegionPassable, traceTravelRoute } from '../core/traversal';
import type { TraversalCosts, TravelField, TravelSeason } from '../core/traversal';
import { ISOCHRONE_DAYS, UNREACHABLE_COLOR, travelTimeColor } from './travel-scale';

function element<T extends HTMLElement>(id: string): T {
  const result = document.getElementById(id);
  if (!result) throw new Error(`Missing interface element: ${id}`);
  return result as T;
}
function svgElement(id: string): SVGSVGElement {
  const result = document.getElementById(id);
  if (!(result instanceof SVGSVGElement)) throw new Error(`Missing interface element: ${id}`);
  return result;
}

const workspace = new Workspace();
const api: DesktopAPI = window.desktop;
const seedInput = element<HTMLInputElement>('seed');
const generatorFamilyInput = element<HTMLSelectElement>('generator-family');
const resolutionInput = element<HTMLSelectElement>('subdivision');
const radiusInput = element<HTMLInputElement>('radius');
const plateCountInput = element<HTMLInputElement>('plate-count');
const plateSpeedInput = element<HTMLInputElement>('plate-speed');
const continentalFractionInput = element<HTMLInputElement>('continental-fraction');
const continentalScaleInput = element<HTMLInputElement>('continental-scale');
const reliefScaleInput = element<HTMLInputElement>('relief-scale');
const boundaryWidthInput = element<HTMLInputElement>('boundary-width');
const detailAmplitudeInput = element<HTMLInputElement>('detail-amplitude');
const terrainPreparationInput = element<HTMLInputElement>('terrain-preparation-passes');
const waterModeInput = element<HTMLSelectElement>('water-mode');
const waterCoverageInput = element<HTMLInputElement>('water-coverage');
const waterVolumeInput = element<HTMLInputElement>('water-volume');
const status = element('status');
const cancel = element<HTMLButtonElement>('cancel');
const save = element<HTMLButtonElement>('save-recipe');
const saveResolved = element<HTMLButtonElement>('save-resolved');
const exportView = element<HTMLButtonElement>('export-view');
const waterAdd = element<HTMLButtonElement>('water-add');
const waterSpill = element<HTMLButtonElement>('water-spill');
const waterOpen = element<HTMLButtonElement>('open-water');
const waterSave = element<HTMLButtonElement>('save-water');
const waterBudgetButton = element<HTMLButtonElement>('water-budget-refresh');
const temperatureMonthInput = element<HTMLSelectElement>('temperature-month');
const temperatureLayerButton = document.querySelector<HTMLButtonElement>('[data-layer="temperature"]')!;
const climateLayerButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-layer="biome"], [data-layer="annualPrecipitation"]')];
element('biome-legend').replaceChildren(...BIOMES.slice(1).map(({ name, color }) => {
  const item = document.createElement('span'), swatch = document.createElement('i');
  swatch.style.background = `rgb(${color.join(',')})`; item.append(swatch, name); return item;
}));
const isochroneKeys = ISOCHRONE_DAYS.map((days) => ({ label: `${days.toLocaleString('en')} d`, color: travelTimeColor(days) }));
element('isochrone-legend').replaceChildren(...[...isochroneKeys,
  { label: 'Unreachable on foot (water)', color: UNREACHABLE_COLOR }].map(({ label, color }) => {
  const item = document.createElement('span'), swatch = document.createElement('i');
  swatch.style.background = `rgb(${color.join(',')})`; item.append(swatch, label); return item;
}), ...[['travel-origin-key', 'Origin'], ['travel-route-key', 'Least-time route']].map(([className, label]) => {
  const item = document.createElement('span'), swatch = document.createElement('i');
  swatch.className = className; item.append(swatch, label); return item;
}));
const riverLayerButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-layer="annualRunoff"], [data-layer="riverDischarge"]')];
const vegetationLayerButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-layer="vegetationState"], [data-layer="woodyCover"], [data-layer="woodyChange"], [data-layer="productivity"], [data-layer="precipitationAnomaly"]')];
const foodLayerButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-layer="foragerCapacity"], [data-layer="leanSeason"], [data-layer="gameStock"]')];
const forageButtons = [...document.querySelectorAll<HTMLButtonElement>('#food-forage button')];
/** Window of the running mean drawn over a region's yearly forager capacity. */
const FOOD_MEAN_YEARS = 10;
const MONTH_NAMES = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const vegetationButtons = ([[1, 'vegetation-1'], [10, 'vegetation-10'], [100, 'vegetation-100']] as const).map(([years, id]) => ({ years, button: element<HTMLButtonElement>(id) }));
const vegetationReset = element<HTMLButtonElement>('vegetation-reset');
const traversalLayerButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-layer="travelCost"], [data-layer="travelTime"]')];
const travelSeasonButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-travel-season]')];
const travelFromHereButton = element<HTMLButtonElement>('travel-from-here');
const travelClearSourceButton = element<HTMLButtonElement>('traversal-clear-source');
const travelFrameButton = element<HTMLButtonElement>('traversal-frame');
const windLayerButton = document.querySelector<HTMLButtonElement>('[data-layer="windSpeed"]')!;
const moistureStart = element<HTMLButtonElement>('moisture-start');
const moistureStartOrographic = element<HTMLButtonElement>('moisture-start-orographic');
const moistureStartPrecise = element<HTMLButtonElement>('moisture-start-precise');
const moistureStartRegional = element<HTMLButtonElement>('moisture-start-regional');
const moistureStartSoil = element<HTMLButtonElement>('moisture-start-soil');
const moistureStep = element<HTMLButtonElement>('moisture-step');
const moisturePlay = element<HTMLButtonElement>('moisture-play');
const moistureInterval = element<HTMLSelectElement>('moisture-interval');
const moistureOpen = element<HTMLButtonElement>('open-seasonal');
const moistureSave = element<HTMLButtonElement>('save-seasonal');
const number = new Intl.NumberFormat('en', { maximumFractionDigits: 1 });
const budgetNumber = new Intl.NumberFormat('en', { maximumSignificantDigits: 6 });
let world: World | null = null;
let waterFrame: WaterFrame | null = null;
let waterBudget: WaterBudget | null = null;
let temperatureNormals: TemperatureNormals | null = null;
let windNormals: WindNormals | null = null;
let climateNormals: ClimateNormals | null = null;
let rivers: Rivers | null = null;
let vegetation: VegetationFrame | null = null;
let vegetationSeries: VegetationSeries | null = null;
let vegetationBusy = false;
let seriesRequest = 0;
let wildFood: WildFoodFrame | null = null;
let wildFoodSeries: WildFoodSeries | null = null;
let forageResult: ForageExperiment | null = null;
let forageBusy = false;
let traversalCosts: TraversalCosts | null = null;
let travelField: TravelField | null = null;
let travelSeason: TravelSeason = 'summer';
let travelOrigin: number | null = null;
let travelRequest = 0;
let moistureFrame: SeasonalDisplayFrame | null = null;
let moistureBusyEpoch: number | null = null;
let moisturePlaying = false;
let moistureTimer = 0;
let temperatureRange = { minimum: 0, maximum: 0 };
let windMaximum = 0;
let waterBusy = false;
let checkpointOpening = false;
let budgetBusyEpoch: number | null = null;
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
let basinStats: ReturnType<typeof summarizeBasins> | null = null;
let basinChildren: number[][] = [];
let inspectedBasin: number | null = null;
let plateAreas = new Float64Array(0);
let boundarySegments = new Map<number, number[]>();
let unsavedProgress = false;
let diagnosticDirty = false;

function showStatus(message: string, error = false): void {
  status.textContent = message;
  status.classList.toggle('error', error);
  workspace.statusChanged(message, error);
}

function updateExperimentControls(): void {
  const generating = document.body.dataset.state === 'generating';
  const busy = generating || checkpointOpening || waterBusy || budgetBusyEpoch === epoch
    || advancing || moistureBusyEpoch === epoch;
  const hasSeasonal = moistureFrame !== null;
  const finished = (moistureFrame?.elapsedSeconds ?? 0) >= MOISTURE_MAX_SECONDS;
  moistureStart.disabled = !world || busy || playing || hasSeasonal || (waterFrame?.step ?? 0) > 0;
  moistureStartOrographic.disabled = moistureStart.disabled;
  moistureStartPrecise.disabled = moistureStart.disabled;
  moistureStartRegional.disabled = moistureStart.disabled;
  moistureStartSoil.disabled = moistureStart.disabled;
  moistureStep.disabled = !hasSeasonal || busy || moisturePlaying || finished;
  // Pause must remain available while the current native step is in flight.
  moisturePlay.disabled = !hasSeasonal || finished || (!moisturePlaying && busy);
  moisturePlay.textContent = moisturePlaying ? 'Pause' : 'Run';
  moistureInterval.disabled = busy || moisturePlaying;
  // File actions require a settled, paused snapshot; the pause control remains
  // usable while an already requested interval finishes.
  moistureOpen.disabled = busy || moisturePlaying || playing;
  moistureSave.disabled = !hasSeasonal || busy || moisturePlaying || playing;
  element<HTMLButtonElement>('generate').disabled = checkpointOpening || waterBusy;
  element<HTMLButtonElement>('open-recipe').disabled = checkpointOpening || waterBusy;
  for (const button of document.querySelectorAll<HTMLButtonElement>('[data-layer]')) {
    const layer = button.dataset.layer ?? '';
    if (isMoistureLayer(layer)) {
      button.disabled = generating || !supportsMoistureLayer(moistureFrame, layer);
      button.title = moistureLayerTitle(moistureFrame, layer);
      if (layer === 'liquidWater' || layer === 'runoffWater') {
        const label = [...button.childNodes].find(node => node.nodeType === Node.TEXT_NODE);
        const soil = !!moistureFrame && isSoilMoistureFrame(moistureFrame);
        if (label) label.textContent = layer === 'liquidWater' ? soil ? 'Unified terrestrial liquid' : 'Local liquid'
          : soil ? 'Delayed soil drainage' : 'Runoff in transit';
      }
    }
  }
  const manualBlocked = !world || busy || playing || moisturePlaying || hasSeasonal;
  waterAdd.disabled = manualBlocked || selected === null; waterSpill.disabled = waterAdd.disabled;
  waterSave.disabled = manualBlocked; waterBudgetButton.disabled = manualBlocked;
  waterOpen.disabled = busy || moisturePlaying || playing;
  element<HTMLButtonElement>('play').disabled = !world || hasSeasonal || (!playing && busy);
  for (const { button } of vegetationButtons) button.disabled = !vegetation || vegetationBusy || generating || vegetation.year >= 10_000;
  vegetationReset.disabled = !vegetation || vegetationBusy || generating || vegetation.year === 0;
  for (const button of forageButtons) button.disabled = !wildFood || vegetationBusy || forageBusy || generating || wildFood.year === 0;
  document.body.dataset.seasonalWater = moisturePlaying ? 'running' : moistureBusyEpoch === epoch ? 'working' : hasSeasonal ? 'paused' : 'idle';
  workspace.update({ layer: currentLayer, selected,
    seasonalModel: moistureFrame ? isSoilMoistureFrame(moistureFrame) ? 'soil'
      : Object.entries(MOISTURE_MODES).find(([, mode]) => mode.modelVersion === moistureFrame!.modelVersion)?.[0] ?? 'baseline' : null,
    seasonalWorking: moistureBusyEpoch === epoch, seasonalRunning: moisturePlaying, seasonalFinished: finished,
    diagnosticRunning: playing, diagnosticDirty, busy, unsaved: unsavedProgress || diagnosticDirty, manualStep: waterFrame?.step ?? 0,
    vegetationYear: vegetation?.year ?? null, vegetationWorking: vegetationBusy,
    layerReading: world && moistureFrame && selected !== null && isMoistureLayer(currentLayer) && supportsMoistureLayer(moistureFrame, currentLayer)
      ? { label: moistureLayerTitle(moistureFrame, currentLayer),
        value: `${budgetNumber.format(moistureLayerValue(moistureFrame, currentLayer, selected, world.surface.areasSquareMeters[selected]))} ${MOISTURE_LAYERS[currentLayer].unit}`,
        scope: `Accepted frame · ${budgetNumber.format(moistureFrame.elapsedSeconds / 86400)} elapsed days` } : null });
}

function appendDetails(target: HTMLElement, rows: Array<[string, string]>): void {
  target.replaceChildren();
  for (const [label, value] of rows) {
    const dt = document.createElement('dt'), dd = document.createElement('dd');
    dt.textContent = label; dd.textContent = value; target.append(dt, dd);
  }
}
const stockNames = ['Local liquid', 'Snow water equivalent', 'Soil water', 'Runoff in transit', 'Terminal water', 'Atmospheric vapor'];
const surfaceTransferNames = ['Rain', 'Snowfall', 'Melt', 'Liquid evaporation', 'Soil evaporation', 'Infiltration', 'Generated liquid runoff', 'Soil drainage'];
const runoffTransferNames = ['Drainage departure', 'Received transit', 'Terminal delivery', 'Terminal evaporation'];

function renderMoisture(): void {
  const frame = moistureFrame;
  if (!frame || !world) {
    delete element('moisture-time').dataset.requestMilliseconds;
    delete element('moisture-budget-details').dataset.relativeMassResidual;
    delete element('moisture-budget-details').dataset.relativeLedgerResidual;
    element('moisture-time').textContent = 'Not initialized. Choose a model and initialize.';
    element('moisture-step-note').textContent = 'No interval recorded. Playback waits for each native response; frame rate does not set model time.';
    element('moisture-budget-note').textContent = 'Initialize seasonal water to inspect its approximate binary64 accounting. Do not add it to the full initial water inventory.';
    element('moisture-budget-details').replaceChildren(); element('moisture-selection-details').replaceChildren();
    element('moisture-selection-title').textContent = 'Select a region';
    element('moisture-selection-note').textContent = 'Stocks and recorded transfers use the same received frame as the map.';
    updateExperimentControls(); return;
  }
  element('moisture-time').textContent = moistureCalendar(frame.elapsedSeconds);
  const month = Math.floor((Math.floor(frame.elapsedSeconds / 86400) % 365) * 12 / 365) + 1;
  if (isSoilMoistureFrame(frame)) {
    element('moisture-step-note').textContent = frame.intervalSeconds === 0
      ? `Read-only observation · no interval transfers · next forcing month ${month}. The normals selector is independent.`
      : `Last interval ${frame.intervalSeconds / 3600} h · ${frame.coupledSubsteps} coupled / ${frame.atmosphericSubsteps} atmospheric / ${frame.surfaceSubsteps} surface substeps · next forcing month ${month}. Pause may finish one requested interval.`;
    const inspection = soilWaterInspection(world, frame, selected);
    element('moisture-budget-details').dataset.relativeMassResidual = String(frame.budget.relativeGlobalResidual);
    element('moisture-budget-details').dataset.relativeLedgerResidual = String(frame.budget.maximumRelativeLocalResidual);
    element('moisture-budget-note').textContent = inspection.budgetNote;
    element('moisture-selection-title').textContent = selected === null ? 'Select a region' : `Region ${selected} · unified soil water`;
    element('moisture-selection-note').textContent = inspection.selectionNote;
    appendDetails(element('moisture-budget-details'), inspection.budgetRows);
    appendDetails(element('moisture-selection-details'), inspection.selectionRows);
    updateExperimentControls(); return;
  }
  element('moisture-step-note').textContent = frame.intervalSeconds === 0
    ? `Read-only observation · no interval transfers · next forcing month ${month}. Advance to record new rates; the normals selector is independent.`
    : `Last interval ${frame.intervalSeconds / 3600} h · ${frame.coupledSubsteps} coupled / ${frame.transportSubsteps} transport substeps · next forcing month ${month}. The normals selector is independent. Pause may finish one requested interval.`;
  const budget = frame.budget;
  const regional = frame.regionalSurface;
  const names = regional ? stockNames.map((name, i) => i === 4 ? 'Regional pooled liquid' : name) : stockNames;
  element('moisture-budget-details').dataset.relativeMassResidual = String(budget.residualKilograms / Math.max(budget.initialMobileWaterKilograms, 1));
  element('moisture-budget-details').dataset.relativeLedgerResidual = String(budget.maximumRelativeLocalSurfaceLedgerResidual);
  const volume = (kilograms: number): string => `${budgetNumber.format(kilograms / 1e12)} km³ WE`;
  const total = MOISTURE_STOCK_FIELDS.reduce((sum, key) => sum + budget[key], budget.referenceBodyWaterKilograms ?? 0);
  appendDetails(element('moisture-budget-details'), [
    ['Initial mobile partition', volume(budget.initialMobileWaterKilograms)],
    ...MOISTURE_STOCK_FIELDS.map((key, i): [string, string] => [names[i], volume(budget[key])]),
    ...(regional ? [['Finite reference-body liquid', volume(budget.referenceBodyWaterKilograms!)],
      ['Cumulative surface-face transfers · flow integral', volume(regional.cumulativeTransferredKilograms)],
      ['Last interval · accepted surface-face transfers', volume(regional.flow.transferredKilograms)],
      ['Last interval · unapplied face requests', `${regional.flow.deferredRequests} requests · ${budgetNumber.format(regional.flow.deferredRequestKilograms)} kg · retained at donors`],
      ['Last interval · unapplied evaporation requests', `${regional.flow.deferredEvaporationRequests} requests · ${budgetNumber.format(regional.flow.deferredEvaporationRequestKilograms)} kg · retained at donors`]] as Array<[string, string]> : []),
    ['Total current owned water', volume(total)],
    ['Mass residual / initial', (budget.residualKilograms / Math.max(budget.initialMobileWaterKilograms, 1)).toExponential(3)],
    ['Maximum regional ledger residual', budget.maximumRelativeLocalSurfaceLedgerResidual.toExponential(3)],
    ['Cumulative evaporation · flow integral', volume(budget.cumulativeEvaporationKilograms)],
    ['Cumulative precipitation · flow integral', volume(budget.cumulativePrecipitationKilograms)],
    ['Cumulative terminal delivery · flow integral', volume(budget.cumulativeRunoffTransfers.terminalDelivery)],
    ['Cumulative terminal evaporation · flow integral', volume(budget.cumulativeRunoffTransfers.terminalEvaporation)],
  ]);
  const response = 'orography' in frame.settings ? ' · experimental upslope-enhanced removal of supersaturation; not calibrated' : '';
  const precision = 'terminalNumerics' in frame.settings ? ' · checkpointed liquid/snow/soil/terminal low components; regional display values are rounded leading fields' : '';
  element('moisture-budget-note').textContent = regional
    ? `${frame.modelVersion} · regional columns plus finite reference-body liquid, counted once per body. Capped Manning-inspired mobility; not calibrated. Initial body heads/coasts remain prescribed. Whole-column wet/dry and soil coupling are coarse approximations. Flow integrals and deferred requests are not stocks. Save preserves complete native state.`
    : `${frame.modelVersion}${response}${precision} · six exclusive stocks. Flow integrals can count recirculated water and must not be added to the inventory. Fixed geography; no lake levels or deep-water replenishment. Save seasonal checkpoint preserves the complete native state, not just these display values.`;
  if (selected === null) {
    element('moisture-selection-title').textContent = 'Select a region';
    element('moisture-selection-details').replaceChildren();
  } else {
    const id = selected, area = world.surface.areasSquareMeters[id];
    const column = (kg: number): string => `${budgetNumber.format(kg / area)} mm WE`;
    element('moisture-selection-title').textContent = `Region ${id} · seasonal water`;
    element('moisture-selection-note').textContent = regional
      ? `At ${frame.elapsedSeconds} elapsed seconds. Surface-face totals are cumulative, not rates. Regional depth is pooled liquid / whole computational area; reference-body heads are prescribed initial geometry. Last-interval atmospheric/runoff transfers cover ${frame.intervalSeconds / 3600} h. Visible shorelines are approximate.`
      : `At ${frame.elapsedSeconds} elapsed seconds. Transfer amounts cover only the last ${frame.intervalSeconds / 3600} h, not an annual normal. Terminal columns use fixed reference area, not a lake depth. Receiver: ${world.drainage.receivers[id] === id ? 'self · terminal' : `region ${world.drainage.receivers[id]}`}.`;
    const bodySlot = regional?.referenceBodyIds.indexOf(world.water.bodyIds[id]) ?? -1;
    appendDetails(element('moisture-selection-details'), [
      ...MOISTURE_STOCK_FIELDS.map((key, i): [string, string] => [names[i], column(frame.stocks[key][id])]),
      ...(regional ? [['Regional pooled liquid depth', `${budgetNumber.format(regional.depthMeters[id])} m`],
        ['Cumulative surface-face inflow', volume(regional.cumulativeIncomingKilograms[id])],
        ['Cumulative surface-face outflow', volume(regional.cumulativeOutgoingKilograms[id])],
        ...(bodySlot >= 0 ? [['Reference body owner', `Body ${regional.referenceBodyIds[bodySlot]} · shared stock, not allocated to this region`],
          ['Whole reference-body mobile liquid', volume(regional.referenceBodyHighKilograms[bodySlot] + regional.referenceBodyLowKilograms[bodySlot])]] : [])] as Array<[string, string]> : []),
      ...SURFACE_TRANSFER_FIELDS.map((key, i): [string, string] => [`Last interval · ${surfaceTransferNames[i]}`, column(frame.surfaceTransfers[key][id])]),
      ...RUNOFF_TRANSFER_FIELDS.map((key, i): [string, string] => [`Last interval · ${runoffTransferNames[i]}`, column(frame.runoffTransfers[key][id])]),
      ['Mean drainage departure', frame.intervalSeconds ? `${budgetNumber.format(frame.runoffTransfers.sent[id] / 1000 / frame.intervalSeconds)} m³/s · not hydraulic discharge` : 'No interval'],
    ]);
  }
  updateExperimentControls();
}

async function requestMoisture(seconds: number, initialize?: 'orographic' | 'precise' | 'regional' | 'soil'): Promise<void> {
  if (!world || moistureBusyEpoch === epoch || waterBusy || checkpointOpening || advancing
    || budgetBusyEpoch === epoch || document.body.dataset.state === 'generating') return;
  const activeEpoch = epoch, origin = world, request = generationId;
  moistureBusyEpoch = activeEpoch; updateExperimentControls();
  const started = performance.now();
  try {
    const frame = initialize === 'soil' ? await api.initializeSoilMoisture(activeEpoch) : initialize
      ? initialize === 'regional' ? await api.initializeRegionalMoisture(activeEpoch) : initialize === 'precise' ? await api.initializePreciseMoisture(activeEpoch) : await api.initializeOrographicMoisture(activeEpoch)
      : moistureFrame && isSoilMoistureFrame(moistureFrame) ? await api.seasonalSoilMoisture(activeEpoch, seconds)
      : await api.seasonalMoisture(activeEpoch, seconds);
    const requestMilliseconds = performance.now() - started;
    // An accepted old-world step may finish while a replacement is preparing.
    // Retain it until that world is actually replaced, including cancellation.
    if (frame.epoch !== epoch || world !== origin) return;
    moistureFrame = frame; unsavedProgress = true; map.setMoistureFrame(frame);
    if (hasRegionalSurface(frame)) {
      const depth = isSoilMoistureFrame(frame) ? frame.regionalLiquidDepthMeters : frame.regionalSurface!.depthMeters;
      maximumWaterDepth = Math.max(world.water.depthMeters.reduce((maximum, depth) => Math.max(maximum, depth), 0),
        depth.reduce((maximum, depth) => Math.max(maximum, depth), 0));
      if (currentLayer === 'terminalWater') selectLayer('regionalDepth');
    }
    waterBudget = null; renderWaterBudget(); renderMoisture(); updateLegend();
    element('moisture-time').dataset.requestMilliseconds = String(requestMilliseconds);
    element('model-label').textContent = `${world.recipe.modelVersion.toUpperCase()} · ${frame.modelVersion.toUpperCase()} · FIXED GEOGRAPHY`;
    const residual = isSoilMoistureFrame(frame) ? frame.budget.relativeGlobalResidual
      : frame.budget.residualKilograms / Math.max(frame.budget.initialMobileWaterKilograms, 1);
    if (request === generationId) showStatus(`Seasonal water ${budgetNumber.format(frame.elapsedSeconds / 86400)} days · native request + IPC ${number.format(requestMilliseconds)} ms · relative total-water residual ${residual.toExponential(2)}.`);
    if (frame.elapsedSeconds >= MOISTURE_MAX_SECONDS) pauseMoisture();
  } catch (error) {
    if (epoch === activeEpoch) {
      pauseMoisture();
      if (request === generationId) showStatus(error instanceof Error ? error.message : String(error), true);
    }
  } finally {
    if (moistureBusyEpoch === activeEpoch) moistureBusyEpoch = null;
    updateExperimentControls();
    if (moisturePlaying && request === generationId && epoch === activeEpoch) {
      moistureTimer = window.setTimeout(() => void requestMoisture(Number(moistureInterval.value)), 33);
    }
  }
}
function pauseMoisture(): void {
  moisturePlaying = false; clearTimeout(moistureTimer); updateExperimentControls();
}
moistureStart.addEventListener('click', () => { pause(); void requestMoisture(0); });
moistureStartOrographic.addEventListener('click', () => { pause(); void requestMoisture(0, 'orographic'); });
moistureStartPrecise.addEventListener('click', () => { pause(); void requestMoisture(0, 'precise'); });
moistureStartRegional.addEventListener('click', () => { pause(); void requestMoisture(0, 'regional'); });
moistureStartSoil.addEventListener('click', () => { pause(); void requestMoisture(0, 'soil'); });
moistureStep.addEventListener('click', () => { pause(); void requestMoisture(Number(moistureInterval.value)); });
moisturePlay.addEventListener('click', () => {
  if (moisturePlaying) pauseMoisture();
  else if (moistureFrame && moistureBusyEpoch !== epoch) {
    pause(); moisturePlaying = true; updateExperimentControls();
    void requestMoisture(Number(moistureInterval.value));
  }
});

function renderWaterBudget(): void {
  const details = element('water-budget-details');
  details.replaceChildren();
  if (!world || !waterBudget) {
    element('water-budget-note').textContent = 'Inspect the native inventory. Displayed depths are not used for accounting.';
    return;
  }
  const budget = waterBudget;
  const add = (label: string, value: string, exact?: string): void => {
    const dt = document.createElement('dt'), dd = document.createElement('dd');
    dt.textContent = label; dd.textContent = value;
    if (exact) dd.title = `${exact} units of 2^-56 m³`;
    details.append(dt, dd);
  };
  const approximate = (units: string): string => {
    const volume = approximateCubicKilometers(units);
    return `≈ ${volume !== 0 && volume < 0.001 ? volume.toExponential(3) : budgetNumber.format(volume)} km³`;
  };
  add('Initial accounted stock', approximate(budget.initialTotalUnits), budget.initialTotalUnits);
  add('Accepted manual input', approximate(budget.acceptedInputUnits), budget.acceptedInputUnits);
  add('Current active stocks', approximate(budget.storedTotalUnits), budget.storedTotalUnits);
  const residual = BigInt(budget.storedTotalUnits) - BigInt(budget.initialTotalUnits) - BigInt(budget.acceptedInputUnits);
  add('Exact balance residual', `${residual} units`);
  add('Active basin branches', String(budget.stocks.length));
  const largest: Array<{ branch: number; volumeUnits: string; amount: bigint }> = [];
  for (const stock of budget.stocks) {
    const amount = BigInt(stock.volumeUnits);
    if (amount === 0n) continue;
    const slot = largest.findIndex((entry) => amount > entry.amount || (amount === entry.amount && stock.branch < entry.branch));
    largest.splice(slot < 0 ? largest.length : slot, 0, { ...stock, amount });
    if (largest.length > 3) largest.pop();
  }
  for (const stock of largest) {
    add(`Large stock · branch ${stock.branch}`, approximate(stock.volumeUnits), stock.volumeUnits);
  }
  if (selected !== null) {
    const destination = runoffDestination(world, budget, selected);
    add('Selected source destination', `Region ${destination.terminal} → branch ${destination.branch}`);
    add('Destination branch stock', approximate(destination.volumeUnits), destination.volumeUnits);
  }
  element('water-budget-note').textContent = `Prescribed step ${budget.step} · exact native ledger: initial + accepted = exclusive active stocks. Approximate km³ labels are display only.`;
}

async function refreshWaterBudget(): Promise<void> {
  if (!world || moistureFrame || moistureBusyEpoch === epoch || budgetBusyEpoch === epoch) return;
  const request = generationId, activeEpoch = epoch;
  budgetBusyEpoch = activeEpoch; waterBudgetButton.disabled = true;
  updateExperimentControls();
  try {
    const budget = await api.inspectWaterBudget(activeEpoch);
    if (request !== generationId || budget.epoch !== epoch) return;
    waterBudget = budget; renderWaterBudget();
  } catch (error) {
    if (request === generationId) showStatus(error instanceof Error ? error.message : String(error), true);
  } finally {
    if (budgetBusyEpoch === activeEpoch) {
      budgetBusyEpoch = null;
      waterBudgetButton.disabled = document.body.dataset.state === 'generating' || checkpointOpening || waterBusy;
      updateExperimentControls();
    }
  }
}

function riverRows(frame: Rivers, id: number, bodyId: number, staticOutlet: number): [string, string][] {
  const monthly = frame.monthlyDischarge.subarray(id * frame.monthDays.length, (id + 1) * frame.monthDays.length);
  const low = Math.min(...monthly), high = Math.max(...monthly);
  const flow = (value: number): string => `${value.toLocaleString('en', { maximumFractionDigits: 2 })} m³/s`;
  const mm = (value: number): string => `${number.format(value)} mm/year`;
  const rows: [string, string][] = [['Potential evapotranspiration (PET)', mm(frame.pet[id])]];
  if (!bodyId) rows.push(
    ['Actual evapotranspiration (AET)', mm(frame.aet[id])],
    ['Land runoff', mm(frame.runoff[id])],
  );
  const lake = frame.lakes.get(staticOutlet);
  rows.push(
    ['Mean river discharge', flow(frame.meanDischarge[id])],
    ['Minimum monthly discharge', `${flow(low)} · month ${monthly.indexOf(low) + 1}`],
    ['Maximum monthly discharge', `${flow(high)} · month ${monthly.indexOf(high) + 1}`],
    ['River class', RIVER_CLASSES[frame.class[id]]],
    ['Downstream terminal', frame.terminal[id] === frame.oceanTerminal ? 'Ocean' : `Endorheic basin at region ${number.format(frame.terminal[id])}`],
  );
  if (lake && (id === staticOutlet || bodyId !== 0)) {
    const km3 = (value: number): string => `${(value / 1e9).toLocaleString('en', { maximumFractionDigits: 3 })} km³/year`;
    rows.push(
      ['Terminal lake area', `${number.format(lake.area / 1e6)} km² · ${bodyId ? 'initial water body' : 'effective evaporation area'}`],
      ['Terminal lake inflow', `${km3(lake.inflow)} · direct precipitation ${km3(lake.precipitation)}`],
      ['Terminal lake evaporation', `${km3(lake.evaporation)} · capacity ${km3(lake.potentialEvaporation)}`],
      ['Terminal lake spill', lake.spill > 0 ? `${km3(lake.spill)} · outlet ${lake.outlet!.join(' → ')} at ${number.format(lake.sillElevation)} m` : 'Closed'],
    );
    if (lake.storage > 0) rows.push(['Terminal lake accumulation', km3(lake.storage)]);
  }
  return rows;
}

function inspect(id: number, preserveBasin = false): void {
  if (!world) return;
  selected = id;
  renderMoisture();
  if (waterBudget) renderWaterBudget();
  updateExperimentControls();
  inspectBasin(preserveBasin && inspectedBasin !== null ? inspectedBasin : world.basins.regionNodes[id]);
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
    ...(temperatureNormals ? [
      [`Month ${Number(temperatureMonthInput.value) + 1} temperature normal`, `${temperatureNormals.monthlyTemperatureCelsius[Number(temperatureMonthInput.value)][id].toFixed(1)} °C`],
      ['Annual mean temperature normal', `${temperatureNormals.annualMeanCelsius[id].toFixed(1)} °C`],
      ['Daily normal range', `${temperatureNormals.annualMinimumCelsius[id].toFixed(1)} to ${temperatureNormals.annualMaximumCelsius[id].toFixed(1)} °C`],
    ] : []),
    ...(climateNormals ? [
      ['Biome (climate class)', BIOMES[climateNormals.biome[id]].name],
      ['Climate annual mean', `${climateNormals.annualMeanCelsius[id].toFixed(1)} °C`],
      ['Climate warmest / coldest month', `${climateNormals.warmestMonthCelsius[id].toFixed(1)} / ${climateNormals.coldestMonthCelsius[id].toFixed(1)} °C`],
      ['Annual precipitation', `${number.format(climateNormals.annualPrecipitationMm[id])} mm/year · driest month ${number.format(climateNormals.driestMonthMm[id])} mm`],
      ['Zonal precipitation potential', `${number.format(climateNormals.zonalPrecipitationMm[id])} mm/year`],
      ['Moisture supply', `${(climateNormals.moistureSupply[id] * 100).toFixed(0)}% of zonal potential`],
      ['Windward lift', `×${climateNormals.lift[id].toFixed(2)}`],
    ] : []),
    ...(rivers ? riverRows(rivers, id, world.water.bodyIds[id], world.drainage.outlets[id]) : []),
    ...(vegetation ? [
      ['Vegetation state', `${BIOMES[vegetation.label[id]].name} · year ${vegetation.year}`],
      ['Woody / herb cover', `${vegetation.woody[id].toFixed(2)} / ${vegetation.herb[id].toFixed(2)}`],
      [`Woody change, ${CHANGE_YEARS} years`, `${vegetation.woodyChange[id] >= 0 ? '+' : ''}${vegetation.woodyChange[id].toFixed(2)}`],
      ['Net primary productivity', `${number.format(Math.round(vegetation.productivity[id]))} g dry matter/m²/year`],
      ['Established cover', `${vegetation.establishedCover[id].toFixed(2)} · ${DEFAULT_VEGETATION_SETTINGS.labelMemoryYears}-year memory`],
      ['Woody capacity', `${vegetation.woodyCapacity[id].toFixed(2)} · total cover capacity ${vegetation.coverCapacity[id].toFixed(2)}`],
      ['This year\'s precipitation', `${(vegetation.precipitationFactor[id] * 100).toFixed(0)}% of normal`],
      ['Root-zone moisture', `${(vegetation.rootMoisture[id] * 100).toFixed(0)}% of normal · ${DEFAULT_VEGETATION_SETTINGS.rootMemoryYears}-year memory`],
    ] : []),
    ...(wildFood && world.water.bodyIds[id] === 0 ? [
      ['Forager capacity', `${(wildFood.capacity[id] * 100).toFixed(2)} people/100 km² · ${number.format(Math.round(wildFood.capacity[id] * s.areasSquareMeters[id] / 1e6))} in this region`],
      ['With perfect storage', `${(wildFood.storedCapacity[id] * 100).toFixed(2)} people/100 km² · lean month ${MONTH_NAMES[wildFood.leanMonth[id]]}`],
      ['Plant food / game offtake', `${number.format(Math.round(wildFood.plantKcal[id] * 1e3))} / ${number.format(Math.round(wildFood.gameKcal[id] * 1e3))} Mcal/km²/year`],
      ['Game stock', `${(wildFood.game[id] * 1e3).toFixed(0)} kg/km² · ${wildFood.gameCapacity[id] > 0 ? `${(wildFood.game[id] / wildFood.gameCapacity[id] * 100).toFixed(0)}% of capacity` : 'no capacity'}`],
    ] : []),
    ...(traversalCosts ? traversalRegionRows(id) : []),
    ...(windNormals ? [
      [`Month ${Number(temperatureMonthInput.value) + 1} wind speed normal`, `${Math.hypot(windNormals.monthlyEastMetersPerSecond[Number(temperatureMonthInput.value)][id], windNormals.monthlyNorthMetersPerSecond[Number(temperatureMonthInput.value)][id]).toFixed(2)} m/s`],
      ['Eastward wind component', `${windNormals.monthlyEastMetersPerSecond[Number(temperatureMonthInput.value)][id].toFixed(2)} m/s`],
      ['Northward wind component', `${windNormals.monthlyNorthMetersPerSecond[Number(temperatureMonthInput.value)][id].toFixed(2)} m/s`],
    ] : []),
    ...(waterFrame ? [
      ['Displayed prescribed-water depth', `${number.format(waterFrame.depthMeters[id])} m · approximate view`],
      ['Displayed water body', waterFrame.bodyIds[id] ? `Body ${waterFrame.bodyIds[id]}` : 'Dry land'],
    ] : []),
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
  const eroded = world.recipe.modelVersion === MULTISCALE_ERODED_MODEL_VERSION;
  const lowlands = world.recipe.modelVersion === MULTISCALE_LOWLANDS_MODEL_VERSION || eroded;
  const belts = world.recipe.modelVersion === MULTISCALE_BELTS_MODEL_VERSION || lowlands;
  const continentalUplands = world.recipe.modelVersion === MULTISCALE_UPLANDS_MODEL_VERSION || belts;
  const multiscaleGeography = world.recipe.modelVersion === MULTISCALE_GEOGRAPHY_MODEL_VERSION || continentalUplands;
  for (const [label, value] of [['Crust baseline', world.terrain.baseline[id]], [belts ? 'Mountain-belt uplift' : 'Convergence uplift', world.terrain.convergence[id]],
    ['Divergence (ridge − rift)', world.terrain.divergence[id]], [multiscaleGeography ? 'Conditioned multiscale detail' : 'Bounded detail', world.terrain.detail[id]],
    [belts ? 'Dry preparation + filled plains' : 'Dry preparation', world.terrain.preparation[id]],
    ...(world.terrain.lowland ? [['Coastal lowland', world.terrain.lowland[id]] as const] : []),
    ...(world.terrain.seafloor ? [['Ocean floor', world.terrain.seafloor[id]] as const] : []),
    ...(world.terrain.erosion ? [['Erosion and deposition', world.terrain.erosion[id]] as const] : []), ['Total elevation', world.terrain.elevation[id]]] as const) {
    const row = document.createElement('li'); row.textContent = `${label}: ${value.toFixed(2)} m`; elevationDetails.append(row);
  }
  element('crust-note').textContent = multiscaleGeography
    ? `Seeded smooth 3D value-noise fBm with a bounded tangent warp gives potential ${world.crust.potential[id].toFixed(6)}. Threshold ${world.crust.threshold.toFixed(6)} uses a fixed subdivision-5 reference sphere (global bounds for 0%/100%), not refitting to the displayed mesh; sampled continental coverage can differ from the target. The reference fit and fine bands have finite sampling limits. Smooth transition width is 0.12. Continentality blends the 7–35 km thickness and 3,000–2,800 kg/m³ density endmembers. This is a procedural initial-condition heuristic, not reconstructed geological history, elevation or water depth.`
    : `Seeded spherical potential ${world.crust.potential[id].toFixed(6)}; fitted threshold ${world.crust.threshold.toFixed(6)}; smooth transition width 0.12. Continentality blends the 7–35 km thickness and 3,000–2,800 kg/m³ density endmembers. These are initial model approximations, not elevation or water depth.`;
  element('elevation-note').textContent = multiscaleGeography
    ? 'Crust baseline, static plate response, tectonic-envelope-conditioned multiscale ridging/detail, and bounded dry preparation sum to the native elevation. Detail uses smooth 3D value noise; it is physical initial roughness, not extra display geometry. These are heuristics, not geological history or computed river erosion. Display exaggeration changes none of the fields.'
    : 'Crust, plate response, detail, and dry preparation sum to the native elevation. Display exaggeration changes none of them. No geological age or climate erosion is modeled.';
  if (continentalUplands) {
    const interior = Math.max(world.crust.potential[id] - world.crust.threshold, 0);
    const extra = world.crust.thicknessMeters[id] - (7000 + 28000 * world.crust.continentality[id]);
    element('crust-note').textContent = `Version-1 smooth 3D geographic fields and fixed subdivision-5 reference sphere are retained. Continental potential excess ${interior.toFixed(6)} supplies ${(extra / 1000).toFixed(3)} km of additional initial crust thickness through 15 km × continentality × excess / (excess + 0.25). Density is unchanged. Thickness is bounded by 7–50 km; the common color palette saturates above 35 km. This is an initial continental-interior heuristic, not coast distance, crust age or simulated tectonic thickening.`;
    element('elevation-note').textContent = eroded
      ? 'Version 4 plus two explicit contributions. Erosion and deposition replace dry land by a steady-state stream-power landscape (uplift balanced by river incision) whose regional mean follows the version-4 bed, so the drainage network carves its own valleys; fluvial relief is damped on low ground. Shorelines do not move. The ocean floor (bathymetry-1) is still zero in this build. This is an initial-condition synthesis, not a simulated geological history. Display exaggeration changes no physical field.'
      : lowlands
      ? `Version-3 crust, mountain belts and filled plains, plus coastal lowlands: dry height above the initial water level that is not belt uplift is scaled by a kept fraction rising smoothly from ${COASTAL_LOWLAND.coast} at the shore to ${COASTAL_LOWLAND.interior} beyond ${COASTAL_LOWLAND.rampKm} km of graph distance from water, a proxy for passive-margin subsidence and coastal-plain sediment. Every dry region stays above the water level, so shorelines do not move. These are procedural initial conditions, not plate history, river erosion or isostasy. Display exaggeration changes no physical field.`
      : belts
      ? `Variable initial continental columns set the crust-buoyancy baseline. Convergent belts use graph distance to closing plate boundaries, looked up at a noise-warped direction, with noise-varied width and along-belt strength and ridged ranges; the continental side receives most uplift. After bounded dry preparation, closed dry depressions shallower than ${BELTS_MAX_FILL_METERS} m are filled to their spill level as sediment plains; deeper ones remain closed basins. These are procedural initial conditions, not plate history, river erosion or isostasy. Display exaggeration changes no physical field.`
      : 'Variable initial continental columns change the crust-buoyancy baseline. Static plate response, conditioned multiscale detail and bounded dry preparation are unchanged in law; their sum is the native bed. No depressions are secretly filled or breached. This is not geological history, river erosion or a solved isostatic equilibrium. Display exaggeration changes no physical field.';
  }
  element('water-note').textContent = waterFrame
    ? `Prescribed runoff follows the frozen initial drainage outlet ${world.drainage.outlets[id]}. The displayed level here is ${waterFrame.surfaceLevelsMeters[id].toFixed(2)} m and the approximate depth is ${waterFrame.depthMeters[id].toFixed(2)} m. Exact basin stocks remain in the native core; visible depth can lag tiny accepted inputs. No elapsed time or climate forcing is implied.`
    : `Initial level ${world.water.levelMeters.toFixed(2)} m − bed ${world.terrain.elevation[id].toFixed(2)} m → depth max(0, level − bed) = ${world.water.depthMeters[id].toFixed(2)} m. Regional stock: ${(world.water.depthMeters[id] * s.areasSquareMeters[id] / 1e9).toFixed(3)} km³ using reference-sphere area. Positive-depth neighbors form water bodies; disconnected bodies share only this initial level, not a permanent connection.`;
  const receiver = world.drainage.receivers[id], steps = world.drainage.flatSteps[id];
  element('drainage-note').textContent = receiver === id
    ? world.water.bodyIds[id] ? 'Initial water is a terminal receiver. Incoming land area is counted here; water-body area itself is excluded. The frozen receiver graph does not represent underwater flow.'
      : 'Initial closed dry sink: no lower exit from this equal-height component. A closed flat uses its smallest region ID as the analysis outlet. Manual water can fill this basin, but the bed and initial receiver graph stay unchanged.'
    : steps ? `Equal-height routing: ${steps} graph hops to a downhill exit or closed-flat sink. The receiver has one fewer hop. This deterministic tie-break is not a measured hydraulic gradient and changes no bed heights.`
      : `Steepest bed descent to region ${receiver}: ${(world.terrain.elevation[id] - world.terrain.elevation[receiver]).toFixed(2)} m drop over ${number.format(s.neighborDistancesMeters[s.neighborOffsets[id] + neighbors.indexOf(receiver)] / 1000)} km. Gradient ties prefer the smaller region ID. Area accumulation assumes connectivity only, not rain, travel time, or discharge.`;
  element('temperature-note').textContent = temperatureNormals
    ? `Latitude ${lat.toFixed(2)}° sets daily solar geometry for a ${temperatureNormals.settings.axialTiltDegrees}° tilt and 365-day circular orbit. Initial ${world.water.depthMeters[id] > 0 ? 'water' : 'dry land'} uses a ${world.water.depthMeters[id] > 0 ? temperatureNormals.settings.waterResponseDays : temperatureNormals.settings.landResponseDays}-day response time. ${world.water.depthMeters[id] > 0 ? 'Wet regions receive no bed-height lapse correction.' : `Positive bed elevation contributes a −${(temperatureNormals.settings.lapseRateCelsiusPerMeter * Math.max(0, world.terrain.elevation[id])).toFixed(1)} °C correction.`} The solar-to-temperature sensitivity is prescribed; no atmospheric heat transport, weather, ice feedback, or complete energy budget is modeled.`
    : 'Seasonal temperature normals are being calculated from initial geography.';
  element('climate-note').textContent = climateNormals ? climateExplanation(climateNormals, id, world.water.bodyIds[id] !== 0)
    : 'Climate normals are unavailable for this world.';
  renderVegetationNote(id);
  renderFoodNote(id);
  void requestVegetationSeries(id);
  renderTraversalNote(id);
  updateTravelButtons(id);
  renderTraversalRoute();
  element('wind-note').textContent = windNormals
    ? `Latitude ${lat.toFixed(2)}° and the selected month determine a prescribed east/north tangent vector. Trades are easterly, midlatitude winds westerly, and polar winds easterly; the convergence belt follows half the subsolar declination. These are smooth, zonally symmetric model choices, not simulated atmospheric circulation. Terrain and water do not redirect wind. Seasonal water uses these normals on its own elapsed-time clock.`
    : 'Seasonal surface-wind normals are being calculated.';
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
  workspace.inspectionChanged(!preserveBasin);
}

function inspectBasin(node: number): void {
  if (!world || selected === null) return;
  inspectedBasin = node;
  const b = world.basins, parent = b.parents[node], root = parent === node, children = basinChildren[node];
  map.setBasinContact(root ? -1 : b.spillFrom[node], root ? -1 : b.spillTo[node]);
  element('basin-title').textContent = `Branch ${node} · ${root ? 'global root' : children.length ? 'merged basin' : 'minimum basin'}`;
  element('basin-note').textContent = `Region ${selected} belongs to branch ${b.regionNodes[selected]}. Colors show exclusive branch ownership, not whole nested footprints. Cyan marks the threshold contact on basin layers; gold marks the selected region. Capacity includes descendant storage: do not sum it across the hierarchy. This does not measure present water or available empty capacity. Inspecting a parent does not change region selection.`;
  element<HTMLButtonElement>('basin-parent').disabled = root;
  element<HTMLButtonElement>('basin-owner').disabled = node === b.regionNodes[selected];
  const rows = element('basin-details'); rows.replaceChildren();
  const edge = root ? '' : `Region ${b.spillFrom[node]} ↔ region ${b.spillTo[node]}`;
  for (const [label, value] of [
    ['Parent branch', root ? 'None · closed planet' : `Branch ${parent}`],
    ['Child branches', children.length ? `${children.length}: ${children.slice(0, 12).join(', ')}${children.length > 12 ? ' …' : ''}` : 'None · minimum plateau'],
    ['Formation threshold', `${number.format(b.birthLevels[node])} m · reference datum`],
    ['Next connection threshold', root ? 'None · no external drain' : `${number.format(b.spillLevels[node])} m · not current water level`],
    ['Total capacity at threshold', root ? 'No finite spill capacity' : `${number.format(b.capacities[node] / 1e9)} km³ · includes children`],
    ['Subtree support area', `${number.format(b.supportAreas[node] / 1e6)} km² · not current wet area`],
    ['Threshold contact', root ? 'None' : `${edge} · adjacency witness, not a flow path`],
  ]) { const dt = document.createElement('dt'), dd = document.createElement('dd'); dt.textContent = label; dd.textContent = value; rows.append(dt, dd); }
}

element('basin-parent').addEventListener('click', () => { if (world && inspectedBasin !== null) inspectBasin(world.basins.parents[inspectedBasin]); });
element('basin-owner').addEventListener('click', () => { if (world && selected !== null) inspectBasin(world.basins.regionNodes[selected]); });

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
const navigation = new MapNavigation(map);

function updateLegend(): void {
  const thickenedColumns = !!world && [MULTISCALE_UPLANDS_MODEL_VERSION, MULTISCALE_BELTS_MODEL_VERSION, MULTISCALE_LOWLANDS_MODEL_VERSION].includes(world.recipe.modelVersion as never);
  const legends: Record<Exclude<Layer, keyof typeof MOISTURE_LAYERS>, [string, string, string]> = {
    surface: [
      `Land and water surface${hasRegionalSurface(moistureFrame) ? ' · regional seasonal liquid; initial reference heads prescribed' : waterFrame ? ' · prescribed-water display' : ''}${map.getSurfaceStyle() === 'natural'
        ? ` · natural color from vegetation, climate & relief${map.getDisplayDetail() ? ' · display-only detail & smooth shoreline' : ' · smoothed shoreline'}`
        : ' · hypsometric tint by elevation, not biomes · interpolated shoreline is a display approximation'}`,
      '', '',
    ],
    signal: ['Seed field · dimensionless diagnostic', '−1', '+1'],
    area: ['Region area · true spherical area', world ? `${number.format(world.stats.minimumAreaSquareMeters / 1e6)} km²` : 'min', world ? `${number.format(world.stats.maximumAreaSquareMeters / 1e6)} km²` : 'max'],
    latitude: ['Latitude · distance from the equator', '90°', '0°'],
    plates: [`${world?.recipe.plateCount ?? '—'} connected plates · colors identify plates, not continents`, '', ''],
    boundaries: ['Boundary motion · dominant component; oblique motion retained in inspector', '', ''],
    speed: ['Plate speed · model reference frame', '0', `${world?.recipe.maxPlateSpeedCmPerYear ?? '—'} cm/year`],
    crust: ['Continentality · initial crust, not land or ocean coverage', '0 · oceanic', '1 · continental'],
    thickness: [thickenedColumns ? 'Crust thickness · common palette saturates above 35 km; inspect full 7–50 km columns' : 'Crust thickness · initial approximation, not elevation', '7 km', thickenedColumns ? '≥35 km' : '35 km'],
    elevation: ['Elevation · reference datum, not sea level · world-relative color scale', `${number.format(terrainStats.minimumMeters)} m`, `${number.format(terrainStats.maximumMeters)} m`],
    uplift: ['Convergence uplift · strongest attenuated source, not accumulated history', '0 m', '12,000 m'],
    depth: [`${hasRegionalSurface(moistureFrame) ? 'Regional seasonal / prescribed initial-body depth' : waterFrame ? 'Prescribed-water display depth' : 'Initial water depth'} · gray = dry · globe shows the bed, not a water-surface mesh`, '0 m', `${number.format(maximumWaterDepth)} m`],
    waterBodies: [`${hasRegionalSurface(moistureFrame) ? 'Initial reference-body labels · not evolving regional connectivity' : waterFrame ? 'Connected displayed water bodies' : 'Connected water bodies'} · gray = dry · main ocean = body ${waterFrame?.mainOceanId || world?.water.mainOceanId || 'none'}`, '', ''],
    catchments: ['Drainage catchments · colors identify terminal outlets, not states or rivers', '', ''],
    contributingArea: ['Contributing dry-land area · logarithmic color scale · not river discharge', '0 km²', `${number.format(maximumContributingArea / 1e6)} km²`],
    basins: ['Basin branches · exclusive ownership, not full nested footprints or current lakes', '', ''],
    spill: ['Next basin connection threshold · not current water level · gray = root without external drain',
      basinStats?.minimumSpillMeters === null ? 'No finite thresholds' : `${number.format(basinStats?.minimumSpillMeters ?? 0)} m`,
      basinStats?.maximumSpillMeters === null ? '' : `${number.format(basinStats?.maximumSpillMeters ?? 0)} m`],
    temperature: [`Month ${Number(temperatureMonthInput.value) + 1} temperature normal · fixed initial geography, not weather`,
      `${number.format(temperatureRange.minimum)} °C`, `${number.format(temperatureRange.maximum)} °C`],
    biome: ['Biomes · climate classes from prescribed normals, not simulated vegetation', '', ''],
    annualPrecipitation: ['Annual precipitation · prescribed estimate, not the seasonal-water budget · logarithmic', '0 mm', '≥4,000 mm'],
    annualRunoff: ['Runoff generated locally · annual depth · log scale', '0 mm/year', '≥2,000 mm/year'],
    riverDischarge: ['Mean river discharge · log scale · blue perennial, teal seasonal', '300 m³/s', '≥30,000 m³/s'],
    vegetationState: [`Vegetation state · year ${vegetation?.year ?? 0} · accumulated cover with hysteresis; same class colors as Biomes`, '', ''],
    woodyCover: [`Woody cover · year ${vegetation?.year ?? 0} · fraction of the region under woody plants`, '0', '1'],
    woodyChange: [`Woody change · years ${Math.max(0, (vegetation?.year ?? 0) - CHANGE_YEARS)}–${vegetation?.year ?? 0} · drought losses and regrowth`, '−0.3', '+0.3'],
    productivity: [`Net primary productivity · year ${vegetation?.year ?? 0} · Miami model × realized cover`, '0', '≥2,500 g/m²/year'],
    precipitationAnomaly: [`Year ${vegetation?.year ?? 0} precipitation · fraction of the climate normal · land only`, '≤50%', '≥200%'],
    foragerCapacity: [`Forager capacity · year ${wildFood?.year ?? 0} · year-round wild plant food and game, no storage, no aquatic food · logarithmic`, '≤0.1', '≥100 people/100 km²'],
    leanSeason: [`Lean-season limit · year ${wildFood?.year ?? 0} · year-round capacity as a share of perfect-storage capacity`, '0 · storage decides', '1 · no seasonal limit'],
    gameStock: [`Game stock · year ${wildFood?.year ?? 0} · herbivore live weight relative to its forage capacity · gray = no game`, '0 · depleted', '1 · at capacity'],
    travelCost: [`Foot travel cost (${travelSeason}) · logarithmic · ${traversalCosts?.usesVegetation ? `vegetation year ${traversalCosts.vegetationYear ?? 0}` : 'climate biomes'} · ${DEFAULT_TRAVERSAL_SETTINGS.hoursPerDay} h/day`, '≤20 h/100 km', '≥160 h/100 km'],
    travelTime: [travelField
      ? `Foot travel from region ${number.format(travelField.sources[0])} · ${travelSeason} · contour lines mark days on foot`
      : travelOrigin !== null ? `Calculating ${travelSeason} travel time…`
        : `Foot travel (${travelSeason}) · select a region and choose “Travel time from here”`, '', ''],
    windSpeed: [`Month ${Number(temperatureMonthInput.value) + 1} prescribed surface-wind speed · no atmospheric dynamics`,
      '0 m/s', `${number.format(windMaximum)} m/s`],
  };
  const [title, low, high] = isMoistureLayer(currentLayer)
    ? [`${moistureLayerTitle(moistureFrame, currentLayer)} · fixed logarithmic scale · seasonal water at ${moistureFrame?.elapsedSeconds ?? 0} s`,
      `0 ${MOISTURE_LAYERS[currentLayer].unit}`, `≥ ${number.format(MOISTURE_LAYERS[currentLayer].maximum)} ${MOISTURE_LAYERS[currentLayer].unit}`]
    : legends[currentLayer];
  element('legend-title').textContent = title;
  element('legend-low').textContent = low;
  element('legend-high').textContent = high;
  const plateLayer = currentLayer === 'plates' || currentLayer === 'boundaries';
  element('legend-scale').hidden = plateLayer || currentLayer === 'waterBodies' || currentLayer === 'surface' || currentLayer === 'catchments' || currentLayer === 'basins' || currentLayer === 'biome' || currentLayer === 'vegetationState' || currentLayer === 'travelTime';
  element('biome-legend').hidden = currentLayer !== 'biome' && currentLayer !== 'vegetationState';
  travelFrameButton.hidden = currentLayer !== 'travelTime' || !travelField || map.navigation.mode !== 'globe';
  element('isochrone-legend').hidden = currentLayer !== 'travelTime' || travelField === null;
  element('legend-gradient').classList.toggle('travel-cost-gradient', currentLayer === 'travelCost');
  element('legend-gradient').classList.toggle('woody-gradient', currentLayer === 'woodyCover');
  element('legend-gradient').classList.toggle('anomaly-gradient', currentLayer === 'precipitationAnomaly');
  element('legend-gradient').classList.toggle('change-gradient', currentLayer === 'woodyChange');
  element('legend-gradient').classList.toggle('productivity-gradient', currentLayer === 'productivity');
  element('legend-gradient').classList.toggle('runoff-gradient', currentLayer === 'annualRunoff');
  element('legend-gradient').classList.toggle('discharge-gradient', currentLayer === 'riverDischarge');
  element('legend-gradient').classList.toggle('precipitation-gradient', currentLayer === 'annualPrecipitation');
  element('legend-gradient').classList.toggle('capacity-gradient', currentLayer === 'foragerCapacity');
  element('legend-gradient').classList.toggle('lean-gradient', currentLayer === 'leanSeason');
  element('legend-gradient').classList.toggle('game-gradient', currentLayer === 'gameStock');
  element('boundary-legend').hidden = !plateLayer;
  element('legend-gradient').classList.toggle('water-gradient', currentLayer === 'depth');
  element('legend-gradient').classList.toggle('temperature-gradient', currentLayer === 'temperature');
  element('legend-gradient').classList.toggle('wind-gradient', currentLayer === 'windSpeed');
}

/** Mirrors the native climate-normals-1 classifier so the inspector names the deciding rule. */
function climateExplanation(normals: ClimateNormals, id: number, wet: boolean): string {
  const mean = normals.annualMeanCelsius[id], warm = normals.warmestMonthCelsius[id], cold = normals.coldestMonthCelsius[id];
  const annual = normals.annualPrecipitationMm[id], threshold = aridityThreshold(mean);
  const f = (value: number): string => number.format(value);
  const precipitation = `Precipitation ${f(annual)} mm/year = monthly zonal potential (${f(normals.zonalPrecipitationMm[id])} mm/year for this latitude, shifted with the sun) × moisture supply ${(normals.moistureSupply[id] * 100).toFixed(0)}% × windward lift ×${normals.lift[id].toFixed(2)}. Supply decays with the cheapest path from water that follows the prescribed winds; paths against the wind and every meter of climbing cost extra (rain shadow).`;
  const temperature = `Temperatures come from a prescribed sea-level profile by latitude, a lagged seasonal insolation response (damped near windward water) and −6.5 °C/km above the water level.`;
  if (wet) return `Initial water region; no biome. ${precipitation}`;
  const rule = warm < 0 ? `Warmest month ${warm.toFixed(1)} °C is below 0 °C → ice.`
    : warm < 10 ? `Warmest month ${warm.toFixed(1)} °C is below 10 °C → tundra.`
    : annual < threshold ? `${f(annual)} mm is below the aridity threshold ${f(threshold)} mm (20 × mean + 140) → ${mean >= 18 ? 'hot' : 'cold'} desert.`
    : annual < 2 * threshold ? `${f(annual)} mm is between the aridity threshold ${f(threshold)} mm and twice it → ${mean >= 18 ? 'savanna' : 'temperate grassland'}.`
    : cold >= 18 ? `Humid and coldest month ${cold.toFixed(1)} °C ≥ 18 °C → tropical; driest month ${f(normals.driestMonthMm[id])} mm ${normals.driestMonthMm[id] >= 60 ? '≥' : '<'} 60 mm → ${normals.driestMonthMm[id] >= 60 ? 'rainforest' : 'seasonal forest'}.`
    : cold > -3 || warm >= 22 ? `Humid; coldest month ${cold.toFixed(1)} °C above −3 °C or warmest ≥ 22 °C → temperate forest.`
    : `Humid; coldest month ${cold.toFixed(1)} °C and warmest ${warm.toFixed(1)} °C → boreal forest.`;
  return `${BIOMES[normals.biome[id]].name}. ${rule} ${precipitation} ${temperature} These are prescribed climate normals, not weather, a water budget or vegetation dynamics.`;
}

/** Polylines over [0, 1] in a fixed SVG viewBox; values outside are clamped. */
function drawChart(svg: SVGSVGElement, lines: Array<{ className: string; values: number[] }>, normal?: number): void {
  const [, , width, height] = svg.getAttribute('viewBox')!.split(' ').map(Number);
  const ns = 'http://www.w3.org/2000/svg';
  svg.replaceChildren();
  if (normal !== undefined) {
    const line = document.createElementNS(ns, 'line');
    line.setAttribute('class', 'normal');
    line.setAttribute('x1', '0'); line.setAttribute('x2', String(width));
    line.setAttribute('y1', String(height * (1 - normal))); line.setAttribute('y2', String(height * (1 - normal)));
    svg.append(line);
  }
  for (const { className, values } of lines) {
    const polyline = document.createElementNS(ns, 'polyline');
    polyline.setAttribute('class', className);
    polyline.setAttribute('points', values.map((v, i) => `${(i / Math.max(1, values.length - 1) * width).toFixed(2)},${(height * (1 - Math.min(1, Math.max(0, v)))).toFixed(2)}`).join(' '));
    svg.append(polyline);
  }
  svg.toggleAttribute('hidden', lines.every(l => l.values.length < 2));
}
/** Precipitation factor on a log scale: 50% → 0, normal → 0.5, 200% → 1. */
const factorScale = (factor: number): number => 0.5 + Math.log2(factor) / 2;

function renderVegetation(): void {
  const chart = svgElement('vegetation-chart');
  if (!vegetation || !world) {
    element('vegetation-time').textContent = climateNormals === null ? 'Starts after climate normals load.' : 'Not initialized.';
    chart.toggleAttribute('hidden', true); updateExperimentControls(); return;
  }
  const s = vegetation.summaries, last = s[s.length - 1];
  // Forest share drawn over its own range with five points of padding.
  const forest = s.map(x => x.forestShare), low = Math.max(0, Math.min(...forest) - 0.05), high = Math.max(...forest) + 0.05;
  element('vegetation-time').textContent = `Year ${vegetation.year}`;
  if (!last) {
    element('vegetation-summary').textContent = `Year 0 follows a ${DEFAULT_VEGETATION_SETTINGS.spinUpYears}-year spin-up on its own random stream, so it is a typical state rather than the capacity of normal precipitation. Advance to draw spatially coherent wet and dry years. Not saved in checkpoints; the recipe and year count reproduce a run exactly.`;
  } else {
    let driest = 0;
    for (let y = 1; y < s.length; y++) if (s[y].precipitationFactor < s[driest].precipitationFactor) driest = y;
    const food = wildFood?.year === vegetation.year ? wildFood.summaries[wildFood.summaries.length - 1] : null;
    const foodText = food ? ` · wild food for ${(food.capacityPeople / 1e6).toFixed(1)} M foragers year-round (game stock ${(food.gameRatio * 100).toFixed(0)}% of capacity)` : '';
    element('vegetation-summary').textContent = `Land precipitation this year ${(last.precipitationFactor * 100).toFixed(0)}% of normal · forest ${(last.forestShare * 100).toFixed(1)}% of land (range ${(Math.min(...forest) * 100).toFixed(1)}–${(Math.max(...forest) * 100).toFixed(1)}%) · ${(last.labelChanges / Math.max(1, world.water.bodyIds.filter(body => body === 0).length) * 100).toFixed(2)}% of land regions changed class this year · driest year so far ${driest + 1} at ${(s[driest].precipitationFactor * 100).toFixed(0)}% · land productivity ${last.productivityPetagrams.toFixed(0)} Pg dry matter/year (Earth about 115–125)${foodText}. Green: forest share between ${(low * 100).toFixed(0)}% and ${(high * 100).toFixed(0)}%; blue: land precipitation, 80–120% of normal (dashed = normal)${food ? '; orange: forager capacity over its own range' : ''}.`;
  }
  const capacity = wildFood?.year === vegetation.year ? wildFood.summaries.slice(1).map(x => x.capacityPeople) : [];
  const capacityLow = Math.min(...capacity) * 0.95, capacityHigh = Math.max(...capacity) * 1.05;
  drawChart(chart, [{ className: 'forest', values: s.map(x => (x.forestShare - low) / (high - low)) },
    { className: 'rain', values: s.map(x => 0.5 + (x.precipitationFactor - 1) * 2.5) },
    ...(capacity.length ? [{ className: 'capacity', values: capacity.map(x => (x - capacityLow) / Math.max(1, capacityHigh - capacityLow)) }] : [])], 0.5);
  updateExperimentControls();
}

function applyVegetation(frame: VegetationFrame): void {
  vegetation = frame; vegetationSeries = null; wildFoodSeries = null; forageResult = null; map.setVegetation(frame);
  for (const button of vegetationLayerButtons) button.disabled = false;
  renderVegetation(); updateLegend();
  if (selected !== null) inspect(selected, true);
}

/** Wild food follows the vegetation year; requested after every vegetation frame. */
async function loadWildFood(activeEpoch: number, request: number): Promise<void> {
  const frame = await api.wildFood(activeEpoch);
  if (request !== generationId || frame.epoch !== epoch) return;
  wildFood = frame; map.setWildFood(frame);
  for (const button of foodLayerButtons) button.disabled = false;
  renderVegetation(); updateLegend();
  if (selected !== null) renderFoodNote(selected);
}

async function runVegetation(years: number | 'reset'): Promise<void> {
  if (!world || !vegetation || vegetationBusy || document.body.dataset.state === 'generating') return;
  const activeEpoch = epoch, request = generationId;
  vegetationBusy = true; updateTravelButtons(selected); updateExperimentControls();
  const started = performance.now();
  try {
    const frame = years === 'reset' ? await api.initializeVegetation(activeEpoch) : await api.advanceVegetation(activeEpoch, years);
    if (request !== generationId || frame.epoch !== epoch) return;
    applyVegetation(frame);
    travelRequest++;
    resetTravelField();
    traversalCosts = null;
    map.setTraversalCosts(null);
    updateLegend();
    await refreshTraversalCosts(activeEpoch, request);
    await loadWildFood(activeEpoch, request);
    if (request !== generationId) return;
    vegetationBusy = false;
    updateTravelButtons(selected);
    if (selected !== null) void requestVegetationSeries(selected);
    showStatus(`Vegetation year ${frame.year} · native request + IPC ${number.format(performance.now() - started)} ms.`);
  } catch (error) {
    if (request === generationId) showStatus(error instanceof Error ? error.message : String(error), true);
  } finally {
    if (epoch === activeEpoch) {
      vegetationBusy = false;
      updateTravelButtons(selected);
    }
    updateExperimentControls();
  }
}
for (const { years, button } of vegetationButtons) button.addEventListener('click', () => void runVegetation(years));
vegetationReset.addEventListener('click', () => void runVegetation('reset'));

async function requestVegetationSeries(id: number): Promise<void> {
  const chart = svgElement('vegetation-region-chart');
  const request = ++seriesRequest;
  if (!vegetation || vegetation.year === 0 || world?.water.bodyIds[id] !== 0) { chart.toggleAttribute('hidden', true); return; }
  if (vegetationSeries?.region === id && vegetationSeries.years.length === vegetation.year + 1) return;
  // Native requests are serialized; skip while a run is in flight and refresh after it.
  if (vegetationBusy) return;
  try {
    const series = await api.vegetationSeries(epoch, id);
    if (request !== seriesRequest || series.epoch !== epoch || selected !== id) return;
    vegetationSeries = series; renderVegetationNote(id);
    // The native core reuses this region's replayed vegetation history.
    const food = await api.wildFoodSeries(epoch, id);
    if (request !== seriesRequest || food.epoch !== epoch || selected !== id) return;
    wildFoodSeries = food; renderFoodNote(id);
  } catch (error) {
    if (request === seriesRequest) element('vegetation-note').textContent = `Vegetation history unavailable: ${error instanceof Error ? error.message : String(error)}`;
  }
}

/** Mean year-round capacity of a region over its replayed history, people/km². */
const meanCapacity = (series: WildFoodSeries): number => series.years.reduce((a, y) => a + y.capacity, 0) / series.years.length;

async function runForage(multiple: number): Promise<void> {
  if (!world || !wildFood || selected === null || forageBusy || vegetationBusy || world.water.bodyIds[selected] !== 0) return;
  const id = selected, activeEpoch = epoch;
  if (wildFoodSeries?.region !== id || wildFoodSeries.years.length !== wildFood.year + 1) return;
  const people = Math.max(1, Math.round(multiple * meanCapacity(wildFoodSeries) * world.surface.areasSquareMeters[id] / 1e6));
  forageBusy = true; updateExperimentControls();
  try {
    const result = await api.forage(activeEpoch, id, people, 50);
    if (activeEpoch !== epoch || selected !== id) return;
    forageResult = result; renderFoodNote(id);
  } catch (error) {
    element('forage-note').hidden = false;
    element('forage-note').textContent = `Foraging experiment unavailable: ${error instanceof Error ? error.message : String(error)}`;
  } finally {
    forageBusy = false; updateExperimentControls();
  }
}
for (const button of forageButtons) button.addEventListener('click', () => void runForage(Number(button.dataset.multiple)));

/** Explains the wild-food-1 supply of one region from the frame and its replayed history. */
function renderFoodNote(id: number): void {
  const list = element('food-details'), chart = svgElement('food-region-chart'), forageChart = svgElement('forage-chart');
  list.replaceChildren();
  const hide = (text: string): void => {
    element('food-note').textContent = text; chart.toggleAttribute('hidden', true); forageChart.toggleAttribute('hidden', true);
    element('food-forage').hidden = true; element('forage-note').hidden = true;
  };
  if (!world || !wildFood || !climateNormals || wildFood.year !== vegetation?.year) { hide('Wild food starts with vegetation.'); return; }
  if (world.water.bodyIds[id] !== 0) { hide('Initial water region; aquatic food is not modeled.'); return; }
  const f = wildFood, settings = DEFAULT_WILD_FOOD_SETTINGS, area = world.surface.areasSquareMeters[id] / 1e6;
  const per100 = (x: number): string => (x * 100).toFixed(2), pct = (x: number): string => `${(x * 100).toFixed(0)}%`;
  const mcal = (kcalPerSquareMeter: number): string => number.format(Math.round(kcalPerSquareMeter * 1e3));
  const series = wildFoodSeries?.region === id && wildFoodSeries.years.length === f.year + 1 ? wildFoodSeries.years : null;
  const ratio = f.storedCapacity[id] > 0 ? f.capacity[id] / f.storedCapacity[id] : 0;
  const total = f.plantKcal[id] + f.gameKcal[id];
  const limit = f.storedCapacity[id] <= 0 ? 'no food: no productive cover'
    : ratio > 0.999 ? 'total food decides: game covers the months when plants run short'
    : `the lean season decides: plants run short around ${MONTH_NAMES[f.leanMonth[id]]} and the game offtake cannot cover it`;
  element('food-note').textContent = `${per100(f.capacity[id])} foragers per 100 km² year-round (${number.format(Math.round(f.capacity[id] * area))} in this region) at year ${f.year}: ${limit}.`;
  const cold = climateNormals.coldestMonthCelsius[id], [warmNeed, coldNeed] = COLD_NEED_CELSIUS;
  const coldShare = Math.min(1, Math.max(0, (warmNeed - cold) / (warmNeed - coldNeed)));
  const rows = [
    `Plant food ${mcal(f.plantKcal[id])} Mcal/km²/year (${total > 0 ? pct(f.plantKcal[id] / total) : '0%'} of energy): ${settings.herbEdibleShare * 100}% of herb and ${settings.woodyEdibleShare * 100}% of woody productivity is harvestable food at ${settings.plantEnergyKcalPerGram} kcal/g; a closed canopy hides up to ${pct(settings.canopyPenalty)} of tree food. Standing food loses ${pct(settings.plantSpoilagePerMonth)} per month.`,
    `Game ${(f.game[id] * 1e3).toFixed(0)} kg/km² live weight, capacity ${(f.gameCapacity[id] * 1e3).toFixed(0)} kg/km² from forage (herbs plus ${pct(settings.browseShare)} of woody growth as browse; ${settings.gameCapacityGrams * 1e3} kg/km² at ${settings.gameReferenceForage} g/m²/year, exponent ${settings.gameCapacityExponent}, remembered over ${settings.gameMemoryYears} years). Grows ${settings.gameGrowthPerYear}/year, loses ${pct(settings.gameDiebackPerYear)} of any excess per year.`,
    `Sustainable offtake ${mcal(f.gameKcal[id])} Mcal/km²/year: logistic production at up to half the capacity, ${pct(settings.gameHumanShare)} left to people by other predators, hunting success ${pct(f.access[id])} under this canopy, ${settings.gameEnergyKcalPerGram} kcal per gram of live weight.`,
    `Need ${number.format(settings.needKcalPerDay)} kcal/person/day, up to ${pct(settings.coldNeedIncrease)} more in cold months (coldest month ${cold.toFixed(1)} °C → +${pct(settings.coldNeedIncrease * coldShare)}). With perfect storage ${per100(f.storedCapacity[id])} per 100 km²; the year-round value is ${pct(ratio)} of that.`,
  ];
  if (series && series.length > 1) {
    const season = series[series.length - 1].season;
    const peak = season.indexOf(Math.max(...season));
    let low = 1;
    for (let y = 2; y < series.length; y++) if (series[y].capacity < series[low].capacity) low = y;
    let recovered = -1;
    for (let y = low; y < series.length; y++) if (series[y].capacity >= 0.95 * series[0].capacity) { recovered = y; break; }
    rows.push(`Plant food peaks in ${MONTH_NAMES[peak]} (${pct(season[peak])} of the year's growth). Worst year ${low}: ${per100(series[low].capacity)} per 100 km², ${pct(series[0].capacity > 0 ? series[low].capacity / series[0].capacity : 0)} of year 0; ${recovered >= 0 ? `back within 5% by year ${recovered}.` : 'not yet back within 5%.'} Chart: faint orange yearly capacity, orange its ${FOOD_MEAN_YEARS}-year mean, green game stock as a share of capacity.`);
    const high = Math.max(...series.map(y => y.capacity)) * 1.05 || 1;
    const mean = series.map((_, y) => { const window = series.slice(Math.max(0, y - FOOD_MEAN_YEARS + 1), y + 1); return window.reduce((a, b) => a + b.capacity, 0) / window.length; });
    drawChart(chart, [{ className: 'capacity yearly', values: series.map(y => y.capacity / high) },
      { className: 'capacity', values: mean.map(c => c / high) },
      { className: 'game', values: series.map(y => y.gameCapacity > 0 ? y.game / y.gameCapacity : 0) }]);
  } else {
    if (f.year > 0) rows.push('History: waiting for the region replay…');
    chart.toggleAttribute('hidden', true);
  }
  rows.push('Not modeled: fish and shellfish, human storage, fire, game migration, competition among species.');
  for (const text of rows) { const row = document.createElement('li'); row.textContent = text; list.append(row); }
  element('food-forage').hidden = f.year === 0 || !series || series.every(y => y.capacity <= 0);
  const result = forageResult?.region === id && forageResult.game.length === f.year + 1 && series ? forageResult : null;
  element('forage-note').hidden = !result && f.year > 0;
  if (f.year === 0) element('forage-note').textContent = 'Advance vegetation to run a foraging experiment over the recorded years.';
  if (result && series) {
    const end = Math.min(result.years, f.year), base = series.map(y => y.game), unmet = result.shortfall.slice(1, end + 1);
    let back = -1;
    for (let y = end + 1; y < result.game.length; y++) if (result.game[y] >= 0.9 * base[y]) { back = y; break; }
    element('forage-note').textContent = `${number.format(result.people)} people (${number.format(result.people / Math.max(1, meanCapacity(wildFoodSeries!) * area))}× the mean year-round capacity) foraged here in years 1–${end}: game ${pct(base[end] > 0 ? result.game[end] / base[end] : 0)} of the untouched stock at the end, average unmet need ${pct(unmet.reduce((a, b) => a + b, 0) / Math.max(1, unmet.length))}. ${end >= f.year ? 'Advance further to see recovery after they leave.' : back >= 0 ? `Game back within 10% of the untouched stock ${back - end} years after they left.` : 'Game has not yet recovered.'} Chart: green with the group, dashed without, red unmet need.`;
    const high = Math.max(...base, ...result.game) * 1.05 || 1;
    drawChart(forageChart, [{ className: 'baseline', values: base.map(g => g / high) }, { className: 'game', values: result.game.map(g => g / high) },
      { className: 'shortfall', values: result.shortfall }]);
  } else forageChart.toggleAttribute('hidden', true);
}

/** Mirrors the native vegetation-2 rules so the inspector names the deciding state. */
function renderVegetationNote(id: number): void {
  const chart = svgElement('vegetation-region-chart'), list = element('vegetation-details');
  list.replaceChildren();
  if (!world || !vegetation || !climateNormals) {
    element('vegetation-note').textContent = 'Vegetation starts after climate normals load.'; chart.toggleAttribute('hidden', true); return;
  }
  if (world.water.bodyIds[id] !== 0) {
    element('vegetation-note').textContent = 'Initial water region; no vegetation.'; chart.toggleAttribute('hidden', true); return;
  }
  const v = vegetation, settings = DEFAULT_VEGETATION_SETTINGS, f = (x: number): string => x.toFixed(2), pct = (x: number): string => `${(x * 100).toFixed(0)}%`;
  const label = v.label[id], warm = climateNormals.warmestMonthCelsius[id], mean = climateNormals.annualMeanCelsius[id];
  const normal = climateNormals.annualPrecipitationMm[id], threshold = aridityThreshold(mean);
  const ratio = (factor: number): string => threshold <= 0 ? 'unlimited' : f(normal * factor / threshold);
  const forest = [3, 4, 9, 10].includes(label), open = label === 5 || label === 8;
  const rule = warm < 0 ? `warmest month ${warm.toFixed(1)} °C < 0 °C → ice regardless of cover`
    : warm < 10 ? `warmest month ${warm.toFixed(1)} °C < 10 °C → tundra; no trees, herbs scaled by summer warmth`
    : forest ? `woody cover ${f(v.woody[id])} ≥ ${FOREST_COVER[1]} → forest (starts at ${FOREST_COVER[0]}, holds down to ${FOREST_COVER[1]}); climate picks the type`
    : open ? `woody below the forest threshold; established cover ${f(v.establishedCover[id])} ≥ ${OPEN_COVER[1]} → ${mean >= 18 ? 'savanna' : 'grassland'} (${settings.labelMemoryYears}-year memory)`
    : `established cover ${f(v.establishedCover[id])} below ${OPEN_COVER[0]} (or fell below ${OPEN_COVER[1]}) → ${mean >= 18 ? 'hot' : 'cold'} desert`;
  const climate = climateNormals.biome[id] !== label ? ` Climate class: ${BIOMES[climateNormals.biome[id]].name}.` : '';
  element('vegetation-note').textContent = `${BIOMES[label].name} at year ${v.year}: ${rule}.${climate}`;
  const s = world.surface;
  let neighbors = 0;
  for (let k = s.neighborOffsets[id]; k < s.neighborOffsets[id + 1]; k++) neighbors += v.woody[s.neighbors[k]];
  neighbors /= Math.max(1, s.neighborOffsets[id + 1] - s.neighborOffsets[id]);
  const [wet, dry] = VARIABILITY_PRECIPITATION_MM;
  const sigma = settings.humidVariability + Math.min(1, Math.max(0, Math.log10(wet / Math.max(normal, Number.MIN_VALUE)) / Math.log10(wet / dry)))
    * (settings.aridVariability - settings.humidVariability);
  const [heat, water] = miami(mean, normal * v.precipitationFactor[id]);
  const reference = Math.max(MINIMUM_REFERENCE_COVER, coverCapacityAtNormal(id)), cover = v.woody[id] + v.herb[id];
  const rows = [
    `Woody ${f(v.woody[id])} → capacity ${f(v.woodyCapacity[id])}: root-zone moisture ${pct(v.rootMoisture[id])} of normal, aridity ${ratio(v.rootMoisture[id])} (0 at ${WOODY_ARIDITY[0]}, full at ${WOODY_ARIDITY[1]}). Grows ${settings.woodyGrowthPerYear}/year, loses ${pct(settings.woodyDiebackPerYear)} of any excess per year.`,
    `Herbs ${f(v.herb[id])} fill total cover capacity ${f(v.coverCapacity[id])}: this year ${pct(v.precipitationFactor[id])} of normal, aridity ${ratio(v.precipitationFactor[id])} (0 at ${COVER_ARIDITY[0]}, full at ${COVER_ARIDITY[1]}).`,
    `Seeds: neighbors' woody cover ${f(neighbors)} gives establishment ${(settings.backgroundEstablishment + settings.seedEstablishment * neighbors).toFixed(3)} for next year; bare land fills from wooded edges.`,
    `Wet and dry years: about ±${pct(Math.expm1(sigma))} in a typical year here (${number.format(Math.round(normal))} mm normal; drier climates vary more).`,
    `Productivity ${number.format(Math.round(v.productivity[id]))} g/m²/year: Miami limits ${number.format(Math.round(heat))} (temperature) and ${number.format(Math.round(water))} (this year's precipitation), the smaller × ${pct(Math.min(1, cover / reference))} of normal cover realized.`,
    `Woody change over ${CHANGE_YEARS} years: ${v.woodyChange[id] >= 0 ? '+' : ''}${f(v.woodyChange[id])}.`,
  ];
  const series = vegetationSeries?.region === id && vegetationSeries.years.length === v.year + 1 ? vegetationSeries.years : null;
  if (series && series.length > 1) {
    let driest = 1;
    for (let y = 2; y < series.length; y++) if (series[y].precipitationFactor < series[driest].precipitationFactor) driest = y;
    const before = series[driest - 1].woody;
    let low = driest;
    for (let y = driest; y < Math.min(series.length, driest + 8); y++) if (series[y].woody < series[low].woody) low = y;
    let recovered = -1;
    for (let y = low; y < series.length; y++) if (series[y].woody >= before - 0.05) { recovered = y; break; }
    const loss = before - series[low].woody;
    rows.push(`History: driest year ${driest} at ${pct(series[driest].precipitationFactor)} of normal. ${loss > 0.02
      ? `Woody cover fell from ${f(before)} to ${f(series[low].woody)} by year ${low}; ${recovered >= 0 ? `back within 0.05 by year ${recovered} (${recovered - low} years).` : 'not yet recovered.'}`
      : 'Woody cover barely changed.'} Chart: green woody, yellow herbs, blue precipitation (log scale, dashed = normal).`);
    drawChart(chart, [{ className: 'rain', values: series.map(y => factorScale(y.precipitationFactor)) },
      { className: 'herb', values: series.map(y => y.herb) }, { className: 'woody', values: series.map(y => y.woody) }], 0.5);
  } else {
    if (v.year > 0) rows.push(`History: replaying ${v.year} years in the native core…`);
    chart.toggleAttribute('hidden', true);
  }
  rows.push('Not modeled: fire, grazing, soils, temperature anomalies, feedback to water.');
  for (const text of rows) { const row = document.createElement('li'); row.textContent = text; list.append(row); }
}

/** Total cover capacity under normal precipitation, as in the native model. */
function coverCapacityAtNormal(id: number): number {
  if (!climateNormals || climateNormals.warmestMonthCelsius[id] < 0) return 0;
  const mean = climateNormals.annualMeanCelsius[id], threshold = aridityThreshold(mean);
  const ratio = threshold <= 0 ? Infinity : climateNormals.annualPrecipitationMm[id] / threshold;
  const t = Math.min(1, Math.max(0, (ratio - COVER_ARIDITY[0]) / (COVER_ARIDITY[1] - COVER_ARIDITY[0]))), cover = t * t * (3 - 2 * t);
  const warm = climateNormals.warmestMonthCelsius[id];
  return warm < 10 ? cover * warm / 10 : cover;
}

/* --- traversal --- */
/** Seasonal discharge (m³/s) implied by a river-crossing delay; the native rule inverted. */
const crossingDischarge = (c: TraversalCosts, hours: number): number =>
  c.settings.riverThresholdCubicMetersPerSecond * 10 ** (hours / c.settings.riverCrossingHoursPerDecade);
function updateTravelButtons(id: number | null): void {
  const ready = traversalCosts !== null && id !== null;
  travelFromHereButton.hidden = !ready;
  travelFromHereButton.disabled = !ready || vegetationBusy || !isRegionPassable(traversalCosts!, id!, travelSeason);
  travelFromHereButton.textContent = travelOrigin !== null && id === travelOrigin
    ? 'Origin region · recompute' : 'Travel time from here';
  travelClearSourceButton.hidden = travelOrigin === null;
}

function renderTraversalRoute(): void {
  const route = travelField && selected !== null ? traceTravelRoute(travelField, selected) : null;
  map.setTravelRoute(route ?? []);
  element('isochrone-legend').classList.toggle('has-route', (route?.length ?? 0) > 1);
}

function traversalRegionRows(id: number): Array<[string, string]> {
  const c = traversalCosts!;
  const summer = travelSeason === 'summer';
  const passable = isRegionPassable(c, id, travelSeason);
  const hours = summer ? c.hoursPer100KmSummer[id] : c.hoursPer100KmWinter[id];
  const slope = summer ? c.slopeFactorSummer[id] : c.slopeFactorWinter[id];
  const cover = summer ? c.coverFactorSummer[id] : c.coverFactorWinter[id];
  const snow = summer ? 1 : c.snowFactorWinter[id];
  const river = summer ? c.riverDelaySummerHours[id] : c.riverDelayWinterHours[id];
  const kmPerDay = passable && hours > 0 ? (100 * c.settings.hoursPerDay) / hours : 0;
  const rows: Array<[string, string]> = [
    [`Travel cost (${travelSeason})`, passable ? `${hours.toFixed(1)} h / 100 km` : 'Impassable on foot'],
    [`Walking pace (${travelSeason})`, passable ? `${kmPerDay.toFixed(1)} km/day · ${(100 / hours).toFixed(2)} km/h` : 'Blocked by water'],
    ['Slope & ruggedness', passable ? `×${slope.toFixed(2)} · local relief ${number.format(c.localReliefMeters[id])} m` : '—'],
    ['Land cover factor', passable ? `×${cover.toFixed(2)} · ${isFrozenLakeWinter(c, id) && !summer ? 'frozen inland lake' : c.usesVegetation ? `vegetation year ${c.vegetationYear}` : 'climate biome'}` : '—'],
    ['Snow factor', summer ? '×1.00 · summer (snow-free)' : `×${snow.toFixed(2)} · ${freezingMonths(c, id)} months < 0 °C`],
    ['River crossing', isFrozenRiverWinter(c, id) && !summer ? '0 h · frozen river' : river > 0 ? `+${river.toFixed(1)} h · about ${number.format(Math.round(crossingDischarge(c, river)))} m³/s` : '0 h · wadeable'],
  ];
  if (travelField && travelOrigin !== null) {
    const days = travelField.days[id], distKm = travelField.distanceMeters[id] / 1000;
    if (days >= 0) {
      rows.push(
        ['Travel time from origin', id === travelOrigin ? `0 days (origin · region ${travelOrigin})` : `${days.toFixed(1)} days from region ${travelOrigin}`],
        ['Least-time route length', id === travelOrigin ? '0 km' : `${number.format(distKm)} km`],
        ['Average route pace', id === travelOrigin || days <= 0 ? '—' : `${(distKm / days).toFixed(1)} km/day`],
      );
    } else {
      rows.push(['Travel time from origin', `Unreachable on foot (water) from region ${travelOrigin}`]);
    }
  }
  return rows;
}

function renderTraversalNote(id: number): void {
  const list = element('traversal-details');
  list.replaceChildren();
  if (!world || !traversalCosts) {
    element('traversal-note').textContent = 'Movement costs start after climate normals load.';
    return;
  }
  const c = traversalCosts, summer = travelSeason === 'summer', passable = isRegionPassable(c, id, travelSeason);
  const body = world.water.bodyIds[id], isOcean = body !== 0 && body === world.water.mainOceanId;
  const isLake = body !== 0 && !isOcean;
  if (!passable) {
    element('traversal-note').textContent = isOcean
      ? `Main ocean (body ${body}): impassable on foot in both summer and winter. Sea ice is not modeled.`
      : isLake
        ? `Inland lake (body ${body}): impassable in ${travelSeason}${!summer ? ` (freezes only when coldest month ≤ ${c.settings.freezeTempCelsius} °C with ≥ ${c.settings.freezeMinMonths} freezing months)` : '; may freeze in deep winter'}.`
        : 'Impassable on foot in this season.';
    return;
  }
  const hours = summer ? c.hoursPer100KmSummer[id] : c.hoursPer100KmWinter[id];
  const slope = summer ? c.slopeFactorSummer[id] : c.slopeFactorWinter[id];
  const cover = summer ? c.coverFactorSummer[id] : c.coverFactorWinter[id];
  const snow = summer ? 1 : c.snowFactorWinter[id];
  const river = summer ? c.riverDelaySummerHours[id] : c.riverDelayWinterHours[id];
  const kmPerDay = (100 * c.settings.hoursPerDay) / hours;
  element('traversal-note').textContent = `${summer ? 'Summer' : 'Winter'} foot cost ${hours.toFixed(1)} h / 100 km (${kmPerDay.toFixed(1)} km/day at ${c.settings.hoursPerDay} h/day, ${(100 / hours).toFixed(2)} km/h).`;
  const rows: string[] = [
    `Slope & ruggedness ×${slope.toFixed(2)}: Tobler hiking function over edge elevation gradients plus neighborhood relief ${number.format(c.localReliefMeters[id])} m (scale ${c.settings.reliefScaleMeters} m; no sub-grid roughness field is stored).`,
    isFrozenLakeWinter(c, id) && !summer
      ? `Land cover ×${cover.toFixed(2)}: deep-winter frozen inland lake ice (body ${body}; impassable in summer).`
      : `Land cover ×${cover.toFixed(2)}: ${c.usesVegetation && vegetation ? `uses live vegetation-2 woody cover ${vegetation.woody[id].toFixed(2)} (year ${c.vegetationYear})` : 'uses climate-normals-1 biome class fallback'} plus climate heat/aridity/ice factor.`,
    summer
      ? `Seasonal snow ×1.00: local summer (seasonal snow melted; ${freezingMonths(c, id)} months below 0 °C in winter).`
      : `Seasonal snow ×${snow.toFixed(2)}: ${freezingMonths(c, id)} months below 0 °C in climate normals.`,
    isFrozenRiverWinter(c, id) && !summer
      ? 'River crossing: 0 h delay — the river freezes solid in deep winter.'
      : river > 0
        ? `River crossing: +${river.toFixed(1)} h for a ${travelSeason} discharge of about ${number.format(Math.round(crossingDischarge(c, river)))} m³/s (mean of the three ${summer ? 'warmest' : 'coldest'} months from rivers-1; ${c.settings.riverCrossingHoursPerDecade} h per tenfold above ${c.settings.riverThresholdCubicMetersPerSecond} m³/s; ${number.format(c.settings.alongRiverFactor * 100)}% when walking along the river).`
        : `River crossing: 0 h delay — ${travelSeason} discharge here is below the ${c.settings.riverThresholdCubicMetersPerSecond} m³/s wading threshold${rivers ? ` (mean annual ${number.format(Math.round(rivers.meanDischarge[id]))} m³/s)` : ''}.`,
  ];
  if (travelField && travelOrigin !== null) {
    const days = travelField.days[id], distKm = travelField.distanceMeters[id] / 1000;
    const route = traceTravelRoute(travelField, id);
    rows.push(days >= 0
      ? id === travelOrigin
        ? `Origin: region ${travelOrigin} (0 days). Click another region to trace the least-time route.`
        : `Route from region ${travelOrigin}: ${days.toFixed(1)} days over ${number.format(distKm)} km (${Math.max(0, (route?.length ?? 1) - 1)} edges, avg ${(distKm / days).toFixed(1)} km/day).`
      : `Route from region ${travelOrigin}: unreachable on foot (water).`);
  }
  for (const text of rows) { const row = document.createElement('li'); row.textContent = text; list.append(row); }
}

async function refreshTraversalCosts(activeEpoch: number, request: number): Promise<void> {
  try {
    const costs = await api.traversalCosts(activeEpoch);
    if (request !== generationId || costs.epoch !== epoch) return;
    traversalCosts = costs;
    map.setTraversalCosts(costs);
    for (const button of traversalLayerButtons) button.disabled = false;
    updateTravelButtons(selected);
    updateLegend();
    if (selected !== null) inspect(selected, true);
    if (travelOrigin !== null) await requestTravelTime(travelOrigin, false);
  } catch (error) {
    if (request === generationId) element('traversal-note').textContent = `Movement costs unavailable: ${error instanceof Error ? error.message : String(error)}`;
  }
}

async function requestTravelTime(origin: number, switchLayer = true): Promise<void> {
  if (!world || !traversalCosts || document.body.dataset.state === 'generating') return;
  if (!isRegionPassable(traversalCosts, origin, travelSeason)) return;
  const activeEpoch = epoch, generation = generationId, season = travelSeason, req = ++travelRequest;
  const costs = traversalCosts;
  travelOrigin = origin;
  resetTravelField();
  if (switchLayer && currentLayer !== 'travelTime') selectLayer('travelTime');
  updateTravelButtons(selected);
  updateLegend();
  if (selected !== null) inspect(selected, true);
  const started = performance.now();
  try {
    const field = await api.travelTime(activeEpoch, season, [origin]);
    if (req !== travelRequest || generation !== generationId || field.epoch !== epoch || costs !== traversalCosts
      || field.vegetationYear !== costs.vegetationYear) return;
    travelField = field;
    map.setTravelField(field);
    updateLegend();
    if (selected !== null) inspect(selected, true);
    const reachable = field.days.reduce((n, d) => n + (d >= 0 ? 1 : 0), 0);
    showStatus(`Travel time (${travelSeason}) from region ${origin}: ${reachable.toLocaleString('en')} reachable regions · ${number.format(performance.now() - started)} ms.`);
  } catch (error) {
    if (req === travelRequest && generation === generationId) {
      clearTravelOrigin();
      showStatus(error instanceof Error ? error.message : String(error), true);
    }
  }
}

function setTravelSeason(season: TravelSeason): void {
  if (season === travelSeason) return;
  travelSeason = season;
  resetTravelField();
  for (const button of travelSeasonButtons) {
    const active = button.dataset.travelSeason === season;
    button.classList.toggle('active', active);
    button.setAttribute('aria-pressed', String(active));
  }
  map.setTravelSeason(season);
  updateTravelButtons(selected);
  updateLegend();
  if (selected !== null) inspect(selected, true);
  if (travelOrigin === null) return;
  if (traversalCosts && !isRegionPassable(traversalCosts, travelOrigin, season)) {
    clearTravelOrigin();
    showStatus('The previous origin is blocked by water in this season. Select a new origin.');
  } else {
    void requestTravelTime(travelOrigin, false);
  }
}

function resetTravelField(): void {
  travelField = null;
  map.setTravelField(null);
  renderTraversalRoute();
}

function clearTravelOrigin(): void {
  travelRequest++;
  travelOrigin = null;
  resetTravelField();
  updateTravelButtons(selected);
  updateLegend();
  if (selected !== null) inspect(selected, true);
}

travelFromHereButton.addEventListener('click', () => { if (selected !== null) void requestTravelTime(selected, true); });
travelClearSourceButton.addEventListener('click', clearTravelOrigin);
travelFrameButton.addEventListener('click', () => {
  if (!travelField || travelOrigin === null) return;
  const route = selected === null ? null : traceTravelRoute(travelField, selected);
  map.frameTravelRoute(route ?? [travelOrigin]);
});
for (const button of travelSeasonButtons) {
  button.addEventListener('click', () => setTravelSeason(button.dataset.travelSeason as TravelSeason));
}
/* --- end traversal --- */

function setInputs(recipe: Recipe): void {
  generatorFamilyInput.value = recipe.modelVersion;
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
  terrainPreparationInput.value = String(recipe.terrainPreparationPasses ?? 0);
  updateGeneratorInputs();
  waterModeInput.value = recipe.water.mode;
  if (recipe.water.mode === 'coverage') waterCoverageInput.value = String(recipe.water.fraction * 100);
  else waterVolumeInput.value = String(recipe.water.volumeCubicMeters / 1e9);
  updateWaterInputs();
}
function updateGeneratorInputs(): void {
  const legacy = generatorFamilyInput.value === LEGACY_MODEL_VERSION;
  terrainPreparationInput.disabled = legacy;
  const finest = maxSubdivision(generatorFamilyInput.value);
  for (const option of resolutionInput.options) option.disabled = Number(option.value) > finest;
  if (Number(resolutionInput.value) > finest) resolutionInput.value = String(finest);
  element('generator-note').textContent = generatorFamilyInput.value === MULTISCALE_ERODED_MODEL_VERSION
    ? 'Experimental. Version 4 with river-carved valleys and ridges from a steady-state stream-power landscape, and up to 163,842 regions. The ocean floor model is not implemented yet. Changing the model creates a different world.'
    : generatorFamilyInput.value === MULTISCALE_LOWLANDS_MODEL_VERSION
    ? 'Default for new worlds. Version 3 with coastal lowlands. Non-belt land height rises gradually with distance from water, so coastal plains are wider and interiors lower. Shorelines match version 3. Changing the model creates a different world.'
    : generatorFamilyInput.value === MULTISCALE_BELTS_MODEL_VERSION
    ? 'Continental uplands with meandering, varied mountain belts and sediment-filled shallow depressions for connected drainage. Changing the model creates a different world.'
    : generatorFamilyInput.value === MULTISCALE_UPLANDS_MODEL_VERSION
    ? 'Continental uplands reuse version-1 geographic structure with variable initial crust thickness. No geological history, river incision or automatic depression removal. Changing the model creates a different world.'
    : generatorFamilyInput.value === MULTISCALE_GEOGRAPHY_MODEL_VERSION
    ? 'Experimental multiscale crust and terrain fields, with continuous plate roots. No geological history, integrated river erosion, biomes or higher surface resolution is implied. Changing the model creates a different world.'
    : legacy ? 'Legacy terrain without dry preparation. Changing the model creates a different world, not an upgrade of an existing save.'
      : generatorFamilyInput.value === 'continuous-plates-1'
        ? 'Mesh-independent plate-root directions with the original crust and terrain formulas. Changing the model creates a different world.'
        : 'Original terrain with bounded dry preparation. Changing the model creates a different world, not an upgrade of an existing save.';
}
generatorFamilyInput.addEventListener('change', updateGeneratorInputs);
function updateWaterInputs(): void {
  const coverage = waterModeInput.value === 'coverage';
  element('water-coverage-control').hidden = !coverage; waterCoverageInput.disabled = !coverage;
  element('water-volume-control').hidden = coverage; waterVolumeInput.disabled = coverage;
}
waterModeInput.addEventListener('change', updateWaterInputs);
function updateExaggeration(): void {
  const raw = element<HTMLSelectElement>('exaggeration').value;
  if (raw === 'auto') {
    const applied = map.setExaggeration('auto');
    element('exaggeration-note').textContent = `Globe relief and water: ${number.format(applied)}× adaptive (3.5×–6.5× by zoom) · display only; flat map stays flat`;
    return;
  }
  const requested = Number(raw);
  const applied = map.setExaggeration(requested);
  element('exaggeration-note').textContent = `Globe relief and water: ${number.format(applied)}× applied${applied < requested ? ` (${requested}× requested; 20% radius display limit)` : ''} · display only; flat map stays flat`;
}
element('exaggeration').addEventListener('change', updateExaggeration);
element<HTMLSelectElement>('surface-style').addEventListener('change', (event) => {
  const style = (event.target as HTMLSelectElement).value === 'hypsometric' ? 'hypsometric' : 'natural';
  map.setSurfaceStyle(style);
  element<HTMLInputElement>('seasonal-snow').disabled = style !== 'natural' || temperatureNormals === null;
  updateLegend();
});
element<HTMLInputElement>('display-detail').addEventListener('change', (event) => {
  map.setDisplayDetail((event.target as HTMLInputElement).checked);
  updateLegend();
});
element<HTMLInputElement>('seasonal-snow').addEventListener('change', (event) => {
  map.setSeasonalSnow((event.target as HTMLInputElement).checked);
});
resolutionInput.addEventListener('change', () => { plateCountInput.max = String(Math.min(32, 10 * 4 ** Number(resolutionInput.value) + 2)); });

async function generate(recipe: Recipe | null, prepared?: PreparedWaterWorld | PreparedMoistureWorld | PreparedSoilWorld): Promise<void> {
  if (!prepared && (checkpointOpening || waterBusy)) return;
  pause(); map.cancelPreparation();
  const request = ++generationId;
  const start = performance.now();
  cancel.hidden = false; save.disabled = true; saveResolved.disabled = true;
  temperatureLayerButton.disabled = true; windLayerButton.disabled = true; temperatureMonthInput.disabled = true;
  exportView.disabled = true; waterAdd.disabled = true; waterSpill.disabled = true;
  waterOpen.disabled = true; waterSave.disabled = true; waterBudgetButton.disabled = true;
  showStatus(prepared ? 'Preparing restored world views…' : 'Building the surface in the native core…');
  document.body.dataset.state = 'generating';
  if (!world) { element('map-empty').hidden = false; element('map-empty').textContent = 'Generating world…'; }
  workspace.generationStarted();
  element<HTMLButtonElement>('play').disabled = true;
  updateExperimentControls();
  try {
    const result = prepared ?? await api.generate(recipe as Recipe);
    if (request !== generationId) return;
    const calculationMs = performance.now() - start;
    showStatus('Preparing GPU geometry for both views…');
    await map.setWorld(result.world);
    map.setTemperatureMonth(Number(temperatureMonthInput.value));
    const restoredFrame = prepared && 'waterFrame' in prepared ? prepared.waterFrame : null;
    const restoredMoisture = prepared && 'soilMoistureFrame' in prepared ? prepared.soilMoistureFrame
      : prepared && 'moistureFrame' in prepared ? prepared.moistureFrame : null;
    if (restoredFrame) map.setWaterFrame(restoredFrame);
    if (request !== generationId) return;
    // Queue acceptance before any subsequent UI action; no await between view
    // publication and this request. The old native world survives preparation.
    const accepted = api.acceptWorld(result.epoch);
    world = result.world; epoch = result.epoch; selected = null; waterFrame = null; waterBudget = null;
    unsavedProgress = false; diagnosticDirty = false;
    moistureFrame = restoredMoisture;
    map.setMoistureFrame(moistureFrame); renderMoisture();
    temperatureNormals = null; windNormals = null; climateNormals = null;
    rivers = null;
    element('rivers-summary').textContent = 'Rivers: waiting for climate normals.';
    for (const button of riverLayerButtons) button.disabled = true;
    vegetation = null; vegetationSeries = null; vegetationBusy = false; map.setVegetation(null);
    wildFood = null; wildFoodSeries = null; forageResult = null; map.setWildFood(null);
    traversalCosts = null;
    map.setTraversalCosts(null);
    clearTravelOrigin();
    for (const button of [...climateLayerButtons, ...vegetationLayerButtons, ...foodLayerButtons, ...traversalLayerButtons]) button.disabled = true;
    renderVegetation();
    map.setTemperatureNormals(null); map.setWindNormals(null);
    waterBusy = false; budgetBusyEpoch = null;
    if (prepared) setInputs(world.recipe);
    terrainStats = summarizeTerrain(world.surface, world.terrain);
    const waterStats = summarizeWater(world.surface, world.water);
    maximumWaterDepth = waterStats.maximumDepthMeters;
    if (restoredMoisture && hasRegionalSurface(restoredMoisture)) {
      const depth = isSoilMoistureFrame(restoredMoisture) ? restoredMoisture.regionalLiquidDepthMeters : restoredMoisture.regionalSurface!.depthMeters;
      maximumWaterDepth = Math.max(maximumWaterDepth,
        depth.reduce((maximum, depth) => Math.max(maximum, depth), 0));
    }
    const drainageStats = summarizeDrainage(world.surface, world.water, world.drainage);
    maximumContributingArea = drainageStats.maximumContributingAreaSquareMeters;
    basinStats = summarizeBasins(world.basins); basinChildren = basinTree(world.basins).children;
    inspectedBasin = null;
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
    element('basin-details').replaceChildren(); element('basin-title').textContent = 'Select a region';
    element('basin-note').textContent = 'Thresholds describe possible connections, not current water levels. Analysis includes underwater terrain.';
    element<HTMLButtonElement>('basin-parent').disabled = true; element<HTMLButtonElement>('basin-owner').disabled = true;
    element('basin-summary').textContent = `Basins: ${basinStats.leafCount} minima · ${basinStats.nodeCount} hierarchy branches · includes underwater terrain · no simulated filling or overflow`;
    element('height-summary').textContent = `Bed elevation: ${number.format(terrainStats.minimumMeters)} to ${number.format(terrainStats.maximumMeters)} m · area-weighted mean ${number.format(terrainStats.meanMeters)} m · dry preparation ${world.terrain.appliedPasses} / ${world.recipe.terrainPreparationPasses ?? 0} passes, ${number.format(world.terrain.transportedCubicMeters / 1e9)} km³ transported`;
    const waterRequest = world.recipe.water.mode === 'coverage' ? `${(world.recipe.water.fraction * 100).toFixed(2)}% target` : `${number.format(world.recipe.water.volumeCubicMeters / 1e9)} km³ requested`;
    element('water-summary').textContent = `Water: ${(waterStats.waterAreaFraction * 100).toFixed(2)}% actual / ${waterRequest} · main ocean ${(waterStats.mainOceanAreaFraction * 100).toFixed(2)}% · inland ${(waterStats.inlandWaterAreaFraction * 100).toFixed(2)}% · ${waterStats.bodyCount} bodies · level ${number.format(waterStats.levelMeters)} m · resolved stock ${number.format(waterStats.resolvedVolumeCubicMeters / 1e9)} km³`;
    element('water-note').textContent = 'Select a region to inspect its initial water depth and stored volume. Coverage fitting never splits equal-elevation plateaus; actual coverage can differ from the target. Seasonal exchange does not alter this full reference inventory.';
    element('drainage-summary').textContent = `Drainage: ${drainageStats.catchmentCount} terminal catchments · ${drainageStats.closedSinkCount} closed dry sinks · ${(drainageStats.closedDrainageLandFraction * 100).toFixed(2)}% of dry land ends in closed sinks · topology only, no flowing water`;
    element('drainage-note').textContent = 'Select a region to inspect its receiver, flat-routing rule, and contributing land area. Existing water bodies stop routing; closed sinks are preserved. Basin analysis is separate; lake dynamics are not implemented.';
    element('temperature-note').textContent = 'Calculating repeatable seasonal temperature normals from initial geography…';
    element('wind-note').textContent = 'Calculating repeatable surface-wind belts…';
    updateExaggeration();
    element('crust-note').textContent = 'Select a region to inspect its crust potential, fitted threshold, and material approximations.';
    const crustSummary = summarizeCrust(world.surface, world.crust);
    element('crust-summary').textContent = `Continental-dominant crust: ${(crustSummary.continentalAreaFraction * 100).toFixed(2)}% actual / ${(world.recipe.continentalFraction * 100).toFixed(2)}% target · ${crustSummary.continentalPatchCount} connected patches · largest ${number.format(crustSummary.largestContinentalPatchAreaSquareMeters / 1e12)} M km² · not emerged land`;
    element('boundary-note').textContent = 'Select a region to inspect its plate and any inter-plate boundary segments.';
    element('diagnostic-tick').textContent = 'Step 0';
    element('water-step').textContent = 'Select a source region · manual input, no elapsed time';
    element('model-label').textContent = `${world.recipe.modelVersion.toUpperCase()} · STATIC INITIAL CONDITIONS`;
    if (restoredMoisture) element('model-label').textContent = `${restoredMoisture.modelVersion.toUpperCase()} · FIXED GEOGRAPHY`;
    renderWaterBudget();
    if (restoredFrame) applyWaterFrame(restoredFrame, false);
    updateLegend();
    await accepted;
    if (request !== generationId) return;
    showStatus('Calculating seasonal temperature normals in the native core…');
    let temperatureError: string | null = null;
    let windError: string | null = null;
    try {
      const normals = await api.seasonalTemperature(epoch);
      if (request !== generationId || normals.epoch !== epoch) return;
      temperatureNormals = normals; map.setTemperatureNormals(normals);
      temperatureRange = {
        minimum: normals.annualMinimumCelsius.reduce((low, value) => Math.min(low, value), Infinity),
        maximum: normals.annualMaximumCelsius.reduce((high, value) => Math.max(high, value), -Infinity),
      };
      temperatureLayerButton.disabled = false; temperatureMonthInput.disabled = false;
      element<HTMLInputElement>('seasonal-snow').disabled = map.getSurfaceStyle() !== 'natural';
      element('temperature-note').textContent = 'Select a region to inspect solar geometry, initial wetness, elevation, and thermal response time.';
      updateLegend();
    } catch (error) {
      if (request !== generationId) return;
      temperatureError = error instanceof Error ? error.message : String(error);
      element('temperature-note').textContent = `Seasonal temperature normals unavailable: ${temperatureError}`;
    }
    showStatus('Calculating prescribed seasonal surface winds in the native core…');
    try {
      const normals = await api.seasonalWind(epoch);
      if (request !== generationId || normals.epoch !== epoch) return;
      windNormals = normals; map.setWindNormals(normals);
      windMaximum = 0;
      for (let month = 0; month < 12; month++) {
        const east = normals.monthlyEastMetersPerSecond[month], north = normals.monthlyNorthMetersPerSecond[month];
        for (let id = 0; id < east.length; id++) windMaximum = Math.max(windMaximum, Math.hypot(east[id], north[id]));
      }
      windLayerButton.disabled = false; temperatureMonthInput.disabled = false;
      element('wind-note').textContent = 'Select a region to inspect its eastward and northward wind components.';
      updateLegend();
    } catch (error) {
      if (request !== generationId) return;
      windError = error instanceof Error ? error.message : String(error);
      element('wind-note').textContent = `Seasonal surface-wind normals unavailable: ${windError}`;
    }
    showStatus('Calculating climate normals and biomes in the native core…');
    let climateError: string | null = null;
    try {
      const normals = await api.climateNormals(epoch);
      if (request !== generationId || normals.epoch !== epoch) return;
      climateNormals = normals; map.setClimateNormals(normals);
      for (const button of climateLayerButtons) button.disabled = false;
      updateLegend();
      showStatus('Starting vegetation at the climate-normal equilibrium…');
      const frame = await api.initializeVegetation(epoch);
      if (request !== generationId || frame.epoch !== epoch) return;
      applyVegetation(frame);
      await loadWildFood(epoch, request);
    } catch (error) {
      if (request !== generationId) return;
      climateError = error instanceof Error ? error.message : String(error);
      element('climate-note').textContent = `Climate normals or vegetation unavailable: ${climateError}`;
    }
    if (request !== generationId) return;
    if (climateNormals) {
      showStatus('Calculating foot-traversal costs in the native core…');
      await refreshTraversalCosts(epoch, request);
    }
    if (request !== generationId) return;
    if (climateNormals) {
      showStatus('Routing climate-normal runoff in the native core…');
      try {
        const analysis = await api.rivers(epoch);
        if (request !== generationId || analysis.epoch !== epoch) return;
        map.setRivers(analysis); rivers = analysis;
        for (const button of riverLayerButtons) button.disabled = false;
        const { landRunoff, landRunoffToOcean, landRunoffToEndorheic, landRunoffEvaporatedInSpillingLakes, endorheicArea, landArea } = analysis.totals;
        const percent = (fraction: number): string => `${(fraction * 100).toFixed(1)}%`;
        const fate = landRunoff > 0
          ? `${percent(landRunoffToOcean / landRunoff)} of land runoff reaches the ocean · ${percent(landRunoffToEndorheic / landRunoff)} ends in closed basins · ${percent(landRunoffEvaporatedInSpillingLakes / landRunoff)} evaporates in overflowing lakes`
          : 'no land runoff';
        element('rivers-summary').textContent = `Rivers: ${fate} · ${percent(landArea > 0 ? endorheicArea / landArea : 0)} of dry land drains to closed basins.`;
        if (selected !== null) inspect(selected, true);
      } catch (error) {
        if (request !== generationId) return;
        element('rivers-summary').textContent = `Rivers unavailable: ${error instanceof Error ? error.message : String(error)}`;
      }
    } else {
      element('rivers-summary').textContent = 'Rivers unavailable without climate normals.';
    }
    if (request !== generationId) return;
    if ((currentLayer === 'temperature' && !temperatureNormals) || (currentLayer === 'windSpeed' && !windNormals)
      || ((currentLayer === 'annualRunoff' || currentLayer === 'riverDischarge') && !rivers)
      || ((currentLayer === 'biome' || currentLayer === 'annualPrecipitation') && !climateNormals)
      || (['vegetationState', 'woodyCover', 'woodyChange', 'productivity', 'precipitationAnomaly'].includes(currentLayer) && !vegetation)
      || (['foragerCapacity', 'leanSeason', 'gameStock'].includes(currentLayer) && !wildFood)
      || (['travelCost', 'travelTime'].includes(currentLayer) && !traversalCosts)
      || (isMoistureLayer(currentLayer) && !supportsMoistureLayer(moistureFrame, currentLayer))) {
      document.querySelector<HTMLButtonElement>('[data-layer="surface"]')?.click();
    }
    cancel.hidden = true; save.disabled = false; saveResolved.disabled = false; exportView.disabled = false;
    waterOpen.disabled = false; waterSave.disabled = false; waterBudgetButton.disabled = false;
    element<HTMLButtonElement>('play').disabled = false;
    document.body.dataset.state = 'ready';
    updateExperimentControls();
    showStatus(temperatureError || windError || climateError ? `World ready; climate layer unavailable: ${temperatureError ?? windError ?? climateError}`
      : restoredMoisture ? `Seasonal checkpoint restored at ${restoredMoisture.elapsedSeconds} elapsed seconds. Complete stocks and ledgers are ready to continue.`
      : restoredFrame ? `Water checkpoint restored at prescribed step ${restoredFrame.step}. Exact stocks are ready to continue.`
      : `${world.recipe.plateCount} connected plates · ${world.tectonics.boundaryTypes.length} boundary segments · ${(world.stats.arrayBytes / 2 ** 20).toFixed(1)} MiB of model arrays · Static kinematics, no geological time integration`);
  } catch (error) {
    if (request === generationId) {
      void api.cancelGeneration();
      cancel.hidden = true; save.disabled = world === null; saveResolved.disabled = world === null;
      exportView.disabled = world === null;
      waterOpen.disabled = false; waterSave.disabled = world === null || waterBusy;
      temperatureLayerButton.disabled = temperatureNormals === null;
      windLayerButton.disabled = windNormals === null;
      for (const button of climateLayerButtons) button.disabled = climateNormals === null;
      for (const button of riverLayerButtons) button.disabled = rivers === null;
      for (const button of vegetationLayerButtons) button.disabled = vegetation === null;
      for (const button of foodLayerButtons) button.disabled = wildFood === null;
      for (const button of traversalLayerButtons) button.disabled = traversalCosts === null;
      temperatureMonthInput.disabled = temperatureNormals === null && windNormals === null;
      waterBudgetButton.disabled = world === null || budgetBusyEpoch === epoch;
      element<HTMLButtonElement>('play').disabled = world === null;
      showStatus(error instanceof Error ? error.message : String(error), true);
      document.body.dataset.state = 'error';
      if (!world) element('map-empty').textContent = 'World generation failed. Choose New world to review the settings.';
      waterAdd.disabled = world === null || selected === null || waterBusy;
      waterSpill.disabled = waterAdd.disabled;
      updateExperimentControls();
    }
  }
}

function applyWaterFrame(frame: WaterFrame, updateMap = true): void {
  if (!world) throw new Error('No generated world for the prescribed-water display.');
  if (updateMap) map.setWaterFrame(frame);
  waterFrame = frame; waterBudget = null; renderWaterBudget();
  maximumWaterDepth = frame.depthMeters.reduce((maximum, depth) => Math.max(maximum, depth), 0);
  let displayedWetArea = 0;
  for (let region = 0; region < frame.bodyIds.length; region++) {
    if (frame.bodyIds[region]) displayedWetArea += world.surface.areasSquareMeters[region];
  }
  element('water-summary').textContent = `Initial water stock ${number.format(world.water.resolvedVolumeCubicMeters / 1e9)} km³ · manual input ≈ ${number.format(Number(frame.acceptedInputUnits) / 2 ** 56 / 1e9)} km³ · displayed wet area ${(displayedWetArea / world.stats.totalAreaSquareMeters * 100).toFixed(2)}% · displayed main body ${frame.mainOceanId || 'none'}`;
  element('water-step').textContent = `Prescribed step ${frame.step} · cumulative input ≈ ${number.format(Number(frame.acceptedInputUnits) / 2 ** 56 / 1e9)} km³`;
  element('water-step').title = `Exact cumulative input: ${frame.acceptedInputUnits} units of 2^-56 m³`;
  element('model-label').textContent = 'BASINS-1 · BOUNDED PRESCRIBED WATER';
  element('basin-summary').textContent = `Basins: ${basinStats?.leafCount ?? 0} minima · ${basinStats?.nodeCount ?? 0} hierarchy branches · bounded prescribed-water spill/merge enabled; no climate or elapsed time`;
  if (selected !== null) inspect(selected, true);
  updateLegend();
}

async function prescribeWater(mode: PrescribedWaterMode): Promise<void> {
  if (!world || selected === null || moistureFrame || moistureBusyEpoch === epoch || waterBusy || checkpointOpening || document.body.dataset.state === 'generating') return;
  pause(); waterBusy = true; waterAdd.disabled = true; waterSpill.disabled = true;
  waterSave.disabled = true; waterBudgetButton.disabled = true;
  updateExperimentControls();
  const request = generationId, activeEpoch = epoch, source = selected;
  try {
    const frame = await api.prescribeWater(activeEpoch, source, mode);
    if (request !== generationId || frame.epoch !== epoch || !world) return;
    unsavedProgress = true;
    applyWaterFrame(frame);
    await refreshWaterBudget();
    if (waterBudget?.step === frame.step) {
      showStatus(`Prescribed runoff accepted at region ${source}. Exact basin stocks remain native; rendered water is an approximation.`);
    }
  } catch (error) {
    if (request === generationId) showStatus(error instanceof Error ? error.message : String(error), true);
  } finally {
    if (epoch === activeEpoch) {
      waterBusy = false;
      waterSave.disabled = checkpointOpening || document.body.dataset.state === 'generating';
      waterBudgetButton.disabled = checkpointOpening || budgetBusyEpoch === activeEpoch
        || document.body.dataset.state === 'generating';
      if (world && selected !== null && !checkpointOpening && document.body.dataset.state !== 'generating') {
        waterAdd.disabled = false; waterSpill.disabled = false;
      }
      updateExperimentControls();
    }
  }
}
waterAdd.addEventListener('click', () => void prescribeWater('oneCubicKilometer'));
waterSpill.addEventListener('click', () => void prescribeWater('fillToSpill'));
waterOpen.addEventListener('click', async () => {
  if (checkpointOpening || document.body.dataset.state === 'generating') return;
  if (!await workspace.confirmReplacement()) return;
  pause(); checkpointOpening = true;
  waterOpen.disabled = true; waterAdd.disabled = true; waterSpill.disabled = true;
  waterSave.disabled = true; waterBudgetButton.disabled = true;
  updateExperimentControls();
  try {
    const prepared = await api.openWaterCheckpoint();
    if (prepared) {
      await generate(null, prepared);
      if (world && epoch === prepared.epoch) await refreshWaterBudget();
    }
    else showStatus('Water checkpoint opening canceled.');
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
  finally {
    checkpointOpening = false;
    if (document.body.dataset.state !== 'generating') {
      waterOpen.disabled = false;
      waterSave.disabled = world === null || waterBusy;
      waterBudgetButton.disabled = world === null || budgetBusyEpoch === epoch;
      waterAdd.disabled = world === null || selected === null || waterBusy;
      waterSpill.disabled = waterAdd.disabled;
      updateExperimentControls();
    }
  }
});
waterBudgetButton.addEventListener('click', () => {
  if (!checkpointOpening && !waterBusy && document.body.dataset.state !== 'generating') {
    pause();
    void refreshWaterBudget();
  }
});
waterSave.addEventListener('click', async () => {
  if (!world || moistureFrame || moistureBusyEpoch === epoch || waterBusy || checkpointOpening || document.body.dataset.state === 'generating') return;
  pause();
  const activeEpoch = epoch;
  waterBusy = true; updateExperimentControls();
  waterSave.disabled = true;
  try {
    if (await api.saveWaterCheckpoint(epoch)) { unsavedProgress = false; showStatus('Exact prescribed-water checkpoint saved.'); }
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
  finally { if (epoch === activeEpoch) waterBusy = false; updateExperimentControls(); }
});
moistureOpen.addEventListener('click', async () => {
  if (checkpointOpening || waterBusy || advancing || moistureBusyEpoch === epoch || budgetBusyEpoch === epoch
    || document.body.dataset.state === 'generating') return;
  if (!await workspace.confirmReplacement()) return;
  pause(); checkpointOpening = true; updateExperimentControls();
  try {
    const prepared = await api.openSeasonalCheckpoint();
    if (prepared) await generate(null, prepared);
    else showStatus('Seasonal checkpoint opening canceled.');
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
  finally { checkpointOpening = false; updateExperimentControls(); }
});
moistureSave.addEventListener('click', async () => {
  if (!world || !moistureFrame || checkpointOpening || waterBusy || advancing || moistureBusyEpoch === epoch
    || document.body.dataset.state === 'generating') return;
  pause();
  const activeEpoch = epoch, seconds = moistureFrame.elapsedSeconds;
  waterBusy = true; updateExperimentControls();
  try {
    if (await api.saveSeasonalCheckpoint(activeEpoch)) { unsavedProgress = false; showStatus(`Complete seasonal checkpoint saved at ${seconds} elapsed seconds.`); }
    else showStatus('Seasonal checkpoint saving canceled.');
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
  finally { if (epoch === activeEpoch) waterBusy = false; updateExperimentControls(); }
});

function pause(): void {
  playing = false; clearTimeout(playbackTimer);
  element('play').textContent = 'Run diagnostic';
  pauseMoisture();
}
element('pause-active').addEventListener('click', pause);
async function advance(): Promise<void> {
  if (!playing || advancing || !world) return;
  advancing = true;
  updateExperimentControls();
  const request = generationId, activeEpoch = epoch;
  try {
    const frame = await api.advance(activeEpoch, 4);
    if (request !== generationId || frame.epoch !== epoch || !world) return;
    world.diagnosticField = frame.field; diagnosticDirty = true; map.refreshField();
    if (selected !== null) inspect(selected, true);
    element('diagnostic-tick').textContent = `Step ${frame.tick}`;
    showStatus(`Diagnostic diffusion · relative mass error ${frame.relativeMassError.toExponential(2)} · Recipe saves the initial state, not this diagnostic step.`);
  } catch (e) { if (request === generationId) { pause(); showStatus(String(e), true); } }
  finally {
    advancing = false;
    updateExperimentControls();
    if (playing) playbackTimer = window.setTimeout(() => void advance(), 33);
  }
}
element('play').addEventListener('click', () => {
  if (playing) pause();
  else if (!moistureFrame && moistureBusyEpoch !== epoch) { playing = true; element('play').textContent = 'Pause diagnostic'; updateExperimentControls(); void advance(); }
});
for (const button of document.querySelectorAll<HTMLButtonElement>('button[data-view]')) {
  button.addEventListener('click', () => {
    const mode = button.dataset.view as ViewMode;
    map.setMode(mode);
    updateLegend();
    for (const other of document.querySelectorAll('button[data-view]')) {
      other.classList.toggle('active', other === button); other.setAttribute('aria-pressed', String(other === button));
    }
  });
}

element('recipe-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  try {
    const { terrainPreparationPasses: _defaultPasses, ...baseRecipe } = DEFAULT_RECIPE;
    const recipe = parseRecipe({ ...baseRecipe, modelVersion: generatorFamilyInput.value, seed: seedInput.value, subdivision: Number(resolutionInput.value), radiusMeters: Number(radiusInput.value) * 1000,
      plateCount: Number(plateCountInput.value), maxPlateSpeedCmPerYear: Number(plateSpeedInput.value),
      continentalFraction: Number(continentalFractionInput.value) / 100, continentalScale: Number(continentalScaleInput.value),
      reliefScale: Number(reliefScaleInput.value), boundaryWidthKm: Number(boundaryWidthInput.value), detailAmplitudeMeters: Number(detailAmplitudeInput.value),
      ...(generatorFamilyInput.value !== LEGACY_MODEL_VERSION ? { terrainPreparationPasses: Number(terrainPreparationInput.value) } : {}),
      water: waterModeInput.value === 'coverage' ? { mode: 'coverage', fraction: Number(waterCoverageInput.value) / 100 }
        : { mode: 'volume', volumeCubicMeters: Number(waterVolumeInput.value) * 1e9 } });
    if (!workspace.hasUnsavedProgress) void generate(recipe);
    else { pause(); if (await workspace.confirmReplacement()) void generate(recipe); }
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
});
element('recipe-form').addEventListener('invalid', (event) => {
  if (event.target instanceof HTMLElement) {
    const settings = event.target.closest<HTMLDetailsElement>('.advanced-settings');
    if (settings) settings.open = true;
  }
}, true);
cancel.addEventListener('click', () => {
  generationId++; map.cancelPreparation(); void api.cancelGeneration(); cancel.hidden = true;
  save.disabled = world === null; saveResolved.disabled = world === null; exportView.disabled = world === null;
  waterOpen.disabled = false; waterSave.disabled = world === null || waterBusy;
  temperatureLayerButton.disabled = temperatureNormals === null;
  windLayerButton.disabled = windNormals === null;
  temperatureMonthInput.disabled = temperatureNormals === null && windNormals === null;
  waterBudgetButton.disabled = world === null || budgetBusyEpoch === epoch;
  element<HTMLButtonElement>('play').disabled = world === null;
  document.body.dataset.state = world ? 'ready' : 'idle';
  if (!world) element('map-empty').textContent = 'No world loaded. Choose New world to begin.';
  waterAdd.disabled = world === null || selected === null || waterBusy;
  waterSpill.disabled = waterAdd.disabled;
  showStatus(world ? 'Generation canceled. The previous surface is still displayed.' : 'Generation canceled.');
  updateExperimentControls();
});
element('new-seed').addEventListener('click', () => { seedInput.value = crypto.randomUUID().slice(0, 8); });
element('open-recipe').addEventListener('click', async () => {
  if (checkpointOpening || waterBusy) return;
  if (workspace.hasUnsavedProgress) pause();
  if (!await workspace.confirmReplacement()) return;
  try {
    const recipe = await api.openRecipe();
    if (recipe && !checkpointOpening && !waterBusy) { const validated = parseRecipe(recipe); setInputs(validated); void generate(validated); }
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
});
save.addEventListener('click', async () => {
  if (!world) return;
  try { if (await api.saveRecipe(world.recipe)) showStatus('Recipe saved. Reopen it to reproduce this surface.'); }
  catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
});
saveResolved.addEventListener('click', async () => {
  if (!world || document.body.dataset.state === 'generating') return;
  pause(); saveResolved.disabled = true;
  try {
    if (await api.saveResolvedWorld(epoch)) showStatus('Calculated initial world data exported. Manual water steps and diagnostic state are separate.');
    else showStatus('World-data export canceled.');
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
  finally { saveResolved.disabled = world === null || document.body.dataset.state === 'generating'; }
});
exportView.addEventListener('click', async () => {
  if (!world) return;
  exportView.disabled = true;
  showStatus('Exporting the current view to PNG…');
  try {
    const canvas = element<HTMLCanvasElement>('map');
    canvas.scrollIntoView({ block: 'center', inline: 'nearest' });
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    const bounds = canvas.getBoundingClientRect();
    const x = Math.max(0, Math.floor(bounds.left)), y = Math.max(0, Math.floor(bounds.top));
    const rect = { x, y, width: Math.min(window.innerWidth, Math.ceil(bounds.right)) - x,
      height: Math.min(window.innerHeight, Math.ceil(bounds.bottom)) - y };
    if (await api.exportView(rect)) showStatus('Current map or globe view exported as PNG. This is an image, not a world checkpoint.');
    else showStatus('PNG export canceled.');
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
  finally { exportView.disabled = world === null || document.body.dataset.state === 'generating'; }
});
function selectLayer(layer: Layer): void {
  currentLayer = layer;
  for (const other of document.querySelectorAll<HTMLButtonElement>('button[data-layer]')) {
    const active = other.dataset.layer === layer;
    other.classList.toggle('active', active);
    other.setAttribute('aria-pressed', String(active));
  }
  map.setLayer(currentLayer); updateLegend(); updateExperimentControls();
}
for (const button of document.querySelectorAll<HTMLButtonElement>('[data-layer]')) {
  button.addEventListener('click', () => selectLayer(button.dataset.layer as Layer));
}
temperatureMonthInput.addEventListener('change', () => {
  const month = Number(temperatureMonthInput.value);
  map.setTemperatureMonth(month);
  if (selected !== null) inspect(selected, true);
  updateLegend();
});
element<HTMLInputElement>('boundaries').addEventListener('change', (event) => map.setBoundaries((event.target as HTMLInputElement).checked));
element<HTMLInputElement>('smooth-shading').addEventListener('change', (event) => map.setSmoothShading((event.target as HTMLInputElement).checked));
element<HTMLInputElement>('relief-shading').addEventListener('change', (event) => map.setReliefShading((event.target as HTMLInputElement).checked));
element<HTMLInputElement>('drainage-lines').addEventListener('change', (event) => map.setDrainageLines((event.target as HTMLInputElement).checked));
element('reset-view').addEventListener('click', () => map.reset());
element('zoom-in').addEventListener('click', () => map.zoomBy(1.5));
element('zoom-out').addEventListener('click', () => map.zoomBy(1 / 1.5));
window.addEventListener('beforeunload', event => {
  pause();
  if (unsavedProgress || diagnosticDirty) { event.preventDefault(); event.returnValue = ''; }
});
window.addEventListener('unload', () => { navigation.dispose(); map.dispose(); });
setInputs(NEW_WORLD_RECIPE);
void generate({ ...NEW_WORLD_RECIPE });
