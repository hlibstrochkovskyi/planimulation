/** Presentation state only. Native commands and accepted model state stay in main.ts. */
export interface WorkspaceState {
  layer: string;
  selected: number | null;
  seasonalModel: string | null;
  seasonalWorking: boolean;
  seasonalRunning: boolean;
  seasonalFinished: boolean;
  diagnosticRunning: boolean;
  diagnosticDirty: boolean;
  busy: boolean;
  unsaved: boolean;
  manualStep: number;
  vegetationYear: number | null;
  vegetationWorking: boolean;
  layerReading: { label: string; value: string; scope: string } | null;
}

function node<T extends HTMLElement = HTMLElement>(id: string): T {
  const result = document.getElementById(id);
  if (!result) throw new Error(`Missing workspace element: ${id}`);
  return result as T;
}

const models: Record<string, { button: string; note: string }> = {
  baseline: { button: 'moisture-start', note: 'Six finite water stores on fixed geography, driven by prescribed monthly climate.' },
  orographic: { button: 'moisture-start-orographic', note: 'Experimental upslope removal of supersaturation. Fixed geography; not calibrated.' },
  precise: { button: 'moisture-start-precise', note: 'Upslope response with checkpointed water precision. Fixed geography; not calibrated.' },
  regional: { button: 'moisture-start-regional', note: 'Regional surface flow with finite reference bodies and prescribed initial heads. Not calibrated.' },
  soil: { button: 'moisture-start-soil', note: 'Unified soil and surface water with finite reference bodies. Unresolved requests remain at donors.' },
};

interface Preferences { layersWidth?: number; inspectorWidth?: number; closedGroups?: string[] }

function readPreferences(): Preferences {
  try {
    const value: unknown = JSON.parse(localStorage.getItem('planimulation.workspace.v1') ?? '{}');
    return value && typeof value === 'object' ? value as Preferences : {};
  } catch { return {}; }
}

export class Workspace {
  private readonly preferences = readPreferences();
  private inspectorTab = 'overview';
  private experimentTab = 'seasonal';
  private state: WorkspaceState | null = null;
  private readonly filter = node<HTMLInputElement>('layer-filter');
  private readonly model = node<HTMLSelectElement>('moisture-model');
  private readonly groups = [...document.querySelectorAll<HTMLDetailsElement>('.layer-group')];
  private filtering = false;

  constructor() {
    for (const button of document.querySelectorAll<HTMLButtonElement>('[data-copy-target]')) {
      button.addEventListener('click', () => { void this.copyText(button, node(button.dataset.copyTarget!).textContent ?? ''); });
    }
    node<HTMLButtonElement>('copy-region').addEventListener('click', () => {
      const lines = [
        `World seed\t${node('world-name').textContent}`,
        `Initial fingerprint\t${node('fingerprint').textContent}`,
        node('selection-title').textContent ?? '',
        node('selection-note').textContent ?? '',
        `Layer\t${node('active-layer-title').textContent}`,
      ];
      if (!node('layer-reading').hidden) {
        lines.push(`${node('layer-reading-label').textContent}\t${node('layer-reading-value').textContent}`, node('layer-reading-scope').textContent ?? '');
      }
      if (this.state?.seasonalModel) {
        lines.push(`Seasonal model\t${this.state.seasonalModel}`, `Seasonal time\t${node('moisture-time').textContent}`);
      }
      const rows = new Set<string>();
      for (const section of node('inspect-overview').querySelectorAll<HTMLElement>('.inspector')) {
        if (!section.checkVisibility()) continue;
        const heading = section.querySelector('h3');
        if (heading) lines.push(heading.textContent ?? '');
        for (const note of section.querySelectorAll<HTMLElement>('.muted')) {
          if (note.checkVisibility()) lines.push(note.textContent ?? '');
        }
        for (const dt of section.querySelectorAll<HTMLElement>('dt')) {
          const dd = dt.nextElementSibling;
          if (!dt.checkVisibility() || dd?.tagName !== 'DD') continue;
          const row = `${dt.textContent}\t${dd.textContent}`;
          if (!rows.has(row)) { rows.add(row); lines.push(row); }
        }
      }
      void this.copyText(node<HTMLButtonElement>('copy-region'), lines.filter(Boolean).join('\n'));
    });
    if (!Array.isArray(this.preferences.closedGroups)) this.preferences.closedGroups = this.groups.filter(group => !group.open).map(group => group.dataset.group!);
    this.bindTabs('[data-inspect-tab]', 'inspectTab', value => { this.inspectorTab = value; this.renderInspection(); });
    this.bindTabs('[data-experiment]', 'experiment', value => {
      this.experimentTab = value;
      for (const panel of document.querySelectorAll<HTMLElement>('.experiment-panel')) panel.hidden = panel.id !== `experiment-${value}`;
      this.renderExecutionState();
    });
    for (const group of this.groups) {
      if (Array.isArray(this.preferences.closedGroups)) group.open = !this.preferences.closedGroups.includes(group.dataset.group!);
      group.addEventListener('toggle', () => {
        if (this.filtering || this.filter.value.trim()) return;
        this.preferences.closedGroups = this.groups.filter(item => !item.open).map(item => item.dataset.group!);
        this.savePreferences();
      });
    }
    this.filter.addEventListener('input', () => this.filterLayers());
    this.filter.addEventListener('keydown', event => {
      if (event.key === 'Escape' && this.filter.value) { this.filter.value = ''; this.filterLayers(); event.preventDefault(); event.stopPropagation(); }
      if (event.key === 'ArrowDown') {
        const first = [...document.querySelectorAll<HTMLButtonElement>('.layer:not([hidden]):not(:disabled)')].find(button => button.checkVisibility());
        first?.focus(); event.preventDefault();
      }
    });
    node('layer-navigation').addEventListener('keydown', event => {
      if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key) || !(event.target instanceof HTMLButtonElement)) return;
      const visible = [...document.querySelectorAll<HTMLButtonElement>('.layer:not([hidden]):not(:disabled)')]
        .filter(button => button.checkVisibility());
      const index = visible.indexOf(event.target);
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? visible.length - 1
        : (index + (event.key === 'ArrowDown' ? 1 : -1) + visible.length) % visible.length;
      visible[next]?.focus(); event.preventDefault();
    });
    this.model.addEventListener('change', () => this.renderModel());
    node('dismiss-notice').addEventListener('click', () => { node('workspace-notice').hidden = true; });
    this.renderModel();
    for (const opener of document.querySelectorAll<HTMLButtonElement>('[data-dialog]')) {
      opener.addEventListener('click', () => {
        if (opener.dataset.dialog === 'status-dialog') {
          node('full-status').textContent = node('status').textContent;
          node('full-model').textContent = node('model-label').textContent;
        }
        node<HTMLDialogElement>(opener.dataset.dialog!).showModal();
      });
    }
    for (const closer of document.querySelectorAll<HTMLButtonElement>('[data-close-dialog]')) {
      closer.addEventListener('click', () => closer.closest('dialog')!.close());
    }
    for (const dialog of document.querySelectorAll('dialog')) {
      dialog.addEventListener('click', event => {
        if (event.target !== dialog || dialog.id === 'replace-dialog') return;
        const bounds = dialog.getBoundingClientRect();
        if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
      });
    }
    node('keep-world').addEventListener('click', () => node<HTMLDialogElement>('replace-dialog').close('keep'));
    node('replace-world').addEventListener('click', () => node<HTMLDialogElement>('replace-dialog').close('replace'));
    node('file-menu').addEventListener('click', event => {
      if (event.target instanceof Element && event.target.closest('button')) node('file-menu').hidePopover();
    });
    node('display-menu').addEventListener('beforetoggle', event => {
      if ((event as ToggleEvent).newState !== 'open') return;
      const bounds = node('display-trigger').getBoundingClientRect();
      node('display-menu').style.left = `${Math.max(8, Math.min(bounds.right - 310, window.innerWidth - 318))}px`;
      node('display-menu').style.top = `${bounds.bottom + 8}px`;
    });
    for (const pane of ['layers', 'inspector'] as const) {
      node(`toggle-${pane}`).addEventListener('click', () => this.togglePane(pane));
      node(`close-${pane}`).addEventListener('click', () => this.togglePane(pane, false));
    }
    node('collapse-dock').addEventListener('click', () => {
      const content = node('dock-content'); content.hidden = !content.hidden;
      const button = node('collapse-dock');
      button.textContent = content.hidden ? '+' : '−';
      button.setAttribute('aria-expanded', String(!content.hidden));
      button.setAttribute('aria-label', content.hidden ? 'Expand experiment controls' : 'Collapse experiment controls');
      this.renderExecutionState();
    });
    this.bindResizer('layers', 200, 380, 248);
    this.bindResizer('inspector', 280, 520, 340);
    window.addEventListener('resize', () => this.syncPaneAccessibility());
    document.addEventListener('keydown', event => {
      if (event.key === 'Escape' && !document.querySelector('dialog[open], :popover-open')) {
        const open = document.body.classList.contains('inspector-overlay') ? 'inspector' : document.body.classList.contains('layers-overlay') ? 'layers' : null;
        document.body.classList.remove('layers-overlay', 'inspector-overlay'); this.syncPaneAccessibility();
        if (open) node(`toggle-${open}`).focus();
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k' && !document.querySelector('dialog[open]')) {
        this.togglePane('layers', true); this.filter.focus(); this.filter.select(); event.preventDefault();
      }
    });
    document.addEventListener('pointerdown', event => {
      if (!(event.target instanceof Element)) return;
      for (const pane of ['layers', 'inspector']) {
        if (!event.target.closest(`#${pane}-pane, #toggle-${pane}`)) document.body.classList.remove(`${pane}-overlay`);
      }
      this.syncPaneAccessibility();
    });
    this.syncPaneAccessibility();
    this.renderInspection();
  }

  update(state: WorkspaceState): void {
    const previous = this.state;
    this.state = state;
    node('unsaved-indicator').hidden = !state.unsaved;
    node<HTMLButtonElement>('copy-seed').disabled = state.busy || node('world-name').textContent === 'Preparing world…';
    node('layer-reading').hidden = !state.layerReading;
    node('primary-region-details').hidden = !!state.layerReading;
    node('layer-reading-label').textContent = state.layerReading?.label ?? '';
    node('layer-reading-value').textContent = state.layerReading?.value ?? '';
    node('layer-reading-scope').textContent = state.layerReading?.scope ?? '';
    const active = document.querySelector<HTMLButtonElement>(`[data-layer="${state.layer}"]`);
    if (active && previous && previous.layer !== state.layer) active.closest<HTMLDetailsElement>('details')!.open = true;
    node('active-layer-title').textContent = active ? [...active.childNodes].filter(child => child.nodeType === Node.TEXT_NODE).map(child => child.textContent).join('').trim() : state.layer;
    node('climate-context').hidden = state.layer !== 'temperature' && state.layer !== 'windSpeed';
    node('traversal-context').hidden = state.layer !== 'travelCost' && state.layer !== 'travelTime';
    this.model.disabled = state.busy || state.seasonalModel !== null || state.manualStep > 0 || state.diagnosticRunning;
    if (state.seasonalModel) {
      this.model.value = state.seasonalModel;
    }
    this.renderModel();
    this.renderInspection();
    this.renderExecutionState();
    node('moisture-step-note').hidden = !state.seasonalModel;
    node('layer-availability').textContent = state.seasonalModel
      ? 'Regional depth and surface transfers require regional surface flow or unified soil water.'
      : 'Initialize seasonal water to unlock its layers. Regional depth and transfers require a regional model.';
    for (const layer of document.querySelectorAll<HTMLButtonElement>('[data-layer]')) {
      layer.setAttribute('aria-disabled', String(layer.disabled));
      if (layer.disabled && !layer.title) layer.title = layer.dataset.layer === 'temperature' || layer.dataset.layer === 'windSpeed'
        ? 'Climate normals are being prepared or are unavailable. See status details.'
        : layer.dataset.layer === 'travelCost' || layer.dataset.layer === 'travelTime'
          ? 'Movement costs are being prepared or are unavailable. See status details.'
          : 'Initialize a compatible seasonal water model to inspect this layer.';
      if (!layer.disabled && (layer.title.startsWith('Climate normals') || layer.title.startsWith('Movement costs'))) layer.removeAttribute('title');
    }
  }

  generationStarted(): void {
    node<HTMLDialogElement>('world-dialog').close();
  }

  statusChanged(message: string, error: boolean): void {
    node('notice-message').textContent = message;
    node('workspace-notice').hidden = !error;
    node('full-status').textContent = message;
    node('full-model').textContent = node('model-label').textContent;
  }

  async confirmReplacement(): Promise<boolean> {
    if (!this.state?.unsaved) return true;
    const dialog = node<HTMLDialogElement>('replace-dialog');
    if (dialog.open) return false;
    node('replace-detail').textContent = this.state.diagnosticDirty
      ? 'Diagnostic diffusion has no resumable checkpoint. A recipe saves only the initial world.'
      : 'Checkpoint actions are available in the File menu. Pause and finish the current request before saving.';
    dialog.returnValue = 'keep';
    return new Promise(resolve => {
      dialog.addEventListener('close', () => resolve(dialog.returnValue === 'replace'), { once: true });
      dialog.showModal();
    });
  }

  get hasUnsavedProgress(): boolean { return this.state?.unsaved ?? false; }

  inspectionChanged(reveal = false): void {
    this.renderInspection();
    if (reveal) this.togglePane('inspector', true);
  }

  private renderModel(): void {
    const configuration = models[this.model.value] ?? models.baseline;
    for (const model of Object.values(models)) node(model.button).hidden = model.button !== configuration.button;
    node(configuration.button).parentElement!.hidden = !!this.state?.seasonalModel;
    node('moisture-play').classList.toggle('primary-action', !!this.state?.seasonalModel);
    node('moisture-model-note').textContent = configuration.note + (this.state?.seasonalModel ? ' Create or reopen a world to switch models.' : '');
    if (this.state?.manualStep) node('moisture-model-note').textContent += ' Create or reopen a world before initializing seasonal water after manual inputs.';
    if (this.state?.seasonalFinished) node('moisture-model-note').textContent = 'The supported experiment duration has been reached. Save a checkpoint or create a new world.';
    node('manual-availability').hidden = !this.state?.seasonalModel;
    node('diagnostic-availability').hidden = !this.state?.seasonalModel;
  }

  private renderInspection(): void {
    const selected = this.state?.selected !== null && this.state?.selected !== undefined;
    const budgets = this.inspectorTab === 'budgets';
    node('copy-region').hidden = !selected || this.inspectorTab !== 'overview';
    node('layer-reading').hidden = !this.state?.layerReading || !selected || budgets;
    node('selection-heading').hidden = !selected || budgets;
    node('inspector-empty').hidden = selected || budgets;
    for (const tab of ['overview', 'factors', 'budgets']) node(`inspect-${tab}`).hidden = tab !== this.inspectorTab || (!selected && tab !== 'budgets');
    const seasonal = node('moisture-selection-details').closest<HTMLElement>('.inspector')!;
    seasonal.hidden = !this.state?.seasonalModel;
    node('basin-title').closest<HTMLElement>('.inspector')!.hidden = !['basins', 'spill', 'catchments'].includes(this.state?.layer ?? '');
    const relevant: Record<string, string[]> = {
      'climate-note': ['biome', 'annualPrecipitation'],
      'vegetation-note': ['vegetationState', 'woodyCover', 'woodyChange', 'productivity', 'precipitationAnomaly'],
      'food-note': ['foragerCapacity', 'leanSeason', 'gameStock', 'productivity'],
      'traversal-note': ['travelCost', 'travelTime'],
      'crust-note': ['crust', 'thickness', 'plates', 'surface'],
      'water-note': ['surface', 'depth', 'waterBodies'],
      'drainage-note': ['catchments', 'contributingArea', 'runoffFlow', 'runoffWater', 'basins', 'spill'],
      'temperature-note': ['temperature', 'snowWater', 'soilWater', 'vaporWater', 'precipitation'],
      'wind-note': ['windSpeed', 'vaporWater', 'precipitation'],
      'elevation-note': ['surface', 'elevation', 'uplift', 'basins', 'spill'],
      'boundary-note': ['plates', 'boundaries', 'speed', 'uplift'],
    };
    const layer = this.state?.layer ?? 'surface';
    const priorities: Record<string, string[]> = {
      surface: ['Elevation (reference datum)', 'Initial water depth', 'Initial water body', 'Reference surface area'],
      elevation: ['Elevation (reference datum)', 'Continentality', 'Crust thickness', 'Reference surface area'],
      crust: ['Continentality', 'Crust thickness', 'Crust density', 'Tectonic plate'],
      thickness: ['Crust thickness', 'Crust density', 'Continentality', 'Tectonic plate'],
      depth: ['Initial water depth', 'Initial water body', 'Elevation (reference datum)'],
      waterBodies: ['Initial water body', 'Initial water depth', 'Elevation (reference datum)'],
      temperature: ['temperature normal', 'Annual mean temperature normal', 'Daily normal range', 'Elevation (reference datum)'],
      windSpeed: ['wind speed normal', 'wind component', 'Elevation (reference datum)'],
      biome: ['Biome (climate class)', 'Climate annual mean', 'Climate warmest / coldest month', 'Annual precipitation'],
      annualPrecipitation: ['Annual precipitation', 'Zonal precipitation potential', 'Moisture supply', 'Windward lift'],
      annualRunoff: ['Land runoff', 'evapotranspiration (', 'River class', 'Downstream terminal', 'Terminal lake'],
      riverDischarge: ['river discharge', 'monthly discharge', 'River class', 'Downstream terminal', 'Terminal lake'],
      vegetationState: ['Vegetation state', 'Biome (climate class)', 'Woody / herb cover', 'Established cover'],
      woodyCover: ['Woody / herb cover', 'Woody capacity', 'Root-zone moisture', 'Vegetation state'],
      woodyChange: ['Woody change, 10 years', 'Woody / herb cover', 'This year\'s precipitation', 'Vegetation state'],
      productivity: ['Net primary productivity', 'Woody / herb cover', 'This year\'s precipitation', 'Annual mean temperature normal'],
      precipitationAnomaly: ['This year\'s precipitation', 'Root-zone moisture', 'Annual precipitation', 'Vegetation state'],
      foragerCapacity: ['Forager capacity', 'With perfect storage', 'Plant food / game offtake', 'Vegetation state'],
      leanSeason: ['With perfect storage', 'Forager capacity', 'Climate warmest / coldest month', 'Vegetation state'],
      gameStock: ['Game stock', 'Plant food / game offtake', 'Woody / herb cover', 'This year\'s precipitation'],
      travelCost: ['Travel cost', 'Walking pace', 'Slope & ruggedness', 'Land cover factor', 'Snow factor', 'River crossing'],
      travelTime: ['Travel time from origin', 'Least-time route length', 'Average route pace', 'Travel cost', 'Land cover factor'],
      catchments: ['Drainage receiver', 'Catchment outlet', 'Contributing', 'Elevation (reference datum)'],
      contributingArea: ['Contributing', 'Drainage receiver', 'Catchment outlet'],
      plates: ['Tectonic plate', 'Plate area', 'Speed (model frame)', 'Continentality'],
      speed: ['Speed (model frame)', 'Tectonic plate', 'Plate area'],
      uplift: ['Elevation (reference datum)', 'Tectonic plate', 'Crust thickness'],
      boundaries: ['Tectonic plate', 'Speed (model frame)'],
      basins: ['Elevation (reference datum)', 'Catchment outlet'],
      spill: ['Elevation (reference datum)', 'Catchment outlet'],
      signal: ['Diagnostic signal', 'Reference surface area'],
      area: ['Reference surface area', 'Reference neighbor distance', 'Connected neighbors'],
      latitude: ['Reference surface area'],
    };
    let chosen = priorities[layer] ?? priorities.surface;
    const rows = [...node('selection-details').querySelectorAll('dt')];
    if (['surface', 'depth', 'waterBodies'].includes(layer) && rows.some(dt => dt.textContent === 'Displayed prescribed-water depth')) {
      chosen = chosen.map(label => label === 'Initial water depth' ? 'Displayed prescribed-water depth' : label === 'Initial water body' ? 'Displayed water body' : label);
    }
    const primary = node('primary-region-details');
    primary.replaceChildren();
    for (const dt of rows) {
      if (chosen.some(label => dt.textContent!.includes(label)) && dt.nextElementSibling) primary.append(dt.cloneNode(true), dt.nextElementSibling.cloneNode(true));
    }
    const anyRelevant = Object.values(relevant).some(layers => layers.includes(layer));
    node('factors-empty').hidden = anyRelevant;
    for (const [id, layers] of Object.entries(relevant)) node(id).closest<HTMLElement>('.boundary-inspector')!.hidden = !layers.includes(layer);
  }

  private renderExecutionState(): void {
    const state = this.state;
    const collapsed = node('dock-content').hidden;
    node('pause-active').hidden = !state || !(state.seasonalRunning && (collapsed || this.experimentTab !== 'seasonal')
      || state.diagnosticRunning && (collapsed || this.experimentTab !== 'diagnostic'));
    node('experiment-state').textContent = !state ? 'Preparing world'
      : state.seasonalRunning && this.experimentTab !== 'seasonal' ? 'Seasonal water running'
      : state.diagnosticRunning && this.experimentTab !== 'diagnostic' ? 'Diagnostic running'
      : this.experimentTab === 'manual' ? `Step ${state.manualStep} · no clock`
      : this.experimentTab === 'diagnostic' ? state.diagnosticRunning ? 'Running' : 'Paused'
      : this.experimentTab === 'vegetation' ? state.vegetationWorking ? 'Working…' : state.vegetationYear === null ? 'Not initialized' : `Year ${state.vegetationYear}`
      : state.seasonalFinished ? 'Limit reached' : state.seasonalRunning ? 'Running' : state.seasonalWorking ? state.seasonalModel ? 'Finishing interval…' : 'Initializing…' : state.seasonalModel ? 'Paused' : 'Not initialized';
  }

  private bindTabs(selector: string, key: string, change: (value: string) => void): void {
    const tabs = [...document.querySelectorAll<HTMLButtonElement>(selector)];
    const activate = (tab: HTMLButtonElement) => {
      for (const item of tabs) { item.setAttribute('aria-selected', String(item === tab)); item.tabIndex = item === tab ? 0 : -1; }
      change(tab.dataset[key]!);
    };
    for (const tab of tabs) {
      tab.addEventListener('click', () => activate(tab));
      tab.addEventListener('keydown', event => {
        if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
        const index = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1
          : (tabs.indexOf(tab) + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
        activate(tabs[index]); tabs[index].focus(); event.preventDefault();
      });
    }
  }

  private filterLayers(): void {
    const query = this.filter.value.trim().toLowerCase();
    let count = 0;
    this.filtering = true;
    for (const group of this.groups) {
      let matches = 0;
      for (const button of group.querySelectorAll<HTMLButtonElement>('[data-layer]')) {
        button.hidden = !button.textContent!.toLowerCase().includes(query);
        if (!button.hidden) matches++;
      }
      count += matches; group.hidden = matches === 0;
      group.open = query ? true : !this.preferences.closedGroups?.includes(group.dataset.group!);
    }
    this.filtering = false;
    node('layer-empty').hidden = count > 0;
    node('layer-search-result').textContent = `${count} matching layers`;
  }

  private togglePane(pane: 'layers' | 'inspector', visible?: boolean): void {
    const overlay = window.innerWidth <= (pane === 'layers' ? 760 : 1150);
    const token = `${pane}-${overlay ? 'overlay' : 'hidden'}`;
    const show = visible ?? (overlay ? !document.body.classList.contains(token) : document.body.classList.contains(token));
    document.body.classList.toggle(`${pane}-hidden`, !show);
    document.body.classList.toggle(`${pane}-overlay`, show && overlay);
    if (show && overlay) document.body.classList.remove(`${pane === 'layers' ? 'inspector' : 'layers'}-overlay`);
    this.syncPaneAccessibility();
    if (!show) node(`toggle-${pane}`).focus();
  }

  private syncPaneAccessibility(): void {
    for (const pane of ['layers', 'inspector'] as const) {
      const overlay = window.innerWidth <= (pane === 'layers' ? 760 : 1150);
      const visible = !document.body.classList.contains(`${pane}-hidden`) && (!overlay || document.body.classList.contains(`${pane}-overlay`));
      node(`${pane}-pane`).inert = !visible;
      node(`toggle-${pane}`).setAttribute('aria-expanded', String(visible));
    }
  }

  private bindResizer(pane: 'layers' | 'inspector', minimum: number, maximum: number, initial: number): void {
    const handle = node(`resize-${pane}`);
    const preference = pane === 'layers' ? 'layersWidth' : 'inspectorWidth';
    const stored = this.preferences[preference];
    let width = typeof stored === 'number' && Number.isFinite(stored) ? Math.max(minimum, Math.min(maximum, stored)) : initial;
    const apply = () => {
      document.documentElement.style.setProperty(`--${pane}-width`, `${width}px`);
      handle.setAttribute('aria-valuenow', String(width));
    };
    const save = () => { this.preferences[preference] = width; this.savePreferences(); };
    apply();
    handle.addEventListener('pointerdown', event => {
      if (event.button !== 0) return;
      const origin = event.clientX, start = width;
      handle.setPointerCapture(event.pointerId); document.body.classList.add('resizing-pane');
      const move = (moveEvent: PointerEvent) => {
        width = Math.round(Math.max(minimum, Math.min(maximum, start + (moveEvent.clientX - origin) * (pane === 'layers' ? 1 : -1)))); apply();
      };
      const finish = () => {
        handle.removeEventListener('pointermove', move); handle.removeEventListener('lostpointercapture', finish);
        document.body.classList.remove('resizing-pane'); save();
      };
      handle.addEventListener('pointermove', move); handle.addEventListener('lostpointercapture', finish);
      event.preventDefault();
    });
    handle.addEventListener('keydown', event => {
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      width = event.key === 'Home' ? minimum : event.key === 'End' ? maximum
        : Math.max(minimum, Math.min(maximum, width + (event.key === 'ArrowRight' ? 10 : -10) * (pane === 'layers' ? 1 : -1)));
      apply(); save(); event.preventDefault();
    });
    handle.addEventListener('dblclick', () => { width = initial; apply(); save(); });
  }

  private savePreferences(): void {
    try { localStorage.setItem('planimulation.workspace.v1', JSON.stringify(this.preferences)); } catch { /* Presentation preferences are optional. */ }
  }

  private readonly copyTimers = new WeakMap<HTMLButtonElement, ReturnType<typeof setTimeout>>();

  private async copyText(button: HTMLButtonElement, text: string): Promise<void> {
    const label = button.dataset.copyLabel ?? button.getAttribute('aria-label') ?? 'Copy';
    button.dataset.copyLabel = label;
    clearTimeout(this.copyTimers.get(button));
    try {
      await window.desktop.copyText(text);
      button.dataset.copyState = 'copied';
      button.setAttribute('aria-label', 'Copied to clipboard');
      button.title = 'Copied to clipboard';
    } catch {
      button.dataset.copyState = 'failed';
      button.setAttribute('aria-label', 'Copy failed. Try again.');
      button.title = 'Copy failed. Try again.';
    }
    this.copyTimers.set(button, setTimeout(() => {
      delete button.dataset.copyState;
      button.setAttribute('aria-label', label);
      button.title = label;
    }, 2000));
  }
}
