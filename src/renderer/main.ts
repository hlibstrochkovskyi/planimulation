import './style.css';
import { DEFAULT_RECIPE, parseRecipe } from '../core/recipe';
import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import type { DesktopAPI, PreparedWaterWorld, PrescribedWaterMode, WaterBudget, WaterFrame } from '../shared/desktop-api';
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
import { MOISTURE_MAX_SECONDS, MOISTURE_STOCK_FIELDS, SURFACE_TRANSFER_FIELDS, RUNOFF_TRANSFER_FIELDS } from '../shared/seasonal-moisture';
import type { MoistureFrame } from '../shared/seasonal-moisture';
import { MOISTURE_LAYERS, isMoistureLayer, moistureCalendar } from './seasonal-moisture';

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
const windLayerButton = document.querySelector<HTMLButtonElement>('[data-layer="windSpeed"]')!;
const moistureStart = element<HTMLButtonElement>('moisture-start');
const moistureStep = element<HTMLButtonElement>('moisture-step');
const moisturePlay = element<HTMLButtonElement>('moisture-play');
const moistureInterval = element<HTMLSelectElement>('moisture-interval');
const number = new Intl.NumberFormat('en', { maximumFractionDigits: 1 });
const budgetNumber = new Intl.NumberFormat('en', { maximumSignificantDigits: 6 });
let world: World | null = null;
let waterFrame: WaterFrame | null = null;
let waterBudget: WaterBudget | null = null;
let temperatureNormals: TemperatureNormals | null = null;
let windNormals: WindNormals | null = null;
let moistureFrame: MoistureFrame | null = null;
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

function showStatus(message: string, error = false): void {
  status.textContent = message;
  status.classList.toggle('error', error);
}

function updateExperimentControls(): void {
  const generating = document.body.dataset.state === 'generating';
  const busy = generating || checkpointOpening || waterBusy || budgetBusyEpoch === epoch
    || advancing || moistureBusyEpoch === epoch;
  const hasSeasonal = moistureFrame !== null;
  const finished = (moistureFrame?.elapsedSeconds ?? 0) >= MOISTURE_MAX_SECONDS;
  moistureStart.disabled = !world || busy || playing || hasSeasonal || (waterFrame?.step ?? 0) > 0;
  moistureStep.disabled = !hasSeasonal || busy || moisturePlaying || finished;
  // Pause must remain available while the current native step is in flight.
  moisturePlay.disabled = !hasSeasonal || finished || (!moisturePlaying && busy);
  moisturePlay.textContent = moisturePlaying ? 'Pause seasonal water' : 'Run seasonal water';
  moistureInterval.disabled = busy || moisturePlaying;
  for (const button of document.querySelectorAll<HTMLButtonElement>('[data-layer]')) {
    if (isMoistureLayer(button.dataset.layer ?? '')) button.disabled = !hasSeasonal || generating;
  }
  const manualBlocked = !world || busy || playing || moisturePlaying || hasSeasonal;
  waterAdd.disabled = manualBlocked || selected === null; waterSpill.disabled = waterAdd.disabled;
  waterSave.disabled = manualBlocked; waterBudgetButton.disabled = manualBlocked;
  waterOpen.disabled = busy || moisturePlaying || playing;
  element<HTMLButtonElement>('play').disabled = !world || hasSeasonal || (!playing && busy);
  document.body.dataset.seasonalWater = moisturePlaying ? 'running' : moistureBusyEpoch === epoch ? 'working' : hasSeasonal ? 'paused' : 'idle';
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
    element('moisture-time').textContent = 'Not initialized · regeneration discards the seasonal run';
    element('moisture-step-note').textContent = 'No interval recorded. Playback waits for each native response; frame rate does not set model time.';
    element('moisture-budget-note').textContent = 'Initialize seasonal water to inspect its approximate binary64 accounting. Do not add it to the full initial water inventory.';
    element('moisture-budget-details').replaceChildren(); element('moisture-selection-details').replaceChildren();
    element('moisture-selection-title').textContent = 'Select a region';
    element('moisture-selection-note').textContent = 'Stocks and recorded transfers use the same received frame as the map.';
    updateExperimentControls(); return;
  }
  element('moisture-time').textContent = moistureCalendar(frame.elapsedSeconds);
  const month = Math.floor((Math.floor(frame.elapsedSeconds / 86400) % 365) * 12 / 365) + 1;
  element('moisture-step-note').textContent = `Last interval ${frame.intervalSeconds / 3600} h · ${frame.coupledSubsteps} coupled / ${frame.transportSubsteps} transport substeps · next forcing month ${month}. The normals selector is independent. Pause may finish one requested interval.`;
  const budget = frame.budget;
  element('moisture-budget-details').dataset.relativeMassResidual = String(budget.residualKilograms / Math.max(budget.initialMobileWaterKilograms, 1));
  element('moisture-budget-details').dataset.relativeLedgerResidual = String(budget.maximumRelativeLocalSurfaceLedgerResidual);
  const volume = (kilograms: number): string => `${budgetNumber.format(kilograms / 1e12)} km³ WE`;
  const total = MOISTURE_STOCK_FIELDS.reduce((sum, key) => sum + budget[key], 0);
  appendDetails(element('moisture-budget-details'), [
    ['Initial mobile partition', volume(budget.initialMobileWaterKilograms)],
    ...MOISTURE_STOCK_FIELDS.map((key, i): [string, string] => [stockNames[i], volume(budget[key])]),
    ['Total current owned water', volume(total)],
    ['Mass residual / initial', (budget.residualKilograms / Math.max(budget.initialMobileWaterKilograms, 1)).toExponential(3)],
    ['Maximum regional ledger residual', budget.maximumRelativeLocalSurfaceLedgerResidual.toExponential(3)],
    ['Cumulative evaporation · flow integral', volume(budget.cumulativeEvaporationKilograms)],
    ['Cumulative precipitation · flow integral', volume(budget.cumulativePrecipitationKilograms)],
    ['Cumulative terminal delivery · flow integral', volume(budget.cumulativeRunoffTransfers.terminalDelivery)],
    ['Cumulative terminal evaporation · flow integral', volume(budget.cumulativeRunoffTransfers.terminalEvaporation)],
  ]);
  element('moisture-budget-note').textContent = `${frame.modelVersion} · six exclusive stocks. Flow integrals can count recirculated water and must not be added to the inventory. Fixed geography; no lake levels or deep-water replenishment. Seasonal desktop saving is not implemented.`;
  if (selected === null) {
    element('moisture-selection-title').textContent = 'Select a region';
    element('moisture-selection-details').replaceChildren();
  } else {
    const id = selected, area = world.surface.areasSquareMeters[id];
    const column = (kg: number): string => `${budgetNumber.format(kg / area)} mm WE`;
    element('moisture-selection-title').textContent = `Region ${id} · seasonal water`;
    element('moisture-selection-note').textContent = `At ${frame.elapsedSeconds} elapsed seconds. Transfer amounts cover only the last ${frame.intervalSeconds / 3600} h, not an annual normal. Terminal columns use fixed reference area, not a lake depth. Receiver: ${world.drainage.receivers[id] === id ? 'self · terminal' : `region ${world.drainage.receivers[id]}`}.`;
    appendDetails(element('moisture-selection-details'), [
      ...MOISTURE_STOCK_FIELDS.map((key, i): [string, string] => [stockNames[i], column(frame.stocks[key][id])]),
      ...SURFACE_TRANSFER_FIELDS.map((key, i): [string, string] => [`Last interval · ${surfaceTransferNames[i]}`, column(frame.surfaceTransfers[key][id])]),
      ...RUNOFF_TRANSFER_FIELDS.map((key, i): [string, string] => [`Last interval · ${runoffTransferNames[i]}`, column(frame.runoffTransfers[key][id])]),
      ['Mean drainage departure', frame.intervalSeconds ? `${budgetNumber.format(frame.runoffTransfers.sent[id] / 1000 / frame.intervalSeconds)} m³/s · not hydraulic discharge` : 'No interval'],
    ]);
  }
  updateExperimentControls();
}

async function requestMoisture(seconds: number): Promise<void> {
  if (!world || moistureBusyEpoch === epoch || waterBusy || checkpointOpening || advancing
    || budgetBusyEpoch === epoch || document.body.dataset.state === 'generating') return;
  const activeEpoch = epoch, origin = world, request = generationId;
  moistureBusyEpoch = activeEpoch; updateExperimentControls();
  const started = performance.now();
  try {
    const frame = await api.seasonalMoisture(activeEpoch, seconds);
    const requestMilliseconds = performance.now() - started;
    // An accepted old-world step may finish while a replacement is preparing.
    // Retain it until that world is actually replaced, including cancellation.
    if (frame.epoch !== epoch || world !== origin) return;
    moistureFrame = frame; map.setMoistureFrame(frame);
    waterBudget = null; renderWaterBudget(); renderMoisture(); updateLegend();
    element('moisture-time').dataset.requestMilliseconds = String(requestMilliseconds);
    element('model-label').textContent = `${world.recipe.modelVersion.toUpperCase()} · SEASONAL-MOISTURE-3 · FIXED GEOGRAPHY`;
    if (request === generationId) showStatus(`Seasonal water ${frame.elapsedSeconds / 86400} days · native request + IPC ${number.format(requestMilliseconds)} ms · relative total-water residual ${(frame.budget.residualKilograms / Math.max(frame.budget.initialMobileWaterKilograms, 1)).toExponential(2)}.`);
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
  for (const [label, value] of [['Crust baseline', world.terrain.baseline[id]], ['Convergence uplift', world.terrain.convergence[id]],
    ['Divergence (ridge − rift)', world.terrain.divergence[id]], ['Bounded detail', world.terrain.detail[id]],
    ['Dry preparation', world.terrain.preparation[id]], ['Total elevation', world.terrain.elevation[id]]] as const) {
    const row = document.createElement('li'); row.textContent = `${label}: ${value.toFixed(2)} m`; elevationDetails.append(row);
  }
  element('crust-note').textContent = `Seeded spherical potential ${world.crust.potential[id].toFixed(6)}; fitted threshold ${world.crust.threshold.toFixed(6)}; smooth transition width 0.12. Continentality blends the 7–35 km thickness and 3,000–2,800 kg/m³ density endmembers. These are initial model approximations, not elevation or water depth.`;
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

function updateLegend(): void {
  const legends: Record<Exclude<Layer, keyof typeof MOISTURE_LAYERS>, [string, string, string]> = {
    surface: [`Land and water surface${waterFrame ? ' · prescribed-water display' : ''} · globe shoreline is a display approximation · colors are not biomes`, '', ''],
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
    depth: [`${waterFrame ? 'Prescribed-water display depth' : 'Initial water depth'} · gray = dry · globe shows the bed, not a water-surface mesh`, '0 m', `${number.format(maximumWaterDepth)} m`],
    waterBodies: [`${waterFrame ? 'Connected displayed water bodies' : 'Connected water bodies'} · gray = dry · main ocean = body ${waterFrame?.mainOceanId || world?.water.mainOceanId || 'none'}`, '', ''],
    catchments: ['Drainage catchments · colors identify terminal outlets, not states or rivers', '', ''],
    contributingArea: ['Contributing dry-land area · logarithmic color scale · not river discharge', '0 km²', `${number.format(maximumContributingArea / 1e6)} km²`],
    basins: ['Basin branches · exclusive ownership, not full nested footprints or current lakes', '', ''],
    spill: ['Next basin connection threshold · not current water level · gray = root without external drain',
      basinStats?.minimumSpillMeters === null ? 'No finite thresholds' : `${number.format(basinStats?.minimumSpillMeters ?? 0)} m`,
      basinStats?.maximumSpillMeters === null ? '' : `${number.format(basinStats?.maximumSpillMeters ?? 0)} m`],
    temperature: [`Month ${Number(temperatureMonthInput.value) + 1} temperature normal · fixed initial geography, not weather`,
      `${number.format(temperatureRange.minimum)} °C`, `${number.format(temperatureRange.maximum)} °C`],
    windSpeed: [`Month ${Number(temperatureMonthInput.value) + 1} prescribed surface-wind speed · no atmospheric dynamics`,
      '0 m/s', `${number.format(windMaximum)} m/s`],
  };
  const [title, low, high] = isMoistureLayer(currentLayer)
    ? [`${MOISTURE_LAYERS[currentLayer].title} · fixed logarithmic scale · seasonal water at ${moistureFrame?.elapsedSeconds ?? 0} s`,
      `0 ${MOISTURE_LAYERS[currentLayer].unit}`, `≥ ${number.format(MOISTURE_LAYERS[currentLayer].maximum)} ${MOISTURE_LAYERS[currentLayer].unit}`]
    : legends[currentLayer];
  element('legend-title').textContent = title;
  element('legend-low').textContent = low;
  element('legend-high').textContent = high;
  const plateLayer = currentLayer === 'plates' || currentLayer === 'boundaries';
  element('legend-scale').hidden = plateLayer || currentLayer === 'waterBodies' || currentLayer === 'surface' || currentLayer === 'catchments' || currentLayer === 'basins';
  element('boundary-legend').hidden = !plateLayer;
  element('legend-gradient').classList.toggle('water-gradient', currentLayer === 'depth');
  element('legend-gradient').classList.toggle('temperature-gradient', currentLayer === 'temperature');
  element('legend-gradient').classList.toggle('wind-gradient', currentLayer === 'windSpeed');
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
  terrainPreparationInput.value = String(recipe.terrainPreparationPasses ?? 0);
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

async function generate(recipe: Recipe | null, prepared?: PreparedWaterWorld): Promise<void> {
  pause(); map.cancelPreparation();
  const request = ++generationId;
  const start = performance.now();
  cancel.hidden = false; save.disabled = true; saveResolved.disabled = true;
  temperatureLayerButton.disabled = true; windLayerButton.disabled = true; temperatureMonthInput.disabled = true;
  exportView.disabled = true; waterAdd.disabled = true; waterSpill.disabled = true;
  waterOpen.disabled = true; waterSave.disabled = true; waterBudgetButton.disabled = true;
  showStatus(prepared ? 'Preparing restored world views…' : 'Building the surface in the native core…');
  document.body.dataset.state = 'generating';
  element<HTMLButtonElement>('play').disabled = true;
  updateExperimentControls();
  try {
    const result = prepared ?? await api.generate(recipe as Recipe);
    if (request !== generationId) return;
    const calculationMs = performance.now() - start;
    showStatus('Preparing GPU geometry for both views…');
    await map.setWorld(result.world);
    map.setTemperatureMonth(Number(temperatureMonthInput.value));
    const restoredFrame = prepared?.waterFrame ?? null;
    if (restoredFrame) map.setWaterFrame(restoredFrame);
    if (request !== generationId) return;
    // Queue acceptance before any subsequent UI action; no await between view
    // publication and this request. The old native world survives preparation.
    const accepted = api.acceptWorld(result.epoch);
    world = result.world; epoch = result.epoch; selected = null; waterFrame = null; waterBudget = null;
    moistureFrame = null;
    map.setMoistureFrame(null); renderMoisture();
    temperatureNormals = null; windNormals = null;
    map.setTemperatureNormals(null); map.setWindNormals(null);
    waterBusy = false; budgetBusyEpoch = null;
    if (prepared) setInputs(world.recipe);
    terrainStats = summarizeTerrain(world.surface, world.terrain);
    const waterStats = summarizeWater(world.surface, world.water);
    maximumWaterDepth = waterStats.maximumDepthMeters;
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
    if ((currentLayer === 'temperature' && !temperatureNormals) || (currentLayer === 'windSpeed' && !windNormals)
      || isMoistureLayer(currentLayer)) {
      document.querySelector<HTMLButtonElement>('[data-layer="surface"]')?.click();
    }
    cancel.hidden = true; save.disabled = false; saveResolved.disabled = false; exportView.disabled = false;
    waterOpen.disabled = false; waterSave.disabled = false; waterBudgetButton.disabled = false;
    element<HTMLButtonElement>('play').disabled = false;
    document.body.dataset.state = 'ready';
    updateExperimentControls();
    showStatus(temperatureError || windError ? `World ready; climate layer unavailable: ${temperatureError ?? windError}`
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
      temperatureMonthInput.disabled = temperatureNormals === null && windNormals === null;
      waterBudgetButton.disabled = world === null || budgetBusyEpoch === epoch;
      element<HTMLButtonElement>('play').disabled = world === null;
      showStatus(error instanceof Error ? error.message : String(error), true);
      document.body.dataset.state = 'error';
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
    if (await api.saveWaterCheckpoint(epoch)) showStatus('Exact prescribed-water checkpoint saved.');
  } catch (error) { showStatus(error instanceof Error ? error.message : String(error), true); }
  finally { if (epoch === activeEpoch) waterBusy = false; updateExperimentControls(); }
});

function pause(): void {
  playing = false; clearTimeout(playbackTimer);
  element('play').textContent = 'Run diagnostic';
  pauseMoisture();
}
async function advance(): Promise<void> {
  if (!playing || advancing || !world) return;
  advancing = true;
  updateExperimentControls();
  const request = generationId, activeEpoch = epoch;
  try {
    const frame = await api.advance(activeEpoch, 4);
    if (request !== generationId || frame.epoch !== epoch || !world) return;
    world.diagnosticField = frame.field; map.refreshField();
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
      terrainPreparationPasses: Number(terrainPreparationInput.value),
      water: waterModeInput.value === 'coverage' ? { mode: 'coverage', fraction: Number(waterCoverageInput.value) / 100 }
        : { mode: 'volume', volumeCubicMeters: Number(waterVolumeInput.value) * 1e9 } }));
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
  waterAdd.disabled = world === null || selected === null || waterBusy;
  waterSpill.disabled = waterAdd.disabled;
  showStatus(world ? 'Generation canceled. The previous surface is still displayed.' : 'Generation canceled.');
  updateExperimentControls();
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
temperatureMonthInput.addEventListener('change', () => {
  const month = Number(temperatureMonthInput.value);
  map.setTemperatureMonth(month);
  if (selected !== null) inspect(selected, true);
  updateLegend();
});
element<HTMLInputElement>('boundaries').addEventListener('change', (event) => map.setBoundaries((event.target as HTMLInputElement).checked));
element('reset-view').addEventListener('click', () => map.reset());
element('zoom-in').addEventListener('click', () => map.zoomBy(1.5));
element('zoom-out').addEventListener('click', () => map.zoomBy(1 / 1.5));
window.addEventListener('beforeunload', () => { pause(); map.dispose(); });
setInputs(DEFAULT_RECIPE);
void generate({ ...DEFAULT_RECIPE });
