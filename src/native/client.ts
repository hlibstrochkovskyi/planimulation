import { spawn } from 'node:child_process';
import type { ChildProcessWithoutNullStreams } from 'node:child_process';
import { parseRecipe } from '../core/recipe';
import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import { validateWater } from '../core/water';
import { validateDrainage } from '../core/drainage';
import { BASIN_ANALYSIS_VERSION, validateBasins } from '../core/basins';
import type { DiagnosticFrame } from '../shared/desktop-api';

const MAX_BYTES = 32 * 2 ** 20;
type Header = { protocol: number; byteLength: number; kind: string; [key: string]: unknown };
export interface Packet { header: Header; bytes: Buffer }

/** Bounded incremental framing. No repeated concatenation of the entire world. */
export class FrameReader {
  private chunks: Buffer[] = [];
  private length = 0;
  private headerLength: number | null = null;
  private header: Header | null = null;
  constructor(private readonly receive: (packet: Packet) => void) {}
  private take(n: number): Buffer {
    const out = Buffer.allocUnsafe(n);
    let offset = 0;
    while (offset < n) {
      const chunk = this.chunks[0], count = Math.min(chunk.length, n - offset);
      out.set(chunk.subarray(0, count), offset);
      if (count === chunk.length) this.chunks.shift();
      else this.chunks[0] = chunk.subarray(count);
      offset += count;
    }
    this.length -= n;
    return out;
  }
  push(chunk: Buffer): void {
    if (this.length + chunk.length > MAX_BYTES + 32772) throw new Error('Native frame exceeds memory budget.');
    if (chunk.length) { this.chunks.push(chunk); this.length += chunk.length; }
    for (;;) {
      if (this.headerLength === null) {
        if (this.length < 4) return;
        this.headerLength = this.take(4).readUInt32LE();
        if (this.headerLength < 2 || this.headerLength > 32768) throw new Error('Invalid native header size.');
      }
      if (this.header === null) {
        if (this.length < this.headerLength) return;
        const h = JSON.parse(this.take(this.headerLength).toString('utf8')) as Header;
        if (!h || h.protocol !== 7 || !Number.isSafeInteger(h.byteLength) || h.byteLength < 0 || h.byteLength > MAX_BYTES
          || !['world', 'frame', 'error'].includes(h.kind)) throw new Error('Invalid native protocol header.');
        this.header = h;
      }
      if (this.length < this.header.byteLength) return;
      const packet = { header: this.header, bytes: this.take(this.header.byteLength) };
      this.header = null; this.headerLength = null;
      this.receive(packet);
    }
  }
}

function floats(bytes: Buffer): Float64Array {
  if (bytes.length % 8) throw new Error('Invalid float buffer length.');
  const out = new Float64Array(bytes.length / 8);
  for (let i = 0; i < out.length; i++) {
    out[i] = bytes.readDoubleLE(i * 8);
    if (!Number.isFinite(out[i])) throw new Error('Non-finite native field.');
  }
  return out;
}

export function decodeWorld(packet: Packet): World {
  const { header: h, bytes } = packet;
  if (h.kind !== 'world') throw new Error('Expected native world.');
  const recipe = parseRecipe(h.recipe), n = 10 * 4 ** recipe.subdivision + 2;
  const neighbors = 6 * n - 12, faces = 60 * 4 ** recipe.subdivision;
  const b = h.boundarySegmentCount;
  if (typeof b !== 'number' || !Number.isInteger(b) || b < 2 || b > neighbors || b % 2) throw new Error('Invalid native boundary count.');
  const k = h.basinNodeCount;
  if (typeof k !== 'number' || !Number.isInteger(k) || k < 1 || k > 2 * n - 1 || h.basinAnalysisVersion !== BASIN_ANALYSIS_VERSION) throw new Error('Invalid native basin metadata.');
  const expectedBytes = n * 24 + faces * 4 + (n + 1) * 8 + neighbors * 12 + n * 16 + neighbors * 48
    + n * 4 + recipe.plateCount * 28 + b * 76 + 8 + n * 72 + 20 + n * 32 + n * 4 + k * 44;
  if (bytes.length !== expectedBytes) throw new Error('Invalid native world array lengths.');
  let cursor = 0;
  const f64 = (length: number): Float64Array => {
    const result = floats(bytes.subarray(cursor, cursor + length * 8)); cursor += length * 8; return result;
  };
  const u32 = (length: number): Uint32Array => {
    const result = new Uint32Array(length);
    for (let i = 0; i < length; i++) result[i] = bytes.readUInt32LE(cursor + i * 4);
    cursor += length * 4; return result;
  };
  const surface = { radiusMeters: recipe.radiusMeters, centers: f64(n * 3), faces: u32(faces),
    neighborOffsets: u32(n + 1), neighbors: u32(neighbors), neighborDistancesMeters: f64(neighbors),
    areasSquareMeters: f64(n), boundaryOffsets: u32(n + 1), boundaryDirections: f64(neighbors * 6) };
  const diagnosticField = f64(n);
  const tectonics = { owners: u32(n), seeds: u32(recipe.plateCount), angularVelocities: f64(recipe.plateCount * 3),
    boundaryCells: u32(b * 2), boundaryDirections: f64(b * 6), boundaryMotion: f64(b * 2), boundaryTypes: u32(b) };
  const crust = { threshold: f64(1)[0], potential: f64(n), continentality: f64(n), thicknessMeters: f64(n), densityKgPerCubicMeter: f64(n) };
  const terrain = { baseline: f64(n), convergence: f64(n), divergence: f64(n), detail: f64(n), elevation: f64(n) };
  const water = { levelMeters: f64(1)[0], resolvedVolumeCubicMeters: f64(1)[0], depthMeters: f64(n), bodyIds: u32(n), mainOceanId: u32(1)[0] };
  const drainage = { receivers: u32(n), outlets: u32(n), flatSteps: u32(n), contributingArea: f64(n) };
  const basins = { regionNodes: u32(n), parents: u32(k), birthLevels: f64(k), spillLevels: f64(k),
    spillFrom: u32(k), spillTo: u32(k), supportAreas: f64(k), capacities: f64(k) };
  for (let i = 0; i < n; i++) {
    if (terrain.baseline[i] < -4500.000001 || terrain.baseline[i] > 167
      || terrain.convergence[i] < 0 || terrain.convergence[i] > 12000
      || terrain.divergence[i] < -3000 || terrain.divergence[i] > 5000
      || Math.abs(terrain.detail[i]) > recipe.detailAmplitudeMeters + 1e-9
      || Math.abs(terrain.elevation[i] - (terrain.baseline[i] + terrain.convergence[i] + terrain.divergence[i] + terrain.detail[i])) > 1e-8) {
      throw new Error('Invalid native elevation contributions.');
    }
  }
  if (Math.abs(crust.threshold) > 1.12 || crust.potential.some((v) => Math.abs(v) > 1)
    || crust.continentality.some((v) => v < 0 || v > 1)
    || crust.thicknessMeters.some((v) => v < 7000 || v > 35000)
    || crust.densityKgPerCubicMeter.some((v) => v < 2800 || v > 3000)) throw new Error('Invalid native crust fields.');
  if (tectonics.owners.some((v) => v >= recipe.plateCount) || tectonics.seeds.some((v, p) => v >= n || tectonics.owners[v] !== p)
    || new Set(tectonics.seeds).size !== recipe.plateCount || tectonics.boundaryTypes.some((v) => v > 3)
    || tectonics.boundaryCells.some((v) => v >= n)) throw new Error('Invalid native plate metadata.');
  const stats = h.stats as World['stats'];
  if (!stats || stats.regionCount !== n || stats.faceCount !== faces / 3 || stats.edgeCount !== neighbors / 2
    || stats.arrayBytes !== bytes.length || !Object.values(stats).every(Number.isFinite)
    || stats.relativeAreaError < 0 || stats.relativeAreaError > 1e-10
    || typeof h.checksum !== 'string' || !/^[0-9a-f]{8}$/.test(h.checksum)) throw new Error('Invalid native world metadata.');
  if (surface.neighborOffsets[0] !== 0 || surface.boundaryOffsets[0] !== 0
    || surface.neighborOffsets[n] !== neighbors || surface.boundaryOffsets[n] !== neighbors * 2
    || surface.neighbors.some((v) => v >= n) || surface.faces.some((v) => v >= n)
    || surface.areasSquareMeters.some((v) => v <= 0)) throw new Error('Invalid native topology.');
  for (let i = 0; i < n; i++) {
    const degree = surface.neighborOffsets[i + 1] - surface.neighborOffsets[i];
    if ((degree !== 5 && degree !== 6) || surface.boundaryOffsets[i + 1] - surface.boundaryOffsets[i] !== degree * 2) {
      throw new Error('Invalid native offsets.');
    }
  }
  let crossPlateLinks = 0;
  for (let i = 0; i < n; i++) for (let k = surface.neighborOffsets[i]; k < surface.neighborOffsets[i + 1]; k++) {
    if (tectonics.owners[i] !== tectonics.owners[surface.neighbors[k]]) crossPlateLinks++;
  }
  if (crossPlateLinks !== b) throw new Error('Incomplete native plate boundaries.');
  const pairs = new Map<number, number>();
  for (let i = 0; i < b; i++) {
    const a = tectonics.boundaryCells[i * 2], c = tectonics.boundaryCells[i * 2 + 1];
    if (a >= c || tectonics.owners[a] === tectonics.owners[c]
      || !surface.neighbors.subarray(surface.neighborOffsets[a], surface.neighborOffsets[a + 1]).includes(c)) {
      throw new Error('Invalid native plate boundary pair.');
    }
    const key = a * n + c; pairs.set(key, (pairs.get(key) ?? 0) + 1);
  }
  if ([...pairs.values()].some((count) => count !== 2)) throw new Error('Duplicate or missing native boundary segment.');
  validateWater(surface, terrain, water, recipe.water);
  validateDrainage(surface, terrain.elevation, water, drainage);
  validateBasins(surface, terrain.elevation, basins);
  return { recipe, surface, diagnosticField, tectonics, crust, terrain, water, drainage, basins, checksum: h.checksum, stats };
}

export class NativeSession {
  private readonly child: ChildProcessWithoutNullStreams;
  private pending: { resolve: (packet: Packet) => void; reject: (error: Error) => void; timer: NodeJS.Timeout } | null = null;
  private closed = false;
  private stderr = '';
  constructor(executable: string) {
    this.child = spawn(executable, [], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    const reader = new FrameReader((packet) => {
      const pending = this.pending;
      if (!pending) throw new Error('Unsolicited native response.');
      this.pending = null; clearTimeout(pending.timer);
      if (packet.header.kind === 'error') pending.reject(new Error(String(packet.header.message)));
      else pending.resolve(packet);
    });
    this.child.stdout.on('data', (chunk: Buffer) => { try { reader.push(chunk); } catch (e) { this.close(String(e)); } });
    this.child.stderr.on('data', (chunk: Buffer) => { this.stderr = (this.stderr + chunk.toString()).slice(-4096); });
    this.child.on('error', (e) => this.close(e.message));
    this.child.stdin.on('error', (e) => this.close(e.message));
    this.child.on('exit', (code) => this.close(`Native core exited (${code}). ${this.stderr}`));
  }
  request(command: object): Promise<Packet> {
    if (this.closed) return Promise.reject(new Error('Native session is closed.'));
    if (this.pending) return Promise.reject(new Error('Native session is busy.'));
    const line = `${JSON.stringify(command)}\n`;
    if (Buffer.byteLength(line) > 32768) return Promise.reject(new Error('Command too large.'));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.close('Native request timed out.'), 60_000);
      this.pending = { resolve, reject, timer };
      this.child.stdin.write(line);
    });
  }
  close(message = 'Native task canceled.'): void {
    if (this.closed) return;
    this.closed = true;
    if (this.pending) { clearTimeout(this.pending.timer); this.pending.reject(new Error(message)); this.pending = null; }
    this.child.kill();
  }
}

/** Pending generation never mutates the last successfully published world. */
export class NativeController {
  private active: NativeSession | null = null;
  private candidate: NativeSession | null = null;
  private epoch = 0;
  private count = 0;
  private tick = 0;
  private sequence = 0;
  private prepared: { epoch: number; count: number } | null = null;
  constructor(private readonly executable: string) {}
  async generate(recipe: Recipe): Promise<{ world: World; epoch: number }> {
    this.cancel();
    const session = new NativeSession(this.executable);
    this.candidate = session;
    try {
      const world = decodeWorld(await session.request({ command: 'generate', recipe: parseRecipe(recipe) }));
      if (this.candidate !== session) throw new Error('Native task canceled.');
      const epoch = ++this.sequence;
      this.prepared = { epoch, count: world.stats.regionCount };
      return { world, epoch };
    } catch (e) { session.close(); if (this.candidate === session) this.candidate = null; throw e; }
  }
  accept(epoch: number): void {
    if (!this.candidate || this.prepared?.epoch !== epoch) throw new Error('No matching prepared world.');
    this.active?.close(); this.active = this.candidate; this.candidate = null;
    this.epoch = epoch; this.count = this.prepared.count; this.tick = 0; this.prepared = null;
  }
  cancel(): void { this.candidate?.close(); this.candidate = null; this.prepared = null; }
  async advance(epoch: number, steps: number): Promise<DiagnosticFrame> {
    if (!this.active || this.candidate || epoch !== this.epoch) throw new Error('No matching active world.');
    if (!Number.isInteger(steps) || steps < 1 || steps > 100) throw new Error('Steps must be from 1 to 100.');
    const session = this.active;
    const { header, bytes } = await session.request({ command: 'advance', steps });
    if (this.active !== session || this.epoch !== epoch) throw new Error('Stale diagnostic frame.');
    if (header.kind !== 'frame' || bytes.length !== this.count * 8 || header.tick !== this.tick + steps
      || typeof header.relativeMassError !== 'number' || !Number.isFinite(header.relativeMassError)) {
      session.close(); throw new Error('Invalid diagnostic frame.');
    }
    this.tick += steps;
    return { epoch, tick: this.tick, relativeMassError: header.relativeMassError, field: floats(bytes) };
  }
  close(): void { this.cancel(); this.active?.close(); this.active = null; }
}
