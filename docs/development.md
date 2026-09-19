# Development and validation

## Current increment

Milestone A is a desktop surface laboratory, implemented on September 19, 2026. It establishes geometry, reproducibility, inspection, and recipe persistence before adding geological generation.

Electron provides the native window and dialogs. The renderer runs isolated from Node.js with sandboxing enabled. A bundled CommonJS preload exposes only `openRecipe` and `saveRecipe`; the main process checks the sender and validates recipe content. Local assets and workers are bundled with Vite. The application needs no web server or network access after dependencies and the Electron runtime are available.

Canvas 2D displays the initial atlas. A 3D library is intentionally not a dependency yet. The core's unit vectors, triangle indices, and physical fields remain suitable for a later globe renderer.

## Daily workflow

```sh
npm ci
npm test
npm run build
npm run desktop
```

`npm start` combines build and desktop launch. Use `npm run test:desktop` for the interaction suite. It opens and closes an actual Electron window, uses a temporary profile, and writes `artifacts/surface-desktop.png`. Native dialogs are stubbed only in this test; the real preload, IPC validation, and file handlers remain active.

`npm run package` creates an unpacked application in `release/` for the current platform. On Linux x64 its executable is `release/Planimulation-linux-x64/planimulation`. This is a portable development build, not an installed or signed release. Other operating systems have not been validated by running this Linux environment.

After packaging, run the same interaction suite against the executable with `npm run test:desktop -- release/Planimulation-linux-x64/planimulation`. The packaged run writes `artifacts/surface-desktop-packaged.png`.

Dependencies are exact versions with a lockfile. Review dependency changes separately from model changes. Commit coherent increments with English messages using the owner's configured identity. Never add assistant authorship or co-author trailers.

## Testing procedural behavior

Random-looking output is not exempt from tests. Separate the properties we can assert from qualities requiring inspection:

| Concern | Check |
| --- | --- |
| Topology | Connected graph, reciprocal adjacency, two faces per edge, Euler characteristic 2 |
| Geometry | Unit directions, unique centers, positive areas, total area `4πR²`, independently known spherical triangle area |
| Physical scaling | Doubling radius doubles distances and quadruples areas without changing topology |
| Reproducibility | Identical resolved recipe yields identical arrays and fingerprint in the supported environment |
| Randomness | Fixed PRNG reference vectors, independent named streams, resumable state, seed-ensemble diversity |
| Projection | Projected coverage includes poles and seam; wrapped longitudes select the same region |
| Picking | Centers and sampled directions select their containing barycentric region |
| Desktop | Exact repeatability between desktop workers, cross-runtime numerical agreement, generation, selection, layers, recipe round trip, invalid imports, replacement and cancellation |
| Appearance | Inspect screenshot and interactive map for seams, legibility, distortion, and usability |

Area and projection checks use relative/absolute tolerances rather than exact floating-point equality. Exact equality is appropriate for repeated execution of the same algorithm in the supported environment.

The deterministic 20-seed ensemble checks reproduction, finite bounded signal values, signal differences, and invariant mesh geometry. It does not demonstrate realistic terrain, which does not exist yet.

Barycentric region boundaries can be slightly concave. Picking therefore tests the constituent spherical triangle fan. Boundary ties use a stable region-ID order. The projection densifies geodesic edges, handles polar caps, and wraps seam fragments without creating model regions.

## Recipes and fingerprints

```json
{
  "schemaVersion": 1,
  "modelVersion": "surface-1",
  "randomVersion": "fnv1a-utf8-mulberry32-1",
  "seed": "first-light",
  "subdivision": 5,
  "radiusMeters": 6371000
}
```

Import is strict: all fields are required, unknown fields are rejected, and unsupported versions are not silently interpreted. Seed text is preserved exactly. Radius is stored in meters; the UI displays kilometers. Mesh levels 0–6 are supported. Native recipe import is bounded to 32 KiB.

Named random streams derive a 32-bit state from FNV-1a over the UTF-8 JSON encoding of `[seed, streamName]`, then use Mulberry32. Reference outputs are pinned in tests. Future physical systems get their own stream names.

The fingerprint covers the canonical recipe, geometry, areas, adjacency, distances, boundaries, and diagnostic signal using explicit little-endian numeric encoding. It is a compact regression checksum, not a cryptographic identifier; collisions are possible. Reproducibility tests additionally compare full arrays.

Floating-point transcendental functions can differ in their last bits across V8/runtime versions. The initial desktop test observed fingerprints `ca3e83ee` in Electron 44.4.3 and `2b83f7e1` in Node.js 25.2.1 for the default recipe. Comparing every array element found a maximum error of approximately `4.93e-16` after scaling by `max(1, abs(reference))`; integer topology matched exactly. The cross-runtime test enforces a `1e-12` scaled tolerance. Two independent desktop workers and a desktop save/load round trip must match exactly. A different fingerprint across these runtimes is therefore not itself evidence of a different geographic structure. No cross-runtime bitwise or future trajectory-equivalence guarantee is made.

The diagnostic signal samples eight seeded sine modes in 3D at spherical centers. It has no physical units or geographic interpretation and uses its own stream. The seed has no role in subdividing the icosahedron. This deliberate separation prevents a change in weather or appearance from unexpectedly changing the mesh.

A recipe regenerates a surface; it is not a future simulation checkpoint. Do not reuse this format for evolving state without designing a separate versioned checkpoint schema.

## Performance measurements

Run `npm run benchmark` to measure generation at levels 3–6. Measurements include geometry, the diagnostic field, and the checksum, but exclude rendering, IPC copying, and application startup. Array memory excludes temporary objects, projection paths, and Electron overhead.

An initial Node.js 25.2.1 run on the development machine measured approximately:

| Level | Regions | Generation | Model arrays |
| --- | ---: | ---: | ---: |
| 3 | 642 | 38 ms | 0.26 MiB |
| 4 | 2,562 | 105 ms | 1.05 MiB |
| 5 | 10,242 | 330 ms | 4.22 MiB |
| 6 | 40,962 | 1,070 ms | 16.88 MiB |

These are one-run observations, not cross-device guarantees. Level 5 is the provisional default. Recheck the budget after terrain and hydrology are added. More pixels or a finer mesh alone do not guarantee more realistic generation.

## Next increment: geological structure

Before milestone B, keep the generator's causes separable and testable:

1. Define continentality as a continuous spherical field and plate ownership as a connected partition.
2. Assign plate angular velocities and classify relative movement across boundaries.
3. Build elevation from inspectable contributions: crust baseline, boundary effects, and bounded local detail.
4. Fit the initial water level from either inventory or requested coverage, retaining inferred volume and component statistics.
5. Expose controls only when their mechanisms work; record every resolved value in a new model recipe version.

Use constructed boundaries to check convergence/divergence signs, controlled parameter changes to examine relief effects, and seed ensembles to detect fragmented plates or numerical failures. Add an area-weighted water-volume check before showing oceans as physical quantities. Preserve the original heights when later analyzing drainage.

Geological quality additionally needs visual review across ordinary and extreme supported recipes. Record problematic seeds and turn explainable failures into regressions. Avoid tuning coefficients to make only one attractive planet.

## Reference boundaries

Electron's [context isolation](https://www.electronjs.org/docs/latest/tutorial/context-isolation) and [sandboxed preload guidance](https://www.electronjs.org/docs/latest/tutorial/tutorial-preload) inform the desktop boundary. The desktop smoke suite uses Playwright's [Electron automation](https://playwright.dev/docs/api/class-electron). These references concern application mechanics, not scientific validation of the simulation.
