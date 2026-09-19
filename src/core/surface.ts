import { angularDistance, cross, dot, midpoint, normalize, readVector, triangleArea } from './vector';
import type { Vec3 } from './vector';

export interface Surface {
  radiusMeters: number;
  centers: Float64Array;
  faces: Uint32Array;
  neighborOffsets: Uint32Array;
  neighbors: Uint32Array;
  neighborDistancesMeters: Float64Array;
  areasSquareMeters: Float64Array;
  boundaryOffsets: Uint32Array;
  boundaryDirections: Float64Array;
}

export function buildSurface(subdivision: number, radiusMeters: number): Surface {
  if (!Number.isInteger(subdivision) || subdivision < 0 || subdivision > 6) {
    throw new Error('Unsupported subdivision.');
  }
  if (!Number.isFinite(radiusMeters) || radiusMeters <= 0) throw new Error('Radius must be positive.');
  const phi = (1 + Math.sqrt(5)) / 2;
  const vertices: Vec3[] = [
    [-1, phi, 0], [1, phi, 0], [-1, -phi, 0], [1, -phi, 0],
    [0, -1, phi], [0, 1, phi], [0, -1, -phi], [0, 1, -phi],
    [phi, 0, -1], [phi, 0, 1], [-phi, 0, -1], [-phi, 0, 1],
  ].map((v) => normalize(v as unknown as Vec3));
  let faces = [
    0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11,
    1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7, 1, 8,
    3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9,
    4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9, 8, 1,
  ];
  for (let level = 0; level < subdivision; level++) {
    const cache = new Map<string, number>();
    const bisect = (a: number, b: number): number => {
      const key = a < b ? `${a}:${b}` : `${b}:${a}`;
      const existing = cache.get(key);
      if (existing !== undefined) return existing;
      const id = vertices.length;
      vertices.push(midpoint(vertices[a], vertices[b]));
      cache.set(key, id);
      return id;
    };
    const next: number[] = [];
    for (let i = 0; i < faces.length; i += 3) {
      const [a, b, c] = [faces[i], faces[i + 1], faces[i + 2]];
      const ab = bisect(a, b), bc = bisect(b, c), ca = bisect(c, a);
      next.push(a, ab, ca, b, bc, ab, c, ca, bc, ab, bc, ca);
    }
    faces = next;
  }

  const adjacency = vertices.map(() => new Set<number>());
  const incidentCenters: Vec3[][] = vertices.map(() => []);
  for (let i = 0; i < faces.length; i += 3) {
    const ids = [faces[i], faces[i + 1], faces[i + 2]];
    const [a, b, c] = ids.map((id) => vertices[id]);
    const centroid = normalize([a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]]);
    for (let j = 0; j < 3; j++) {
      adjacency[ids[j]].add(ids[(j + 1) % 3]);
      adjacency[ids[j]].add(ids[(j + 2) % 3]);
      incidentCenters[ids[j]].push(centroid);
    }
  }

  const neighborOffsets = new Uint32Array(vertices.length + 1);
  const boundaryOffsets = new Uint32Array(vertices.length + 1);
  const areasSquareMeters = new Float64Array(vertices.length);
  const neighbors: number[] = [], distances: number[] = [], boundaries: number[] = [];
  for (let id = 0; id < vertices.length; id++) {
    const center = vertices[id];
    neighborOffsets[id] = neighbors.length;
    boundaryOffsets[id] = boundaries.length / 3;
    const orderedNeighbors = [...adjacency[id]].sort((a, b) => a - b);
    for (const neighbor of orderedNeighbors) {
      neighbors.push(neighbor);
      distances.push(angularDistance(center, vertices[neighbor]) * radiusMeters);
    }
    const ring = [...incidentCenters[id], ...orderedNeighbors.map((n) => midpoint(center, vertices[n]))];
    const tangent = normalize(cross(Math.abs(center[2]) < 0.9 ? [0, 0, 1] : [0, 1, 0], center));
    const bitangent = cross(center, tangent);
    ring.sort((a, b) => Math.atan2(dot(a, bitangent), dot(a, tangent)) - Math.atan2(dot(b, bitangent), dot(b, tangent)));
    let area = 0;
    for (let j = 0; j < ring.length; j++) {
      boundaries.push(...ring[j]);
      area += triangleArea(center, ring[j], ring[(j + 1) % ring.length]);
    }
    areasSquareMeters[id] = area * radiusMeters * radiusMeters;
  }
  neighborOffsets[vertices.length] = neighbors.length;
  boundaryOffsets[vertices.length] = boundaries.length / 3;
  return {
    radiusMeters,
    centers: Float64Array.from(vertices.flat()),
    faces: Uint32Array.from(faces),
    neighborOffsets,
    neighbors: Uint32Array.from(neighbors),
    neighborDistancesMeters: Float64Array.from(distances),
    areasSquareMeters,
    boundaryOffsets,
    boundaryDirections: Float64Array.from(boundaries),
  };
}

/** Locate the actual barycentric region, rather than assuming a Voronoi partition. */
export function locateRegion(surface: Surface, point: Vec3): number {
  let nearest = 0, best = -Infinity;
  for (let id = 0; id < surface.areasSquareMeters.length; id++) {
    const score = dot(readVector(surface.centers, id), point);
    if (score > best) { best = score; nearest = id; }
  }
  const candidates = [nearest, ...surface.neighbors.subarray(surface.neighborOffsets[nearest], surface.neighborOffsets[nearest + 1])].sort((a, b) => a - b);
  for (const id of candidates) {
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
    const center = readVector(surface.centers, id);
    for (let i = start; i < end; i++) {
      const a = readVector(surface.boundaryDirections, i);
      const b = readVector(surface.boundaryDirections, i + 1 === end ? start : i + 1);
      // Barycentric regions can be slightly concave. Test their triangle fan,
      // not the intersection of all boundary half-spaces.
      if (dot(cross(center, a), point) >= -1e-12
        && dot(cross(a, b), point) >= -1e-12
        && dot(cross(b, center), point) >= -1e-12) return id;
    }
  }
  throw new Error('Point could not be located on the spherical surface.');
}
