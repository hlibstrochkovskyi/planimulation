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
| [Native/GPU foundation](docs/native-foundation.md) | Rust process ownership, binary protocol, shared GPU views, and diagnostic transport |
| [Foundation validation](docs/validation-native-foundation.md) | Native and desktop measurements, checks, memory costs, and limitations |

## Status

Milestone A and its native/GPU foundation increment implement an Electron desktop application with an independent Rust core and a TypeScript interface:

- A closed spherical mesh with 12–40,962 computational regions, physical areas, neighbors, and distances.
- A GPU-rendered flat map **and a 3D globe of the same world**, with shared layers and region inspection. View changes never regenerate the model.
- Strict versioned recipes, independent random streams, native JSON import/export, and reproducible data fingerprints.
- Native background generation with cancellation and a headless adapter using the same executable.
- An explicitly diagnostic conservative diffusion test, with run/pause and field-only updates while navigating either view.

The seed field is a coherent **diagnostic signal**, not terrain, climate, or a biome. Changing the seed changes this signal; the geometric mesh remains fixed at a given resolution. The globe has no elevation model yet. Terrain, oceans, climate, and civilization are not implemented. Recipes save the initial world, not a running diagnostic state.

See [Native/GPU foundation](docs/native-foundation.md) for the process boundary, version transition, performance measurements, and remaining limits. Legacy `surface-1` recipes are explicitly rejected by `surface-rust-1`; no automatic reinterpretation is performed.

## Run the desktop application

Use Node.js 22.12 or newer, npm, a Rust toolchain (validated with Rust 1.98.1), and platform build tools. The viewer requires WebGL 2. Install dependencies once:

```sh
npm ci
npm start
```

`npm start` type-checks, builds, and opens a native Electron window. It does not require a browser or a running web server. After a build, `npm run desktop` opens the existing build directly.

```sh
npm test                 # Rust, bridge, geometry, reproducibility, and projection tests
npm run typecheck        # Strict TypeScript checks
npm run headless         # Default recipe: fingerprint and surface statistics
npm run headless -- path/to/recipe.json
npm run benchmark        # Mesh levels 3–6 on the current device
npm run benchmark:desktop # Native dynamics + camera movement in both GPU views
npm run test:desktop     # Real Electron interaction and recipe round-trip checks
npm run package          # Unpacked desktop application for the current platform
```

The desktop tests need a graphical session. Linux builds require the usual Electron/Chromium desktop libraries. Run a build before the headless and benchmark commands. Packaging bundles the native executable outside the application archive; the resulting app does not require Rust or Node.js to be installed. Packaging outputs to `release/`; it does not install globally, sign an application, or create a platform installer. Only Linux x64 is currently validated.

Generation and future algorithm proposals are recorded separately from implemented features. The next goal is milestone B: geological structure, elevation contributions, and an explicitly accounted initial water inventory.

Development, documentation, code comments, and project records use English. Conversation with the project owner uses Russian.
