import type { Surface } from './surface';

export const BASIN_ANALYSIS_VERSION = 'basin-analysis-1';
export interface Basins {
  regionNodes: Uint32Array;
  parents: Uint32Array;
  birthLevels: Float64Array;
  spillLevels: Float64Array;
  spillFrom: Uint32Array;
  spillTo: Uint32Array;
  supportAreas: Float64Array;
  capacities: Float64Array;
}

/** Children are derived, not a second authoritative hierarchy. No recursive traversal. */
export function basinTree(b: Basins) {
  const children: number[][] = Array.from({ length: b.parents.length }, () => []);
  let root = -1;
  for (let i = 0; i < b.parents.length; i++) {
    if (b.parents[i] === i) root = i; else children[b.parents[i]].push(i);
  }
  const enter = new Uint32Array(children.length), leave = new Uint32Array(children.length);
  let clock = 0;
  const stack = [root];
  while (stack.length) {
    const node = stack.pop()!;
    if (node < 0) { leave[-node - 1] = clock; continue; }
    enter[node] = clock++; stack.push(-node - 1);
    for (let i = children[node].length - 1; i >= 0; i--) stack.push(children[node][i]);
  }
  return { root, children, enter, leave };
}

/** Structural/volume audit of native analysis, not a duplicate elevation sweep. */
export function validateBasins(s: Surface, heights: Float64Array, b: Basins): void {
  const n = heights.length, k = b.parents.length;
  const fail = (): never => { throw new Error('Invalid native basin analysis.'); };
  if (k < 1 || k > 2 * n - 1 || b.regionNodes.length !== n
    || Object.entries(b).some(([key, a]) => key !== 'regionNodes' && a.length !== k)) fail();
  let roots = 0;
  for (let i = 0; i < k; i++) {
    const p = b.parents[i];
    if (p >= k || p < i || !Number.isFinite(b.birthLevels[i]) || !Number.isFinite(b.spillLevels[i])
      || !Number.isFinite(b.supportAreas[i]) || b.supportAreas[i] <= 0
      || !Number.isFinite(b.capacities[i]) || b.capacities[i] < 0) fail();
    if (p === i) {
      roots++;
      if (b.spillLevels[i] !== b.birthLevels[i] || b.capacities[i] !== 0 || b.spillFrom[i] !== n || b.spillTo[i] !== n) fail();
    } else if (b.spillLevels[i] !== b.birthLevels[p] || b.spillLevels[i] <= b.birthLevels[i]
      || b.spillFrom[i] >= n || b.spillTo[i] >= n) fail();
  }
  if (roots !== 1 || b.regionNodes.some((id) => id >= k)) fail();
  const tree = basinTree(b), areas = new Float64Array(k), volumes = new Float64Array(k);
  const minima = new Float64Array(k).fill(Infinity);
  for (let i = 0; i < n; i++) {
    const node = b.regionNodes[i], h = heights[i];
    if (h < b.birthLevels[node] || (node !== tree.root && h >= b.spillLevels[node])) fail();
    areas[node] += s.areasSquareMeters[i]; minima[node] = Math.min(minima[node], h);
    if (node !== tree.root) volumes[node] += s.areasSquareMeters[i] * (b.spillLevels[node] - h);
  }
  const near = (a: number, expected: number): boolean => Number.isFinite(a) && Number.isFinite(expected)
    && Math.abs(a - expected) <= Math.max(1e-6, Math.abs(expected) * 1e-10);
  for (let i = 0; i < k; i++) {
    const p = b.parents[i];
    if (minima[i] !== b.birthLevels[i] || tree.children[i].length === 1 || !near(areas[i], b.supportAreas[i])) fail();
    if (i === tree.root) continue;
    if (!near(volumes[i], b.capacities[i])) fail();
    const from = b.spillFrom[i], to = b.spillTo[i], owner = b.regionNodes[to];
    if (heights[from] !== b.spillLevels[i] || heights[to] >= heights[from] || b.regionNodes[from] !== p
      || tree.enter[owner] < tree.enter[i] || tree.enter[owner] >= tree.leave[i]
      || !s.neighbors.subarray(s.neighborOffsets[from], s.neighborOffsets[from + 1]).includes(to)) fail();
    areas[p] += areas[i];
    if (p !== tree.root) volumes[p] += volumes[i] + areas[i] * (b.spillLevels[p] - b.spillLevels[i]);
  }
}

export function summarizeBasins(b: Basins) {
  const { root, children } = basinTree(b);
  let minimumSpill = Infinity, maximumSpill = -Infinity;
  for (let i = 0; i < b.parents.length; i++) if (i !== root) {
    minimumSpill = Math.min(minimumSpill, b.spillLevels[i]); maximumSpill = Math.max(maximumSpill, b.spillLevels[i]);
  }
  return { analysisVersion: BASIN_ANALYSIS_VERSION, nodeCount: b.parents.length,
    leafCount: children.filter((c) => c.length === 0).length, root,
    minimumSpillMeters: minimumSpill === Infinity ? null : minimumSpill,
    maximumSpillMeters: maximumSpill === -Infinity ? null : maximumSpill };
}
