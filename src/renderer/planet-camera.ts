import { PerspectiveCamera, Quaternion, Raycaster, Sphere, Vector2, Vector3 } from 'three';

const radians = Math.PI / 180;
const wrap = (angle: number): number => Math.atan2(Math.sin(angle), Math.cos(angle));
const clamp = (value: number, low: number, high: number): number => Math.max(low, Math.min(high, value));

export interface PlanetView {
  heading: number;
  tilt: number;
  latitude: number;
  longitude: number;
  distance: number;
}

/** A surface-relative viewing camera. All distances use the display reference radius. */
export class PlanetCamera {
  enabled = false;
  wasDragged = false;
  private focus = new Vector3();
  private targetFocus = new Vector3();
  private heading = 0;
  private targetHeading = 0;
  private tilt = 0;
  private targetTilt = 0;
  private distance = 2.25;
  private targetDistance = 2.25;
  private radius = 1;
  private frame = 0;
  private previousTime = 0;
  private inertia = new Vector2();
  private readonly pointers = new Map<number, Vector2>();
  private pointerOrigin = new Vector2();
  private gesture = 'pan';
  private pinchDistance = 0;
  private pinchAngle = 0;
  private pointerTime = 0;
  private overview = true;
  private readonly raycaster = new Raycaster();
  private readonly abort = new AbortController();
  private readonly reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

  constructor(private readonly camera: PerspectiveCamera, private readonly canvas: HTMLCanvasElement,
    private readonly changed: (view: PlanetView) => void) {
    const signal = this.abort.signal;
    canvas.addEventListener('pointerdown', this.pointerDown, { signal });
    canvas.addEventListener('pointermove', this.pointerMove, { signal });
    canvas.addEventListener('pointerup', this.pointerUp, { signal });
    canvas.addEventListener('pointercancel', this.pointerCancel, { signal });
    canvas.addEventListener('lostpointercapture', this.pointerCancel, { signal });
    canvas.addEventListener('wheel', this.wheel, { passive: false, signal });
    canvas.addEventListener('contextmenu', event => { if (this.enabled) event.preventDefault(); }, { signal });
    canvas.addEventListener('dblclick', event => {
      if (!this.enabled) return;
      event.preventDefault(); this.zoom(2, event.clientX, event.clientY);
    }, { signal });
    canvas.addEventListener('keydown', this.keyDown, { signal });
    window.addEventListener('blur', () => this.cancelGesture(), { signal });
    this.reset(false);
  }

  get view(): PlanetView {
    return { heading: (this.heading / radians + 360) % 360, tilt: this.tilt / radians,
      latitude: Math.asin(clamp(this.focus.y, -1, 1)) / radians,
      longitude: -Math.atan2(this.focus.z, this.focus.x) / radians, distance: this.distance };
  }

  setEnabled(enabled: boolean): void {
    this.cancelGesture(); this.enabled = enabled;
    if (enabled) this.apply();
  }

  setRadius(radius: number): void {
    this.radius = Math.max(1, radius);
    if (this.overview) this.distance = this.targetDistance = this.overviewDistance();
    this.distance = Math.max(this.distance, this.minimumDistance(this.tilt));
    this.targetDistance = Math.max(this.targetDistance, this.minimumDistance(this.targetTilt));
    if (this.enabled) this.apply();
  }

  private minimumDistance(tilt: number): number {
    return .008 / Math.max(.25, Math.cos(tilt));
  }

  private overviewDistance(): number {
    const halfAngle = Math.atan(Math.tan(this.camera.fov * radians / 2) * Math.min(1, this.camera.aspect));
    return Math.max(2.25, this.radius * (1.12 / Math.sin(halfAngle) - 1));
  }

  resize(): void {
    if (!this.overview) return;
    this.distance = this.targetDistance = this.overviewDistance();
    if (this.enabled) this.apply();
  }

  reset(animate = true): void {
    this.frameDirection(new Vector3(1, 0, 0), animate);
  }

  frameDirection(direction: Vector3, animate = true): void {
    this.stop();
    this.overview = true;
    this.targetFocus.copy(direction).normalize();
    this.targetHeading = 0; this.targetTilt = 0; this.targetDistance = this.overviewDistance();
    if (!animate) this.finish(); else this.schedule();
  }

  north(): void { this.inertia.set(0, 0); this.targetHeading = 0; this.schedule(); }
  topDown(): void { this.inertia.set(0, 0); this.targetTilt = 0; this.schedule(); }
  toggleTilt(): void { this.overview = false; this.inertia.set(0, 0); this.targetTilt = this.targetTilt > .1 ? 0 : 50 * radians; this.schedule(); }
  rotate(degrees: number): void { this.inertia.set(0, 0); this.targetHeading = wrap(this.targetHeading + degrees * radians); this.schedule(); }

  stop(): void {
    cancelAnimationFrame(this.frame); this.frame = 0; this.previousTime = 0;
    this.inertia.set(0, 0);
    this.targetFocus.copy(this.focus); this.targetHeading = this.heading;
    this.targetTilt = this.tilt; this.targetDistance = this.distance;
  }

  private cancelGesture(): void {
    for (const id of this.pointers.keys()) {
      if (this.canvas.hasPointerCapture(id)) this.canvas.releasePointerCapture(id);
    }
    this.pointers.clear(); this.stop();
    this.canvas.classList.remove('dragging'); document.body.classList.remove('map-dragging');
  }

  private pose(focus: Vector3, heading: number, tilt: number, distance: number): void {
    const longitude = Math.atan2(focus.z, focus.x);
    const east = new Vector3(Math.sin(longitude), 0, -Math.cos(longitude));
    const north = new Vector3().crossVectors(focus, east).normalize();
    const forward = north.multiplyScalar(Math.cos(heading)).addScaledVector(east, Math.sin(heading));
    const target = focus.clone().multiplyScalar(this.radius);
    const offset = focus.clone().multiplyScalar(Math.cos(tilt)).addScaledVector(forward, -Math.sin(tilt));
    this.camera.position.copy(target).addScaledVector(offset, distance);
    this.camera.up.copy(focus).multiplyScalar(Math.sin(tilt)).addScaledVector(forward, Math.cos(tilt));
    this.camera.lookAt(target); this.camera.updateMatrixWorld();
  }

  private apply(): void {
    this.pose(this.focus, this.heading, this.tilt, this.distance);
    if (this.enabled) this.changed(this.view);
  }

  private finish(): void {
    this.focus.copy(this.targetFocus); this.heading = this.targetHeading;
    this.tilt = this.targetTilt; this.distance = this.targetDistance; this.apply();
  }

  private schedule(): void {
    this.targetDistance = Math.max(this.targetDistance, this.minimumDistance(this.targetTilt));
    if (this.reducedMotion.matches) { this.finish(); return; }
    if (!this.frame) this.frame = requestAnimationFrame(this.animate);
  }

  private readonly animate = (now: number): void => {
    this.frame = 0;
    const delta = this.previousTime ? Math.min(48, now - this.previousTime) : 16.7;
    this.previousTime = now;
    if (this.inertia.lengthSq() > .015) {
      this.panPixels(this.inertia.x * delta, this.inertia.y * delta, false);
      this.inertia.multiplyScalar(Math.exp(-delta / 115));
    }
    const mix = 1 - Math.exp(-delta / 65);
    const rotation = new Quaternion().setFromUnitVectors(this.focus, this.targetFocus);
    this.focus.applyQuaternion(new Quaternion().slerp(rotation, mix)).normalize();
    this.heading = wrap(this.heading + wrap(this.targetHeading - this.heading) * mix);
    this.tilt += (this.targetTilt - this.tilt) * mix;
    this.distance += (this.targetDistance - this.distance) * mix;
    const pending = this.focus.distanceToSquared(this.targetFocus) > 1e-12 || Math.abs(wrap(this.targetHeading - this.heading)) > 1e-5
      || Math.abs(this.targetTilt - this.tilt) > 1e-5 || Math.abs(this.targetDistance - this.distance) > 1e-5 || this.inertia.lengthSq() > .015;
    if (pending) { this.apply(); this.frame = requestAnimationFrame(this.animate); }
    else { this.previousTime = 0; this.finish(); }
  };

  private hit(x: number, y: number): Vector3 | null {
    const bounds = this.canvas.getBoundingClientRect();
    this.raycaster.setFromCamera(new Vector2((x - bounds.left) / bounds.width * 2 - 1, 1 - (y - bounds.top) / bounds.height * 2), this.camera);
    return this.raycaster.ray.intersectSphere(new Sphere(new Vector3(), this.radius), new Vector3())?.normalize() ?? null;
  }

  zoom(factor: number, x?: number, y?: number): void {
    if (!Number.isFinite(factor) || factor <= 0) return;
    this.overview = false; this.inertia.set(0, 0);
    const anchor = x !== undefined && y !== undefined ? this.hit(x, y) : null;
    this.targetDistance = clamp(this.targetDistance / factor, this.minimumDistance(this.targetTilt), 12);
    if (anchor && x !== undefined && y !== undefined) {
      this.pose(this.targetFocus, this.targetHeading, this.targetTilt, this.targetDistance);
      const after = this.hit(x, y);
      if (after) this.targetFocus.applyQuaternion(new Quaternion().setFromUnitVectors(after, anchor)).normalize();
      this.pose(this.focus, this.heading, this.tilt, this.distance);
    }
    this.schedule();
  }

  private panPixels(dx: number, dy: number, immediate = true): void {
    this.overview = false;
    const longitude = Math.atan2(this.targetFocus.z, this.targetFocus.x);
    const east = new Vector3(Math.sin(longitude), 0, -Math.cos(longitude));
    const north = new Vector3().crossVectors(this.targetFocus, east).normalize();
    const right = east.clone().multiplyScalar(Math.cos(this.heading)).addScaledVector(north, -Math.sin(this.heading));
    const forward = north.multiplyScalar(Math.cos(this.heading)).addScaledVector(east, Math.sin(this.heading));
    const scale = Math.min(2.5, this.distance) * 2 * Math.tan(this.camera.fov * radians / 2) / Math.max(1, this.canvas.clientHeight) / this.radius;
    this.targetFocus.addScaledVector(right, -dx * scale).addScaledVector(forward, dy * scale).normalize();
    if (Math.abs(this.targetFocus.y) > .999998) this.targetFocus.y = Math.sign(this.targetFocus.y) * .999998;
    this.targetFocus.normalize();
    if (immediate) { this.focus.copy(this.targetFocus); this.apply(); }
  }

  private readonly pointerDown = (event: PointerEvent): void => {
    if (!this.enabled || event.button > 2) return;
    this.stop(); this.canvas.focus({ preventScroll: true });
    this.canvas.setPointerCapture(event.pointerId);
    this.pointers.set(event.pointerId, new Vector2(event.clientX, event.clientY));
    if (this.pointers.size === 1) {
      this.wasDragged = event.button !== 0 || event.shiftKey || event.ctrlKey || event.metaKey;
      this.pointerOrigin.set(event.clientX, event.clientY);
      this.gesture = event.button === 2 ? 'zoom' : event.button === 1 || event.shiftKey || event.ctrlKey || event.metaKey ? 'orbit' : 'pan';
      this.pointerTime = event.timeStamp;
    } else {
      this.wasDragged = true;
      const [a, b] = [...this.pointers.values()];
      this.pinchDistance = a.distanceTo(b); this.pinchAngle = Math.atan2(b.y - a.y, b.x - a.x);
    }
    this.canvas.classList.add('dragging'); document.body.classList.add('map-dragging');
    event.preventDefault();
  };

  private readonly pointerMove = (event: PointerEvent): void => {
    const previous = this.pointers.get(event.pointerId);
    if (!this.enabled || !previous) return;
    this.overview = false;
    const dx = event.clientX - previous.x, dy = event.clientY - previous.y;
    const elapsed = Math.max(8, event.timeStamp - this.pointerTime);
    this.pointerTime = event.timeStamp;
    if (this.pointerOrigin.distanceTo(new Vector2(event.clientX, event.clientY)) > 4) this.wasDragged = true;
    previous.set(event.clientX, event.clientY);
    if (this.pointers.size > 1) {
      const [a, b] = [...this.pointers.values()];
      const distance = a.distanceTo(b), angle = Math.atan2(b.y - a.y, b.x - a.x);
      this.panPixels(dx / 2, dy / 2);
      this.targetHeading = wrap(this.heading - wrap(angle - this.pinchAngle)); this.heading = this.targetHeading;
      if (this.pinchDistance > 0) this.zoom(distance / this.pinchDistance, (a.x + b.x) / 2, (a.y + b.y) / 2);
      this.pinchDistance = distance; this.pinchAngle = angle;
      this.inertia.set(0, 0); this.apply();
    } else if (this.gesture === 'orbit') {
      this.targetHeading = wrap(this.heading - dx * .006);
      this.targetTilt = clamp(this.tilt + dy * .004, 0, 75 * radians);
      this.targetDistance = Math.max(this.distance, this.minimumDistance(this.targetTilt));
      this.heading = this.targetHeading; this.tilt = this.targetTilt; this.distance = this.targetDistance; this.apply();
    } else if (this.gesture === 'zoom') {
      this.zoom(Math.exp(-dy * .012), this.pointerOrigin.x, this.pointerOrigin.y);
    } else {
      const before = this.hit(event.clientX - dx, event.clientY - dy), after = this.hit(event.clientX, event.clientY);
      if (before && after && this.tilt < .3) {
        this.targetFocus.applyQuaternion(new Quaternion().setFromUnitVectors(after, before)).normalize();
        this.focus.copy(this.targetFocus); this.apply();
      } else this.panPixels(dx, dy);
      this.inertia.set(dx / elapsed, dy / elapsed).clampLength(0, 1.5);
    }
    event.preventDefault();
  };

  private readonly pointerUp = (event: PointerEvent): void => {
    if (!this.pointers.has(event.pointerId)) return;
    this.pointers.delete(event.pointerId);
    if (this.canvas.hasPointerCapture(event.pointerId)) this.canvas.releasePointerCapture(event.pointerId);
    if (!this.pointers.size) {
      this.canvas.classList.remove('dragging'); document.body.classList.remove('map-dragging');
      if (this.wasDragged && this.gesture === 'pan' && event.timeStamp - this.pointerTime < 90 && !this.reducedMotion.matches) this.schedule();
      else this.inertia.set(0, 0);
    } else {
      this.wasDragged = true; this.inertia.set(0, 0);
    }
  };

  private readonly pointerCancel = (event: PointerEvent): void => {
    if (!this.pointers.has(event.pointerId)) return;
    this.wasDragged = true; this.pointers.delete(event.pointerId); this.inertia.set(0, 0);
    if (!this.pointers.size) { this.canvas.classList.remove('dragging'); document.body.classList.remove('map-dragging'); }
  };

  private readonly wheel = (event: WheelEvent): void => {
    if (!this.enabled) return;
    event.preventDefault();
    const pixels = event.deltaY * (event.deltaMode === WheelEvent.DOM_DELTA_LINE ? 16 : event.deltaMode === WheelEvent.DOM_DELTA_PAGE ? this.canvas.clientHeight : 1);
    if (event.shiftKey) { this.targetTilt = clamp(this.targetTilt + pixels * .002, 0, 75 * radians); this.schedule(); }
    else this.zoom(Math.exp(-clamp(pixels, -180, 180) * .0025), event.clientX, event.clientY);
  };

  private readonly keyDown = (event: KeyboardEvent): void => {
    if (!this.enabled || event.ctrlKey || event.metaKey) return;
    const key = event.key.toLowerCase();
    const slow = event.altKey ? .25 : 1;
    if (key === 'n') this.north();
    else if (key === 'u') this.topDown();
    else if (key === 'o') this.toggleTilt();
    else if (key === 'r' || key === 'home') this.reset();
    else if (key === ' ') this.stop();
    else if (key === '+' || key === '=' || key === 'pageup') this.zoom(1.5 ** slow);
    else if (key === '-' || key === 'pagedown') this.zoom(1 / 1.5 ** slow);
    else if (key.startsWith('arrow')) {
      if (event.shiftKey) {
        if (key === 'arrowleft' || key === 'arrowright') this.targetHeading = wrap(this.targetHeading + (key === 'arrowleft' ? 1 : -1) * 10 * radians * slow);
        else this.targetTilt = clamp(this.targetTilt + (key === 'arrowdown' ? 1 : -1) * 8 * radians * slow, 0, 75 * radians);
        this.schedule();
      } else {
        this.stop(); this.panPixels((key === 'arrowleft' ? 32 : key === 'arrowright' ? -32 : 0) * slow,
          (key === 'arrowup' ? 32 : key === 'arrowdown' ? -32 : 0) * slow); this.schedule();
      }
    } else return;
    event.preventDefault();
  };

  dispose(): void { this.cancelGesture(); this.abort.abort(); }
}
