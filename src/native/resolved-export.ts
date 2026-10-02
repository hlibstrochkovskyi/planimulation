import { createWriteStream } from 'node:fs';
import { rename, unlink } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import type { World } from '../core/world';

export const RESOLVED_WORLD_FORMAT = 'planimulation-resolved-initial-world';
export const RESOLVED_WORLD_FORMAT_VERSION = 2;
const MAX_REPORT_BYTES = 256 * 2 ** 20;

/** Explicit v2 projection: later model fields cannot silently change this format. */
export function resolvedWorldRecord(world: World): object {
  const { surface: s, tectonics: t, crust: c, terrain: h, water: w, drainage: d, basins: b } = world;
  return {
    format: RESOLVED_WORLD_FORMAT, formatVersion: RESOLVED_WORLD_FORMAT_VERSION,
    purpose: 'initial-generated-fields-not-a-checkpoint', recipe: world.recipe,
    initialFingerprint: world.checksum, stats: world.stats,
    surface: { radiusMeters: s.radiusMeters, centers: s.centers, faces: s.faces,
      neighborOffsets: s.neighborOffsets, neighbors: s.neighbors, neighborDistancesMeters: s.neighborDistancesMeters,
      areasSquareMeters: s.areasSquareMeters, boundaryOffsets: s.boundaryOffsets, boundaryDirections: s.boundaryDirections },
    diagnosticField: world.diagnosticField,
    tectonics: { owners: t.owners, seeds: t.seeds, angularVelocities: t.angularVelocities,
      boundaryCells: t.boundaryCells, boundaryDirections: t.boundaryDirections,
      boundaryMotion: t.boundaryMotion, boundaryTypes: t.boundaryTypes },
    crust: { threshold: c.threshold, potential: c.potential, continentality: c.continentality,
      thicknessMeters: c.thicknessMeters, densityKgPerCubicMeter: c.densityKgPerCubicMeter },
    terrain: { baseline: h.baseline, convergence: h.convergence, divergence: h.divergence,
      detail: h.detail, preparation: h.preparation, elevation: h.elevation,
      transportedCubicMeters: h.transportedCubicMeters, appliedPasses: h.appliedPasses },
    water: { levelMeters: w.levelMeters, resolvedVolumeCubicMeters: w.resolvedVolumeCubicMeters,
      depthMeters: w.depthMeters, bodyIds: w.bodyIds, mainOceanId: w.mainOceanId },
    drainage: { receivers: d.receivers, outlets: d.outlets, flatSteps: d.flatSteps,
      contributingArea: d.contributingArea },
    basins: { regionNodes: b.regionNodes, parents: b.parents, birthLevels: b.birthLevels,
      spillLevels: b.spillLevels, spillFrom: b.spillFrom, spillTo: b.spillTo,
      supportAreas: b.supportAreas, capacities: b.capacities },
  };
}

/** Stream JSON arrays in bounded chunks; no second whole-world serialization buffer. */
export function* resolvedWorldJson(world: World): Generator<string> {
  let bytes = 0;
  function* encode(value: unknown): Generator<string> {
    if (value instanceof Float64Array || value instanceof Uint32Array) {
      yield '[';
      for (let i = 0; i < value.length; i += 1024) {
        const chunk = Array.from(value.subarray(i, i + 1024));
        if (chunk.some((number) => !Number.isFinite(number))) throw new Error('Non-finite resolved-world field.');
        if (i) yield ',';
        yield JSON.stringify(chunk).slice(1, -1);
      }
      yield ']';
    } else if (Array.isArray(value)) {
      yield '[';
      for (let i = 0; i < value.length; i++) { if (i) yield ','; yield* encode(value[i]); }
      yield ']';
    } else if (value && typeof value === 'object') {
      yield '{';
      let first = true;
      for (const [key, item] of Object.entries(value)) {
        if (!first) yield ',';
        first = false;
        yield `${JSON.stringify(key)}:`;
        yield* encode(item);
      }
      yield '}';
    } else if (typeof value === 'string' || typeof value === 'boolean' || value === null
      || (typeof value === 'number' && Number.isFinite(value))) {
      yield JSON.stringify(value);
    } else throw new Error('Unsupported resolved-world field.');
  }
  for (const chunk of encode(resolvedWorldRecord(world))) {
    bytes += Buffer.byteLength(chunk, 'utf8');
    if (bytes > MAX_REPORT_BYTES) throw new Error('Resolved-world report exceeds the 256 MiB export limit.');
    yield chunk;
  }
  if (bytes + 1 > MAX_REPORT_BYTES) throw new Error('Resolved-world report exceeds the 256 MiB export limit.');
  yield '\n';
}

/** A failed export leaves the chosen destination untouched. */
export async function writeResolvedWorld(filePath: string, world: World): Promise<void> {
  const temporary = `${filePath}.tmp-${randomUUID()}`;
  try {
    await pipeline(Readable.from(resolvedWorldJson(world)), createWriteStream(temporary, { flags: 'wx' }));
    await rename(temporary, filePath);
  } catch (error) {
    await unlink(temporary).catch(() => {});
    throw error;
  }
}
