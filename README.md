# Planimulation

An exploratory simulation of procedural Earth-like worlds and the histories that emerge within them.

A reproducible world connects geography, climate, water, ecology, and resources. Populations adapt to their environment; production, connections, and decisions shape settlements, states, and history. Civilizations then change the environment that supports them.

The application currently includes a desktop surface laboratory, static plate kinematics, independent initial crust, and explainable elevation on a flat map and relief globe. Next come initial water fitting and erosion, followed by a living natural environment with seasons, water, and vegetation. Human populations follow once that foundation has been checked.

## Documentation

| Document | Contents |
| --- | --- |
| [DESIGN.md](DESIGN.md) | Vision, world laws, future civilization mechanics, and scope |
| [World generation](docs/world-generation.md) | Proposed algorithms, spherical surface, controls, initialization, dynamics, and 2D/3D views |
| [Implementation plan](docs/implementation-plan.md) | Early milestones, minimal architecture, validation, and acceptance criteria |
| [Development guide](docs/development.md) | Desktop commands, testing strategy, reproducibility, and current limits |
| [Native/GPU foundation](docs/native-foundation.md) | Rust process ownership, binary protocol, shared GPU views, and diagnostic transport |
| [Foundation validation](docs/validation-native-foundation.md) | Native and desktop measurements, checks, memory costs, and limitations |
| [Plate kinematics](docs/tectonics.md) | Connected plates, velocity conventions, boundary classification, parameters, and validation |
| [Initial crust](docs/crust.md) | Spherical continentality, area fitting, approximate material properties, and validation |
| [Explainable elevation](docs/terrain.md) | Elevation contributions, physical-distance propagation, globe relief, and display-only exaggeration |

## Status

Milestones A and B1–B3 implement an Electron desktop application with an independent Rust core and a TypeScript interface:

- A closed spherical mesh with 12–40,962 computational regions, physical areas, neighbors, and distances.
- A GPU-rendered flat map **and a 3D globe of the same world**, with shared layers and region inspection. View changes never regenerate the model.
- Strict versioned recipes, independent random streams, native JSON import/export, and reproducible data fingerprints.
- Native background generation with cancellation and a headless adapter using the same executable.
- Seeded, connected tectonic plates; rigid angular velocities; actual shared-boundary segments classified from relative motion. Plate count and maximum speed are editable, and the inspector exposes opening/shear in cm/year.
- An explicitly diagnostic conservative diffusion test, with run/pause and field-only updates while navigating either view.
- Independent continentality, approximate crust thickness/density, editable continental area target and structure scale, and shared flat/globe crust layers. Actual crust coverage and patch sizes are reported separately from the target.

- Explainable static elevation: crust baseline, convergence uplift, divergence effects, and bounded detail. A displaced globe has shared-corner interpolation and display-only vertical exaggeration; the flat map remains planar.

The seed field is a coherent **diagnostic signal**, separate from terrain, climate, or biomes. Mesh topology remains fixed at a given resolution. Water, erosion, climate, and civilization are not implemented. Elevation zero is a reference datum, not sea level. Recipes save the initial world, not a running diagnostic state.

Plates are not continents, and continental crust is not emerged land. Crust, velocities, and elevation are static initial conditions; diagnostic playback does not evolve them. See [Explainable elevation](docs/terrain.md) and its linked foundation documents for assumptions and the process boundary. Current recipes use `terrain-1`; older recipes, including `crust-1`, are explicitly rejected without automatic reinterpretation.

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

Generation and future algorithm proposals are recorded separately from implemented features. The next part of milestone B is an explicitly accounted initial water inventory and connected water components.

Development, documentation, code comments, and project records use English. Conversation with the project owner uses Russian.
