# Planimulation

An exploratory simulation of procedural Earth-like worlds and the histories that emerge within them.

A reproducible world connects geography, climate, water, ecology, and resources. Populations adapt to their environment; production, connections, and decisions shape settlements, states, and history. Civilizations then change the environment that supports them.

The application currently includes a desktop surface laboratory, static plate kinematics, independent initial crust, explainable elevation, initial water filling, static drainage catchments, and basin hierarchy inspection on a flat map and relief globe. Next come dynamic basin storage and overflow, then erosion and a living natural environment with seasons, flowing water, and vegetation. Human populations follow once that foundation has been checked.

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
| [Initial water](docs/water.md) | Coverage/volume fitting, connected water bodies, inventory accounting, analytical layers, and validation |
| [Water-surface display](docs/water-surface.md) | Separate globe water mesh, shoreline approximation, joint exaggeration bounds, and picking |
| [Drainage structure](docs/drainage.md) | Bed receivers, flat routing, terminal catchments, and contributing land-area accounting |
| [Basin analysis](docs/basins.md) | Plateau-batched hierarchy, threshold contacts, and level–storage relationships |
| [Basin inspection](docs/basin-inspection.md) | Versioned transport, branch/threshold layers, and parent/capacity inspection |
| [Reservoir experiment](docs/reservoir-experiment.md) | Standalone prescribed-input storage, external collector, budgets, and checkpoints |

## Status

Milestones A, B1–B5, C1, and C2a–C2b implement an Electron desktop application with an independent Rust core and a TypeScript interface:

- A closed spherical mesh with 12–40,962 computational regions, physical areas, neighbors, and distances.
- A GPU-rendered flat map **and a 3D globe of the same world**, with shared layers and region inspection. View changes never regenerate the model.
- Strict versioned recipes, independent random streams, native JSON import/export, and reproducible data fingerprints.
- Native background generation with cancellation and a headless adapter using the same executable.
- Seeded, connected tectonic plates; rigid angular velocities; actual shared-boundary segments classified from relative motion. Plate count and maximum speed are editable, and the inspector exposes opening/shear in cm/year.
- An explicitly diagnostic conservative diffusion test, with run/pause and field-only updates while navigating either view.
- Independent continentality, approximate crust thickness/density, editable continental area target and structure scale, and shared flat/globe crust layers. Actual crust coverage and patch sizes are reported separately from the target.

- Explainable static elevation: crust baseline, convergence uplift, divergence effects, and bounded detail. A displaced globe has shared-corner interpolation and display-only vertical exaggeration; the flat map remains planar.
- Initial water filling from either a coverage target or a fixed volume. Shared depth/body layers report actual coverage, the largest connected ocean, inland basins, and the resolved inventory. The Land and water layer adds a separate water-surface mesh on the globe; analytical layers expose the bed. Visible shorelines approximate whole-region water data and do not change physical coverage.

The seed field is a coherent **diagnostic signal**, separate from terrain, climate, or biomes. Mesh topology remains fixed at a given resolution. Water dynamics, erosion, climate, and civilization are not implemented. Elevation zero is a reference datum, not sea level. Recipes save generation inputs that reproduce the initial world, not a running diagnostic state.

Static drainage now assigns bed-based receivers, routes equal-height flats without altering elevation, preserves closed dry sinks, and accumulates contributing land area. Catchment and area layers are drainage potential, not flowing rivers; spill heights and lake storage are not computed yet.

Plates are not continents, and continental crust is not emerged land. Geology, water, drainage, and basin hierarchy are static initial conditions; diagnostic playback does not evolve them. [Basin inspection](docs/basin-inspection.md) adds branch/threshold layers, parent navigation, contact highlights, and capacity explanations in both views. Current recipes use `basins-1`; older recipes, including `drainage-1`, are explicitly rejected without automatic reinterpretation.

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

Generation and future algorithm proposals are recorded separately from implemented features. [C2a basin analysis](docs/basins.md) and [C2b inspection](docs/basin-inspection.md) provide a native hierarchy of depression connections with shared desktop inspection. [C3a](docs/reservoir-experiment.md) adds a standalone, checkpointable single-reservoir filling experiment with an explicit external collector; it does not advance desktop water. Next come coupled reservoirs and fill/merge rules; rivers are not yet computed. Resolved-world-state and image export remain outstanding milestone B work.

Development, documentation, code comments, and project records use English. Conversation with the project owner uses Russian.
