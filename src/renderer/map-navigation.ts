import type { NavigationView, SurfaceMap } from './map';

const element = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;

/** Viewport controls share the workspace's presentation vocabulary. */
export class MapNavigation {
  private readonly compass = element<HTMLButtonElement>('compass');
  private readonly abort = new AbortController();
  private compassPointer: number | null = null;
  private compassAngle = 0;
  private compassTravel = 0;
  private compassRing = true;
  private compassX = 0;
  private readonly distance = new Intl.NumberFormat('en', { maximumFractionDigits: 0 });

  constructor(map: SurfaceMap) {
    const signal = this.abort.signal;
    element('map').addEventListener('viewchange', event => this.update((event as CustomEvent<NavigationView>).detail), { signal });
    element('map').addEventListener('keydown', event => {
      if (event.key === '?') { element<HTMLDialogElement>('navigation-dialog').showModal(); event.preventDefault(); }
    }, { signal });
    element('tilt-view').addEventListener('click', () => map.toggleTilt(), { signal });
    element<HTMLInputElement>('show-stars').addEventListener('change', event => map.setStars((event.target as HTMLInputElement).checked), { signal });
    element<HTMLInputElement>('show-atmosphere').addEventListener('change', event => map.setAtmosphere((event.target as HTMLInputElement).checked), { signal });
    element<HTMLInputElement>('distant-lighting').addEventListener('change', event => map.setDistantLighting((event.target as HTMLInputElement).checked), { signal });
    this.compass.addEventListener('pointerdown', event => {
      if (event.button !== 0 || map.navigation.mode !== 'globe') return;
      map.stopCamera(); this.compassPointer = event.pointerId; this.compassTravel = 0;
      const bounds = this.compass.getBoundingClientRect();
      this.compassRing = Math.hypot(event.clientX - bounds.left - bounds.width / 2, event.clientY - bounds.top - bounds.height / 2) > bounds.width * .22;
      this.compassX = event.clientX;
      this.compassAngle = this.angle(event); this.compass.setPointerCapture(event.pointerId);
    }, { signal });
    this.compass.addEventListener('pointermove', event => {
      if (event.pointerId !== this.compassPointer) return;
      const angle = this.angle(event), delta = this.compassRing
        ? Math.atan2(Math.sin(angle - this.compassAngle), Math.cos(angle - this.compassAngle)) : (event.clientX - this.compassX) * .012;
      this.compassTravel += Math.abs(delta);
      map.rotateView(-delta * 180 / Math.PI); this.compassAngle = angle; this.compassX = event.clientX;
    }, { signal });
    this.compass.addEventListener('pointerup', event => {
      if (event.pointerId !== this.compassPointer) return;
      this.compassPointer = null;
      if (this.compass.hasPointerCapture(event.pointerId)) this.compass.releasePointerCapture(event.pointerId);
    }, { signal });
    for (const event of ['pointercancel', 'lostpointercapture']) this.compass.addEventListener(event, () => { this.compassPointer = null; }, { signal });
    this.compass.addEventListener('click', () => { if (this.compassTravel < .04) map.northUp(); this.compassTravel = 0; }, { signal });
    this.compass.addEventListener('keydown', event => {
      if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { map.rotateView(event.key === 'ArrowLeft' ? -15 : 15); event.preventDefault(); }
      else if (event.key === 'Home' || event.key.toLowerCase() === 'n') { map.northUp(); event.preventDefault(); }
    }, { signal });
    this.update(map.navigation);
  }

  private angle(event: PointerEvent): number {
    const bounds = this.compass.getBoundingClientRect();
    return Math.atan2(event.clientY - bounds.top - bounds.height / 2, event.clientX - bounds.left - bounds.width / 2);
  }

  private update(view: NavigationView): void {
    const globe = view.mode === 'globe';
    element('map-container').dataset.view = view.mode;
    element('sky-credit').hidden = !globe || element('map').dataset.sky !== 'ready' || element('map').dataset.skyVisible === 'false';
    this.compass.style.setProperty('--heading', `${-view.heading}deg`);
    const heading = Math.round(view.heading) % 360;
    this.compass.setAttribute('aria-label', globe ? `Heading ${heading} degrees. Reset north; drag to rotate.` : 'North-up map');
    this.compass.title = globe ? `Heading ${heading}° · Click for north · Drag to rotate` : 'North-up map';
    this.compass.setAttribute('aria-disabled', String(!globe));
    element('compass-heading').textContent = globe ? `${String(heading).padStart(3, '0')}°` : 'North up';
    const tilt = element<HTMLButtonElement>('tilt-view');
    tilt.hidden = !globe;
    tilt.textContent = view.tilt > 1 ? 'Top down' : 'Tilt view';
    tilt.title = view.tilt > 1 ? 'Return to top-down view (U)' : 'Tilt the globe (O)';
    tilt.setAttribute('aria-pressed', String(view.tilt > 1));
    element('reset-view').title = globe ? 'Reset globe view (R)' : 'Fit map (R)';
    element('projection-label').textContent = globe ? 'Globe · computed relief' : 'Equirectangular projection';
    element('map-gesture-hint').textContent = globe ? 'Drag to move · Shift + drag to tilt · Scroll to zoom' : 'Drag to pan · Scroll to zoom · Click to inspect';
    element('camera-position').hidden = !globe;
    element('camera-position').textContent = `${Math.abs(view.latitude).toFixed(1)}° ${view.latitude >= 0 ? 'N' : 'S'} · ${Math.abs(view.longitude).toFixed(1)}° ${view.longitude >= 0 ? 'E' : 'W'}`;
    element('camera-distance').hidden = !globe || !view.radiusMeters;
    element('camera-distance').textContent = `${this.distance.format(view.distance * view.radiusMeters / 1000)} km`;
    element<HTMLInputElement>('show-stars').disabled = !globe;
    element<HTMLInputElement>('show-atmosphere').disabled = !globe || element('map').dataset.activeLayer !== 'surface';
    element<HTMLInputElement>('distant-lighting').disabled = !globe || element('map').dataset.activeLayer !== 'surface';
    element('distant-lighting-note').textContent = view.radiusMeters
      ? `Day and night fade in from ${this.distance.format(view.radiusMeters * 3 / 1000)} to ${this.distance.format(view.radiusMeters * 5 / 1000)} km. View lighting is independent of experiment time.`
      : 'Day and night appear when zoomed out. View lighting is independent of experiment time.';
  }

  dispose(): void { this.abort.abort(); }
}
