# Planimulation

An exploratory simulation of procedural Earth-like worlds and the histories that emerge within them.

A reproducible world connects geography, climate, water, ecology, and resources. Populations adapt to their environment; production, connections, and decisions shape settlements, states, and history. Civilizations then change the environment that supports them.

The application currently includes a desktop surface laboratory, static plate kinematics, independent initial crust, explainable elevation, initial water filling, static drainage catchments, basin hierarchy inspection, and a bounded manual runoff/spill view on a flat map and relief globe. Timed hydrology, erosion, and a living natural environment with seasons and vegetation follow later. Human populations follow once that foundation has been checked.

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
| [Coupled reservoir pair](docs/reservoir-pair.md) | Two-bowl conservative spill, threshold/merged states, and shared storage |
| [Spill connections](docs/spill-connections.md) | Geometric child–plateau graph, candidate receivers, and actual region passages |
| [Nested reservoir experiment](docs/nested-reservoir.md) | Bounded binary-hierarchy filling, actual receiver entry, exclusive stocks, and checkpoints |
| [Shared-sill allocation](docs/spill-junction.md) | Explicit capped receiver weights, simultaneous inputs, and order comparisons at one junction |
| [Spill network experiment](docs/spill-network.md) | Event-driven receiving frontiers, saturated transit, nested entry, and explicit pulse-order effects |
| [Concurrent network forcing](docs/simultaneous-network.md) | Constant simultaneous inputs, global saturation events, and input-order invariance |
| [Generated-world readiness](docs/spill-readiness.md) | Measured integration restrictions, conflicting entry witnesses, and next product gates |
| [Initial-water inventory](docs/initial-water-inventory.md) | Read-only transfer and audit of generated water into exclusive basin stocks |
| [Exact initial accounting](docs/exact-initial-accounting.md) | Opt-in local initial-water units, legacy reconciliation and strict replay audit; no dynamic water |
| [Seeded bounded network](docs/seeded-network.md) | Headless generated-water continuation with separate initial and external-input ledgers |
| [Guarded larger network](docs/expanded-seeded-network.md) | Versioned 10,242-region cap, compact branch index, measurements and retained failures |
| [Exact limiting-capacity commit](docs/exact-limit-commit.md) | Versioned correction of a measured floating-point saturation failure |
| [Shared storage index](docs/shared-storage-index.md) | Versioned bounded water runs without duplicated per-branch curves |
| [Repeated forcing probe](docs/repeated-forcing-probe.md) | Compact 100-interval replay, water-budget measurements, and retained numerical failures |
| [Multiple receiving entries](docs/multi-entry-network.md) | Explicit branch and internal-entry allocation for bounded concurrent networks |
| [Prescribed-water desktop view](docs/prescribed-water-desktop.md) | Manual runoff, bounded spill/merge, separate map/globe water levels, and limits |

## Status

Milestones A, B1–B5, C1, and C2a–C2b implement an Electron desktop application with an independent Rust core and a TypeScript interface:

- A closed spherical mesh with 12–40,962 computational regions, physical areas, neighbors, and distances.
- A GPU-rendered flat map **and a 3D globe of the same world**, with shared layers and region inspection. View changes never regenerate the model.
- Export of the currently visible 2D or globe view to PNG; image export does not save simulation state.
- Strict versioned recipes, independent random streams, native JSON import/export, and reproducible data fingerprints.
- Native background generation with cancellation and a headless adapter using the same executable.
- Seeded, connected tectonic plates; rigid angular velocities; actual shared-boundary segments classified from relative motion. Plate count and maximum speed are editable, and the inspector exposes opening/shear in cm/year.
- An explicitly diagnostic conservative diffusion test, with run/pause and field-only updates while navigating either view.
- Independent continentality, approximate crust thickness/density, editable continental area target and structure scale, and shared flat/globe crust layers. Actual crust coverage and patch sizes are reported separately from the target.

- Explainable static elevation: crust baseline, convergence uplift, divergence effects, and bounded detail. A displaced globe has shared-corner interpolation and display-only vertical exaggeration; the flat map remains planar.
- Initial water filling from either a coverage target or a fixed volume. Shared depth/body layers report actual coverage, the largest connected ocean, inland basins, and the resolved inventory. The Land and water layer adds a separate water-surface mesh on the globe; analytical layers expose the bed. Visible shorelines approximate whole-region water data and do not change physical coverage.
- Manual prescribed runoff can fill a selected basin, cross an unambiguous sill, and merge full neighbors. Exact native stocks drive derived depth/body layers and separate globe water levels. A separate water-checkpoint file can save, reopen, and continue this bounded inventory; an exact budget inspector shows the initial/input/storage ledger and a selected source's runoff destination. This has no elapsed time or climate forcing; ambiguous routes refuse atomically.
- `Export world data` writes a [versioned resolved initial-world report](docs/resolved-world-export.md) with native-generated fields for offline analysis. It is not a dynamic checkpoint; manual water steps remain separate.

The seed field is a coherent **diagnostic signal**, separate from terrain, climate, or biomes. Mesh topology remains fixed at a given resolution. Timed planetary water dynamics, erosion, climate, and civilization are not implemented. Elevation zero is a reference datum, not sea level. Recipes reproduce the initial world; the separate prescribed-water checkpoint saves only the bounded manual water inventory, not a full simulation state or diagnostic field.

Static drainage assigns bed-based receivers, routes equal-height flats without altering elevation, preserves closed dry sinks, and accumulates contributing land area. Catchment and area layers are drainage potential, not flowing rivers. Basin analysis computes geometric spill thresholds and storage capacities; it does not evolve the world's water inventories.

Plates are not continents, and continental crust is not emerged land. Geology and drainage remain static initial conditions; manual prescribed steps can change bounded basin stocks without changing the bed or initial drainage. Diagnostic playback does not evolve water. [Basin inspection](docs/basin-inspection.md) adds branch/threshold layers, parent navigation, contact highlights, and capacity explanations in both views. Current recipes use `basins-1`; older recipes, including `drainage-1`, are explicitly rejected without automatic reinterpretation.

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
npm run test:desktop     # Real Electron interaction, recipe, and water-checkpoint replay checks
npm run package          # Unpacked desktop application for the current platform
```

The desktop tests need a graphical session. Linux builds require the usual Electron/Chromium desktop libraries. Run a build before the headless and benchmark commands. Packaging bundles the native executable outside the application archive; the resulting app does not require Rust or Node.js to be installed. Packaging outputs to `release/`; it does not install globally, sign an application, or create a platform installer. Only Linux x64 is currently validated.

Generation and future algorithm proposals are recorded separately from implemented features. [C2a basin analysis](docs/basins.md) and [C2b inspection](docs/basin-inspection.md) provide a native hierarchy of depression connections with shared desktop inspection. [C3a](docs/reservoir-experiment.md)–[C3e](docs/spill-junction.md) provide standalone storage, connection, and allocation experiments. [C3f](docs/spill-network.md) provides a bounded ordered spill network; [C3g](docs/simultaneous-network.md) adds constant concurrent forcing. [C3h](docs/spill-readiness.md) measures generated-world restrictions; [C3i](docs/multi-entry-network.md) adds explicit allocation across multiple internal entries in a separate bounded experiment. [C3j](docs/initial-water-inventory.md) transfers generated water into read-only exclusive stocks; [C3k](docs/seeded-network.md) advances a 42-region generated world with a distinct initial-water ledger; [C3l](docs/expanded-seeded-network.md) conditionally advances worlds up to 10,242 regions under explicit storage bounds; [C3m](docs/exact-limit-commit.md) corrects a measured exact-threshold rounding failure; [C3n](docs/shared-storage-index.md) removes duplicated per-branch curves in a separately versioned bounded run; [C3o](docs/repeated-forcing-probe.md) measures repeated-forcing success and retained numerical failures. The separate [bounded prescribed-water inventory](docs/prescribed-water-inventory.md) powers manual desktop steps through one unambiguous spill/merge path and now has exact desktop checkpoint replay. Sub-precision inputs below its declared unit, unrestricted simultaneous allocation, long-running timed forcing, rivers, and full dynamic checkpoints remain outstanding.
