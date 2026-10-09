# Planimulation

An exploratory simulation of procedural Earth-like worlds and the histories that emerge within them.

A reproducible world connects geography, climate, water, ecology, and resources. Populations adapt to their environment; production, connections, and decisions shape settlements, states, and history. Civilizations then change the environment that supports them.

The application currently includes a desktop surface laboratory, static plate kinematics, independent initial crust, explainable elevation with bounded dry terrain preparation, initial water filling, static drainage catchments, basin hierarchy inspection, a bounded manual runoff/spill view, and seasonal temperature/wind normals on a flat map and relief globe. A separate finite seasonal-water mode now has run/pause, stock/rate layers, and budgets. General planetary hydrology, climate-driven erosion, weather, and a living natural environment with vegetation follow later. Human populations follow once that foundation has been checked.

A native [moisture-transport kernel and headless report](docs/moisture-transport.md) advance prescribed atmospheric column stocks with the seasonal wind. A separate [finite seasonal-moisture model](docs/seasonal-moisture.md) couples transport and atmospheric exchange to [snow, liquid, and soil](docs/surface-water.md), plus [delayed neighboring runoff and evaporating terminal stores](docs/runoff-transport.md), with regional transfer ledgers and complete bounded-model checkpoint replay. Its [desktop viewer](docs/seasonal-water-desktop.md) uses the same Rust calculation and supports [complete seasonal checkpoint save/load](docs/seasonal-water-checkpoints.md). The bed stays fixed; legacy desktop seasonal modes do not derive lake levels or apply spill. The opt-in headless [bounded leaf-spill mode](docs/bounded-leaf-spill.md) fills actual receiving lakes and passes excess through full lower-sill leaves. A separately versioned [common-sill parent mode](docs/common-sill-lakes.md) merges full one-level dry siblings; its [reversible continuation](docs/common-sill-frontier.md) also applies drying/splitting and independent child contraction. A further [bounded receiver mode](docs/common-sill-receiving.md) accepts unique external leaf spill into an already active parent below supplying-head/next-sill ceilings. Its [bounded outlet continuation](docs/parent-outlet.md) sends full-parent excess through one real outlet when the receiver accepts the complete arrival. Higher/nested ownership, downstream parent overflow chains, concurrent spill policy and groundwater remain open.

A separate [regional surface-flow candidate](docs/regional-surface-flow.md) uses compensated regional columns and neighboring physical faces instead of enumerating basin-parent transitions. It couples capped Manning-inspired flow to seasonal exchange and passes the recorded 25-case annual cohort, including explicitly funded active-flow/refinement controls. Its opt-in [desktop mode](docs/regional-water-desktop.md) adds regional depth, cumulative flow layers, separate body budgets, changing regional water caps and complete checkpoint replay in both projections. This is a different, uncalibrated approximation, not a migration; coarse wet/dry/soil coupling, fixed reference heads, capped mobility and spatial sensitivity remain review gates.

The separate [unified regional soil cycle](docs/regional-soil-cycle.md) applies source-independent liquid/soil exchange without the legacy whole-column wet-film switch. Vapor, delayed soil drainage and every directed transfer retain paired precision and complete replay. Strict numerical refusal and explicitly selected donor retention are separately pinned. Its [protocol-15 transport and typed session](docs/soil-water-transport.md) now support an opt-in [desktop mode](docs/soil-water-desktop.md): **Initialize unified soil water** on a fresh world. Ownership-aware layers, paired inspectors, donor-retention diagnostics and complete checkpoint replay work in both projections. Defaults and old saves are unchanged; physical calibration and broader spatial/long-term qualification remain open.

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
| [Continuous plate-root candidate](docs/plate-root-candidate.md) | Opt-in versioned generator path and paired seeded resolution-stability measurements |
| [Initial crust](docs/crust.md) | Spherical continentality, area fitting, approximate material properties, and validation |
| [Explainable elevation](docs/terrain.md) | Elevation contributions, physical-distance propagation, globe relief, and display-only exaggeration |
| [Dry terrain preparation](docs/terrain-preparation.md) | Versioned bounded material transfer before initial water filling, limits, and validation |
| [Terrain-preparation study](docs/terrain-preparation-study.md) | Reproducible 12-seed, five-resolution sensitivity measurements and model limits |
| [Seasonal temperature normals](docs/seasonal-temperature.md) | Derived monthly temperatures, assumptions, headless report, validation, and limits |
| [Seasonal surface-wind normals](docs/seasonal-wind.md) | Prescribed monthly wind vectors and map layer, assumptions, headless report, and limits |
| [Moisture transport](docs/moisture-transport.md) | Conservative native column-water transport, seasonal headless runs, numerical checks, and coupling limits |
| [Finite seasonal moisture](docs/seasonal-moisture.md) | Six owned water stocks, runoff return, bounded checkpoints, matched controls, and integration tests |
| [Experimental upslope response](docs/orographic-response.md) | Opt-in terrain/wind supersaturation response, directed controls, schema-4 replay, and retained refinement failures |
| [Checkpointed soil precision](docs/soil-precision.md) | Headless compensated-soil candidate, independent rounding audits, preserved legacy results, and failed annual snow qualification |
| [Persistent surface precision](docs/surface-precision.md) | Versioned liquid/snow/soil/terminal components, retained failures, annual/ten-year checks, and opt-in desktop replay |
| [Seasonal sensitivity](docs/seasonal-sensitivity.md) | Accepted monthly histories, matched parameter controls, regional differences, and separate recorded years |
| [Seasonal preparation diagnostics](docs/seasonal-preparation.md) | Read-only annual stock/flow stationarity checks, cold-storage controls, and retained ten-year drift |
| [Water-return analysis](docs/water-return-analysis.md) | Connected-body/closed-sink ownership and retained frozen liquid-demand controls |
| [Finite reference-water pool](docs/reference-water-pool.md) | Opt-in headless model 8: finite body-owned liquid, conservative evaporation allocation, and matched legacy controls |
| [Body-aware seasonal diagnostics](docs/body-seasonal-preparation.md) | Annual body/regional stock and flow checks, recorded candidates, and retained closed-dry storage drift |
| [Closed-leaf lakes](docs/closed-leaf-lakes.md) | Derived minimum-basin exposure, finite evaporation operator, and static first-spill recipients |
| [Coupled leaf-lake exchange](docs/coupled-leaf-lakes.md) | Opt-in headless model 9: applied exposure/interception and finite evaporation below the first connection; no spill/merge |
| [Applied bounded leaf spill](docs/bounded-leaf-spill.md) | Opt-in headless model 10: fill the actual receiving lake before lower-sill passage to a wet contact; explicit queues, flow ledgers and rollback; no parent merge |
| [Bounded common-sill parents](docs/common-sill-lakes.md) | Opt-in headless model 11: exclusive one-level parent contents, above-sill exposure/exchange, strict replay and atomic split/next-spill refusal |
| [Reversible common-sill lakes](docs/common-sill-frontier.md) | Opt-in headless model 12: one-level drying/splitting, lifetime provenance, strict replay and read-only current-owner inspection |
| [Bounded parent spill receivers](docs/common-sill-receiving.md) | Opt-in headless model 13: unique external leaf arrivals into active parents, head/storage ceilings, lifetime incoming provenance and atomic refusals |
| [Bounded parent outlets](docs/parent-outlet.md) | Opt-in headless model 14: full-parent excess through a unique geographic outlet, independent gross-flow identities, replay and atomic refusals |
| [Regional surface flow](docs/regional-surface-flow.md) | Opt-in headless model 15: neighboring regional columns, explicit capped mobility, independent face ledgers, annual/refinement evidence and retained prototype failures |
| [Regional-water desktop](docs/regional-water-desktop.md) | Opt-in model-15 map/globe layers, separate regional/body ownership, protocol-14 frames and exact complete checkpoint continuation |
| [Ponded liquid and soil](docs/ponded-soil-exchange.md) | Retained model-15 thin-film discontinuity, conservative vertical exchange and explicitly pinned numerical retention |
| [Unified regional soil cycle](docs/regional-soil-cycle.md) | Separate headless seasonal family, unified terrestrial liquid, paired vapor/drainage, contact accounting/replay and read-only display preparation |
| [Unified soil-water transport](docs/soil-water-transport.md) | Protocol-15 paired owners, strict decoding, separate typed session and complete checkpoint continuation |
| [Unified soil-water desktop](docs/soil-water-desktop.md) | Opt-in five-owner mode, paired inspection, atlas/globe layers, numerical diagnostics and exact checkpoint continuation |
| [Dense soil-water controls](docs/soil-dense-study.md) | Finite-funded 642/2,562-region cases, matched cadence/mobility controls, exact replay and explicitly non-isolated spatial inputs |
| [Fixed-input surface verification](docs/surface-spatial-verification.md) | Shared production operator, analytic sphere reference, isolated time/grid controls and retained spatial/face-flux discrepancy |
| [Seasonal-water desktop](docs/seasonal-water-desktop.md) | Run/pause, eight dynamic layers, interval inspection, versioned frames, baseline timing evidence, and limits |
| [Seasonal-water checkpoints](docs/seasonal-water-checkpoints.md) | Complete native state, candidate validation, bounded file I/O, exact continuation, and limits |
| [Typed surface water](docs/surface-water.md) | Empirical snowmelt, soil bucket, pending-runoff ownership, defaults, and limitations |
| [Delayed runoff](docs/runoff-transport.md) | Neighbor transfers, physical response times, terminal retention/evaporation, and analytic controls |
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

- Explainable static elevation: crust baseline, convergence uplift, divergence effects, bounded detail, and an optional bounded dry-preparation contribution. A displaced globe has shared-corner interpolation and display-only vertical exaggeration; the flat map remains planar.
- Initial water filling from either a coverage target or a fixed volume. Shared depth/body layers report actual coverage, the largest connected ocean, inland basins, and the resolved inventory. The Land and water layer adds a separate water-surface mesh on the globe; analytical layers expose the bed. Visible shorelines approximate whole-region water data and do not change physical coverage.
- Manual prescribed runoff can fill a selected basin, cross an unambiguous sill, and merge full neighbors. Exact native stocks drive derived depth/body layers and separate globe water levels. A separate water-checkpoint file can save, reopen, and continue this bounded inventory; an exact budget inspector shows the initial/input/storage ledger and a selected source's runoff destination. This has no elapsed time or climate forcing; ambiguous routes refuse atomically.
- `Export world data` writes a [versioned resolved initial-world report](docs/resolved-world-export.md) with native-generated fields for offline analysis. It is not a dynamic checkpoint; manual water steps remain separate.
- The [bounded dry terrain-preparation stage](docs/terrain-preparation.md) now runs before initial water fitting in `terrain-prep-1` worlds. Its 0–16 passes transport material without representing elapsed geological time or climate-driven erosion. The desktop exposes the pass count and inspector contribution.
- The [first seasonal temperature model](docs/seasonal-temperature.md) derives twelve monthly normals from latitude, initial water coverage, and elevation. It has a shared 2D/globe layer and inspector, but no weather, moisture, energy conservation, or coupling to manual water steps.
- The [first surface-wind model](docs/seasonal-wind.md) adds twelve monthly east/north velocity normals and a shared speed layer. Its smooth latitude belts shift seasonally and now drive a separate moisture model, but do not respond to terrain.
- [Seasonal water](docs/seasonal-water-desktop.md) advances finite stocks on a fixed bed, with hourly/daily stepping, run/pause, stock layers, interval precipitation/departure rates, and regional/global budgets. [Seasonal checkpoints](docs/seasonal-water-checkpoints.md) save complete native state and validate restoration in a candidate process, with exact same-build continuation. It is separate from the exact manual inventory. Legacy modes retain initial water geometry; the opt-in [regional mode](docs/regional-water-desktop.md) adds changing land-region liquid depths while initial reference-body heads/coasts remain prescribed.

The seed field is a coherent **diagnostic signal**, separate from terrain, temperature, wind, or biomes. Mesh topology remains fixed at a given resolution. General planetary hydrology, climate-driven or ongoing erosion, coupled climate, and civilization are not implemented. Elevation zero is a reference datum, not sea level. Recipes reproduce the initial world; read-only temperature and wind derivations additionally depend on their separately reported model versions and fixed settings. Cross-version climate replay is not yet supported. The separate prescribed-water checkpoint saves only the bounded manual water inventory, not a full simulation state or diagnostic field.

Static drainage assigns bed-based receivers, routes equal-height flats without altering elevation, preserves closed dry sinks, and accumulates contributing land area. Catchment and area layers are drainage potential, not flowing rivers. Basin analysis computes geometric spill thresholds and storage capacities; it does not evolve the world's water inventories.

Plates are not continents, and continental crust is not emerged land. Geology and drainage remain static after generation; manual prescribed steps can change bounded basin stocks without changing the bed or initial drainage. Diagnostic playback does not evolve water. [Basin inspection](docs/basin-inspection.md) adds branch/threshold layers, parent navigation, contact highlights, and capacity explanations in both views. New desktop recipes use `terrain-prep-1`; `continuous-plates-1` is an opt-in recipe for a different plate-root selector, not a validated replacement. Existing `basins-1` recipes remain supported without reinterpretation. Earlier versions, including `drainage-1`, are rejected.

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
