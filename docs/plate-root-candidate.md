# Continuous plate-root candidate

Status: standalone native candidate and reproducible comparison. **No generated-world recipe uses it.** `basins-1` and `terrain-prep-1` retain their original mesh-indexed roots and fingerprints. This work does not establish realistic plate tectonics or resolution convergence.

## Why test another root selector?

The current `Tectonics::build` samples its first root as an index proportional to the number of regions and samples later roots from weights over that mesh. The same seed therefore selects different physical positions when subdivision changes. A [paired-location terrain study](terrain-preparation-study.md) found that the *unprepared* height of generated worlds already differs substantially at identical sphere directions between resolutions. Changing the dry-preparation coefficients cannot remove an upstream change in geography.

The candidate in `native/src/plate_roots.rs` samples directions on the continuous unit sphere using the separate `tectonics.continuous-roots-1` random stream. For each later root it draws 16 continuous candidates and selects one with weight proportional to the square of its minimum `1 − dot` separation from existing roots. The candidate count and weighting are experimental constants, not an accepted plate-size model. Each direction is then snapped to the nearest still-unused mesh region, breaking equal-score ties by region ID. This greedy projection keeps distinct roots but is not a globally optimal assignment. Root directions and their plate-order prefix are independent of subdivision; the snapped cell and resulting boundaries remain resolution-dependent.

A standalone `Tectonics::build_with_continuous_roots` reuses the existing resistance field, multi-source connected partition, velocity stream, and boundary-motion calculation. It does **not** alter the production `build` path. Tests check reproducibility, unit directions, prefix stability, unique projections, declining nearest projection error for fixed roots, connected nonempty candidate plates, supported coarse/dense plate-count bounds, identical motion vectors to the legacy path, and a pinned legacy-world fingerprint `2e66ac09`.

## Reproduce the comparison

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example plate_roots_study
cargo run --release --locked --manifest-path native/Cargo.toml --example plate_roots_study -- --quick
```

The full report uses 12 fixed `terrain-study-XX` seeds, 12 plates, radius 6,371 km, and subdivisions 2–6. For each adjacent pair it compares roots, plate owners, and unprepared height at **exactly shared mesh centers**, weighting owner and height comparisons by coarse physical region area. Plate IDs are paired by root-selection order. Each table entry is the median of 12 per-world means or area fractions, using the mean of the middle two values.

| Coarse → fine | Legacy root shift | Candidate root shift | Legacy owner disagreement | Candidate owner disagreement | Legacy raw-height difference | Candidate raw-height difference |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 → 3 | 74.39° | 6.46° | 89.39% | 13.04% | 153.35 m | 134.98 m |
| 3 → 4 | 61.57° | 3.61° | 75.06% | 6.46% | 259.99 m | 140.90 m |
| 4 → 5 | 58.13° | 1.76° | 76.78% | 3.05% | 319.65 m | 99.95 m |
| 5 → 6 | 54.48° | 0.81° | 69.34% | 1.50% | 348.30 m | 60.47 m |

The candidate substantially stabilizes root placement and plate ownership in this sample. It also lowers median raw-height differences, especially at higher subdivisions, but does not eliminate them. At 2 → 3, two of 12 candidate worlds have a *larger* raw-height difference than their own legacy counterpart. Crust fitting, graph growth, root projection, boundary discretization, and terrain response can still vary with mesh. The table is a numerical comparison inside this model, not evidence that the new plate shapes resemble Earth.

## Decision boundary

Keep this candidate opt-in and outside accepted recipes until broader ensemble and topology behavior are evaluated: low-subdivision/high-plate-count distributions, root collisions across many seeds, plate-area and boundary-length distributions, terrain/water sensitivity, and runtime. A future switch requires a new world model version and explicit migration policy, not a silent change to `terrain-prep-1`. The current study justifies developing that version; it does not by itself justify replacing the published generator.
