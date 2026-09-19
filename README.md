# Planimulation

An exploratory simulation of procedural Earth-like worlds and the histories that emerge within them.

A reproducible world connects geography, climate, water, ecology, and resources. Populations adapt to their environment; production, connections, and decisions shape settlements, states, and history. Civilizations then change the environment that supports them.

The first implemented milestone is a desktop surface laboratory. The next milestone adds explainable terrain and oceans, followed by a living natural environment with seasons, water, and vegetation. Human populations follow once that foundation has been checked.

## Documentation

| Document | Contents |
| --- | --- |
| [DESIGN.md](DESIGN.md) | Vision, world laws, future civilization mechanics, and scope |
| [World generation](docs/world-generation.md) | Proposed algorithms, spherical surface, controls, initialization, dynamics, and 2D/3D views |
| [Implementation plan](docs/implementation-plan.md) | Early milestones, minimal architecture, validation, and acceptance criteria |
| [Development guide](docs/development.md) | Desktop commands, testing strategy, reproducibility, and current limits |

## Status

Milestone A implements an Electron desktop application and a renderer-independent TypeScript core:

- A closed spherical mesh with 12–40,962 computational regions, physical areas, neighbors, and distances.
- A flat map with seam/pole handling, pan/zoom, layer controls, and region inspection.
- Strict versioned recipes, independent random streams, native JSON import/export, and reproducible data fingerprints.
- Background generation with cancellation and a headless adapter for validation and benchmarks.

The seed field is a coherent **diagnostic signal**, not terrain, climate, or a biome. Changing the seed changes this signal; the geometric mesh remains fixed at a given resolution. Terrain, oceans, climate, 3D rendering, and civilization are not implemented yet.

## Run the desktop application

Use Node.js 22.12 or newer and npm. Install dependencies once:

```sh
npm ci
npm start
```

`npm start` type-checks, builds, and opens a native Electron window. It does not require a browser or a running web server. After a build, `npm run desktop` opens the existing build directly.

```sh
npm test                 # Core invariants, reproducibility, and map projection
npm run typecheck        # Strict TypeScript checks
npm run headless         # Default recipe: fingerprint and surface statistics
npm run headless -- path/to/recipe.json
npm run benchmark        # Mesh levels 3–6 on the current device
npm run test:desktop     # Real Electron interaction and recipe round-trip checks
npm run package          # Unpacked desktop application for the current platform
```

The desktop tests need a graphical session. Linux builds require the usual Electron/Chromium desktop libraries. Packaging outputs to `release/`; it does not install globally, sign an application, or create a platform installer.

Generation and future algorithm proposals are recorded separately from implemented features. The next goal is milestone B: geological structure, elevation contributions, and an explicitly accounted initial water inventory.

Development, documentation, code comments, and project records use English. Conversation with the project owner uses Russian.
