import { locateRegion } from '../core/surface';
import type { World } from '../core/world';
import { directionAt, projectRegion } from './projection';

export type Layer = 'signal' | 'area' | 'latitude';

function color(value: number): string {
  const t = Math.max(0, Math.min(1, value));
  const a = t < .5 ? [41, 72, 85] : [100, 137, 133];
  const b = t < .5 ? [100, 137, 133] : [185, 203, 160];
  const k = t < .5 ? t * 2 : (t - .5) * 2;
  return `rgb(${a.map((v, i) => Math.round(v + (b[i] - v) * k)).join(',')})`;
}

export class SurfaceMap {
  private context: CanvasRenderingContext2D;
  private world: World | null = null;
  private paths: Path2D[] = [];
  private layer: Layer = 'signal';
  private boundaries = true;
  private zoom = 1;
  private panX = 0;
  private panY = 0;
  private selected: number | null = null;
  private drag: { x: number; y: number; total: number } | null = null;

  constructor(private canvas: HTMLCanvasElement, private onSelect: (id: number) => void) {
    const context = canvas.getContext('2d');
    if (!context) throw new Error('A 2D canvas context is required.');
    this.context = context;
    new ResizeObserver(() => this.draw()).observe(canvas);
    canvas.addEventListener('wheel', (event) => {
      event.preventDefault();
      const rect = canvas.getBoundingClientRect();
      this.setZoom(this.zoom * Math.exp(-event.deltaY * .0015), event.clientX - rect.left, event.clientY - rect.top);
    }, { passive: false });
    canvas.addEventListener('pointerdown', (event) => {
      if (event.button !== 0) return;
      this.drag = { x: event.clientX, y: event.clientY, total: 0 };
      canvas.setPointerCapture(event.pointerId);
    });
    canvas.addEventListener('pointermove', (event) => {
      if (!this.drag) return;
      const dx = event.clientX - this.drag.x, dy = event.clientY - this.drag.y;
      this.drag.total += Math.hypot(dx, dy);
      this.panX += dx; this.panY += dy;
      this.drag.x = event.clientX; this.drag.y = event.clientY;
      if (this.drag.total > 4) canvas.classList.add('dragging');
      this.draw();
    });
    canvas.addEventListener('pointerup', (event) => {
      if (!this.drag) return;
      const click = this.drag.total < 4;
      this.drag = null;
      canvas.classList.remove('dragging');
      if (!click || !this.world) return;
      const rect = canvas.getBoundingClientRect(), view = this.view();
      const u = (event.clientX - rect.left - view.x) / view.width;
      const v = (event.clientY - rect.top - view.y) / view.height;
      if (v < 0 || v > 1) return;
      this.selected = locateRegion(this.world.surface, directionAt(((u % 1) + 1) % 1, v));
      this.onSelect(this.selected);
      this.draw();
    });
    canvas.addEventListener('pointercancel', () => { this.drag = null; canvas.classList.remove('dragging'); });
  }

  setWorld(world: World): void {
    this.world = world;
    this.paths = Array.from({ length: world.stats.regionCount }, (_, id) => {
      const polygon = projectRegion(world.surface, id), path = new Path2D();
      path.moveTo(...polygon[0]);
      for (const point of polygon.slice(1)) path.lineTo(...point);
      path.closePath();
      return path;
    });
    this.selected = null;
    this.reset();
  }

  setLayer(layer: Layer): void { this.layer = layer; this.draw(); }
  setBoundaries(value: boolean): void { this.boundaries = value; this.draw(); }
  reset(): void { this.zoom = 1; this.panX = 0; this.panY = 0; this.draw(); }
  zoomBy(factor: number): void { this.setZoom(this.zoom * factor, this.canvas.clientWidth / 2, this.canvas.clientHeight / 2); }

  private view(): { x: number; y: number; width: number; height: number } {
    const width = Math.min(this.canvas.clientWidth - 36, (this.canvas.clientHeight - 60) * 2) * this.zoom;
    return { x: (this.canvas.clientWidth - width) / 2 + this.panX,
      y: (this.canvas.clientHeight - width / 2) / 2 - 8 + this.panY, width, height: width / 2 };
  }

  private setZoom(value: number, x: number, y: number): void {
    const before = this.view();
    const u = (x - before.x) / before.width, v = (y - before.y) / before.height;
    this.zoom = Math.max(1, Math.min(12, value));
    const after = this.view();
    this.panX += x - (after.x + u * after.width);
    this.panY += y - (after.y + v * after.height);
    this.draw();
  }

  private draw(): void {
    const { canvas, context: ctx, world } = this;
    const ratio = window.devicePixelRatio || 1;
    const pixelWidth = Math.round(canvas.clientWidth * ratio), pixelHeight = Math.round(canvas.clientHeight * ratio);
    if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) { canvas.width = pixelWidth; canvas.height = pixelHeight; }
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.fillStyle = '#0c1417'; ctx.fillRect(0, 0, canvas.clientWidth, canvas.clientHeight);
    if (!world) return;
    const view = this.view();
    // Bound vertical panning and wrap longitude so the planet has no horizontal edge.
    if (view.height > canvas.clientHeight - 40) {
      this.panY += Math.min(20 - view.y, Math.max(canvas.clientHeight - 40 - view.y - view.height, 0));
    } else this.panY = 0;
    this.panX = ((this.panX + view.width / 2) % view.width + view.width) % view.width - view.width / 2;
    const v = this.view();
    ctx.save();
    ctx.beginPath(); ctx.rect(12, Math.max(12, v.y), canvas.clientWidth - 24, Math.min(v.height, canvas.clientHeight - Math.max(12, v.y) - 32)); ctx.clip();
    const minShift = Math.floor(-v.x / v.width) - 1;
    const maxShift = Math.ceil((canvas.clientWidth - v.x) / v.width) + 1;
    const transform = (shift: number): void => ctx.setTransform(ratio * v.width, 0, 0, ratio * v.height, ratio * (v.x + shift * v.width), ratio * v.y);
    for (let id = 0; id < this.paths.length; id++) {
      let value = .5 + world.diagnosticField[id] * .5;
      if (this.layer === 'area') value = (world.surface.areasSquareMeters[id] - world.stats.minimumAreaSquareMeters)
        / (world.stats.maximumAreaSquareMeters - world.stats.minimumAreaSquareMeters || 1);
      if (this.layer === 'latitude') value = 1 - Math.abs(Math.asin(world.surface.centers[id * 3 + 1])) / (Math.PI / 2);
      ctx.fillStyle = color(value);
      for (let shift = minShift; shift <= maxShift; shift++) {
        transform(shift); ctx.fill(this.paths[id]);
        ctx.strokeStyle = this.boundaries ? '#14262b66' : ctx.fillStyle;
        ctx.lineWidth = (this.boundaries ? .55 : .4) / v.width;
        ctx.stroke(this.paths[id]);
      }
    }
    transform(0); ctx.strokeStyle = '#b6d3cc28'; ctx.lineWidth = .7 / v.width;
    ctx.beginPath();
    for (let lat = 1; lat < 6; lat++) { ctx.moveTo(minShift, lat / 6); ctx.lineTo(maxShift + 1, lat / 6); }
    for (let shift = minShift; shift <= maxShift; shift++) {
      for (let lon = 0; lon < 12; lon++) { ctx.moveTo(shift + lon / 12, 0); ctx.lineTo(shift + lon / 12, 1); }
    }
    ctx.stroke();
    if (this.selected !== null) {
      for (let shift = minShift; shift <= maxShift; shift++) {
        transform(shift); ctx.strokeStyle = '#ecf6c9'; ctx.lineWidth = 2 / v.width; ctx.stroke(this.paths[this.selected]);
      }
    }
    ctx.restore();
  }
}
