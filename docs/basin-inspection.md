# Basin inspection: milestone C2b

Status: integrated native generation, binary transport, headless summary, flat/globe layers, and basin inspector. Model recipe `basins-1`, protocol 7, analysis algorithm `basin-analysis-1`. The [C2a analysis](basins.md) is unchanged. This is static analysis, not reservoir inventories, filling, overflow, or climate. Milestone C is not complete.

## Ownership and reproduction

Rust generates and owns the hierarchy once per world after the existing terrain, water, and drainage stages. Initial water coverage or volume does not change the bed-based hierarchy. Diagnostic diffusion does not update it. Renderer geometry and display exaggeration do not change basin fields.

Recipe parameters are unchanged apart from the model version. Previous recipes, including `drainage-1`, are explicitly rejected without overwriting or automatically migrating them. Upstream generation algorithms are unchanged; the added state and model version change the initial fingerprint. No RNG stream, dependency, or physical parameter is added.

The analysis retains its read-only height/area snapshots for native volume queries. These copies are not sent to the renderer: existing world arrays already contain those values. Model-array byte statistics describe the wire arrays, not total native/renderer memory. The developer example reuses the world's hierarchy rather than computing it a second time.

## Protocol 7

All messages use protocol tag 7. World headers additionally require `basinNodeCount = K` and `basinAnalysisVersion = "basin-analysis-1"`. The adapter bounds `1 ≤ K ≤ 2N − 1` before decoding. Append these arrays after protocol 6's drainage fields, with unchanged little-endian encoding:

| Field | Encoding | Meaning |
| --- | --- | --- |
| Region owners | N × Uint32 | Exclusive branch IDs, not catchment IDs |
| Parents | K × Uint32 | Parent ID; the unique root refers to itself |
| Birth levels | K × Float64 | Minimum/merge threshold in meters |
| Spill levels | K × Float64 | Next merge threshold in meters |
| Contact from | K × Uint32 | Sill-region ID |
| Contact to | K × Uint32 | Adjacent lower region in the child subtree |
| Support areas | K × Float64 | Entire subtree footprint in m² |
| Capacities | K × Float64 | Entire subtree volume at its next merge in m³ |

Additional payload size is `4N + 44K` bytes. Children are derived from parents instead of serialized redundantly. The root has no finite spill threshold/capacity: its binary placeholders are `spill = birth`, `capacity = 0`, and both contact IDs `N`. These values are **not** physical measurements and are never displayed as root capacity or spill level. The standalone JSON report retains nullable root fields.

Validation checks field lengths, finite values, IDs, one root, strictly increasing parent IDs/merge heights, zero-or-at-least-two children, exclusive region elevations, each node's formation elevation, subtree areas, capacities, contact adjacency, sill elevation, and contact membership. An iterative tree interval traversal checks ancestry without recursion; a bottom-up audit reconstructs areas and volumes from physical region values. Tolerance is `max(1e-6, abs(expected) × 1e-10)` in the relevant units. This is a structural/volume audit, not a second connectivity sweep proving optimality of every saddle. C2a's independent threshold-BFS tests cover that algorithm.

The existing 32 MiB body and 32 KiB header bounds remain. Diagnostic frames still contain only the unrelated diagnostic field, tick, and conservation metric.

## Views and interpretation

- **Basin branches** uses categorical colors for exclusive ownership. A parent's full footprint includes its children, which retain their own colors; this is not a map of complete nested basin extents or current lakes.
- **Spill thresholds** colors each region by its owner's next connection height, using the world's finite minimum/maximum. The root is neutral gray, explicitly meaning no external drain. A root-only world reports no finite thresholds. The scalar colors are not water depths or live water levels.
- Both layers show the bed on the globe, including underwater terrain. Initial water surfaces remain exclusive to the Land and water layer. Neither layer adds a hypothetical lake mesh.

Selecting a region opens its owning branch. The inspector reports its type, parent, child IDs (first 12 plus total count), formation height, next connection height, capacity, support area, and threshold contact. **Inspect parent** moves upward in the hierarchy without changing the selected region. **Region branch** returns to that region's owner. View switching, display exaggeration, and diagnostic playback preserve the inspected branch; regeneration clears inspection.

On the two basin layers, cyan marks the inspected branch's contact pair; gold retains priority for the selected region. These are two adjacent native regions, not an animated spill path or a full contour. A contact on the far hemisphere requires rotating the globe to see it. Root inspection clears the contact highlight. Other layers do not display these contact marks.

Capacity includes all descendant storage. It is not current stock, additional rain required, or remaining empty capacity; summing capacities across nested nodes double-counts water. At a threshold the sill has zero depth. Actual overflow requires sufficient source inventory and a receiving reservoir state, neither of which this inspector simulates.

## Validation and next step

Checks include known weighted nested-bowl values, every new binary field, invalid metadata/versions, bad ancestry and in-range wrong contact edges, root-only dry/wet worlds, repeated generation, water-setting independence, display preparation, and finest-resolution transport bounds. Desktop checks exercise both layers and legends, contact IDs, parent/owner navigation, identical data on both views, reset behavior, root placeholders, and unchanged fingerprints during inspection.

Validation recorded September 24, 2026:

- `npm test`: 36 Rust tests and 43 TypeScript tests passed, including type checking and the native release build.
- `cargo fmt --manifest-path native/Cargo.toml -- --check` and `cargo clippy --locked --manifest-path native/Cargo.toml --all-targets -- -D warnings` passed.
- `npm run test:desktop` and `npm run test:desktop -- release/Planimulation-linux-x64/planimulation` passed against the development and packaged Linux applications. Parent inspection remains selected through diagnostic playback. The packaged inspector screenshot was visually checked for readable labels and values.
- Production build and Linux packaging passed. Vite still reports its advisory warning for the renderer chunk exceeding 500 kB; this is not a frame-time measurement.
- The default `first-light` recipe has 10,242 regions, 226 minimum basins, 451 hierarchy branches, and root branch 450. Finite thresholds range from −4,583.571470104631 m to 716.7122810608688 m. Headless and both desktop builds report fingerprint `2e66ac09`.
- Model arrays occupy 5,792,952 wire bytes, including 60,812 additional basin bytes. This excludes object overhead, retained native query snapshots, and render geometry. No new performance benchmark is claimed for this increment.

[C3a](reservoir-experiment.md) now supplies a standalone isolated-reservoir pulse experiment with stock/collector budgets and checkpoints; this inspector remains static. Next define neighbor receiving paths, conservative exchange, and fill/merge behavior before claiming dynamic lakes or rivers. Preserve existing water stock when assigning initial planetary reservoirs; do not repeatedly refit a global water-coverage target.
