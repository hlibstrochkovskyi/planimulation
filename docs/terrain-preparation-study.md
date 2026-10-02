# Dry terrain-preparation sensitivity study

Status: reproducible exploratory measurement of the existing `terrain-prep-1` heuristic. It does not calibrate the model, demonstrate resolution convergence, or simulate elapsed-time erosion. No generator coefficient was changed by this study.

## Reproduce

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example terrain_preparation_study
cargo run --release --locked --manifest-path native/Cargo.toml --example terrain_preparation_study -- --quick
```

The full command emits a version-2 JSON report with 240 seed/mesh/pass samples, 192 generated-world paired-location comparisons, and 16 analytic-bed paired-location controls: seeds `terrain-study-00` through `terrain-study-11`, subdivisions 2–6, and requested pass counts 0, 4, 8, and 16. The quick mode is for smoke checks, not the tables below. Each seed/resolution pair builds the same initial crust, plates, and unprepared terrain once; preparation variants start from that identical bed. The recipe template pins all other parameters, including a 6,371 km radius, 12 plates, and 71% initial water-coverage target; its zero-pass field identifies the shared raw baseline, while each sample records its actual requested passes. The report also records the kernel constants. No diagnostic playback or dynamic water is involved.

For each variant the report records the area fraction with more than 1 nm of net height change, area-weighted mean absolute and RMS bed change, maximum local change, gross transported reference-area-equivalent volume divided by planet area, edge counts above the 0.004 slope threshold, area-weighted RMS and maximum neighbor slope, reference-area material-ledger error, and the initial water-volume difference after separately resolving the same coverage target. Gross transport counts repeated motion and is **not** net erosion or global bed loss. The water difference is **not** a loss of previously existing water: the dry bed changes before the initial coverage constraint is fitted.

## Observations

Medians below use the mean of the two central values across 12 seeds. Water volume is relative to the same seed's unprepared, zero-pass world. The RMS-slope column is the median of paired after/before ratios minus one, not a comparison of unrelated world averages.

| Subdivision | Regions | Changed area, 4 passes | Mean absolute change | RMS slope change | Fitted water volume change |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 162 | 0.00% | 0.00 m | 0.00% | 0.00% |
| 3 | 642 | 7.16% | 11.41 m | −3.10% | −0.93% |
| 4 | 2,562 | 11.55% | 21.45 m | −6.19% | −2.00% |
| 5 | 10,242 | 13.19% | 15.70 m | −5.36% | −0.81% |
| 6 | 40,962 | 14.45% | 9.33 m | −5.86% | −0.33% |

All 12 subdivision-2 worlds remain unchanged even after 16 requested passes. At subdivision 5, increasing the request from 4 to 8 to 16 passes raises median changed area from 13.19% to 13.82% to 14.50%, and median absolute bed change from 15.70 m to 24.43 m to 34.15 m. At four passes, the 100 m-per-pass local cap produces a 400 m maximum net change in all 12 worlds at subdivisions 4–6; seven of twelve subdivision-3 worlds also reach 400 m. Thus the cap is an active part of the observed behavior, not merely a remote safety limit.

The RMS and maximum neighbor slopes both fall in every measured nonzero sample. A count of edges above the fixed threshold does **not** always fall: at subdivision 6 and four passes it rises in 10 of 12 worlds, while area-weighted RMS slope still falls in all 12. Redistribution can flatten steep edges while moving other edges just across the threshold. A single threshold count is therefore a misleading quality metric.

The largest observed relative area-weighted material-ledger error across all 240 samples is below `6.0e-15`. All four-pass worlds at subdivisions 3–6 have lower separately fitted initial water volume than their zero-pass counterparts under the fixed 71% coverage target. This is a consistent observation in this small seeded set, not a general law about erosion or water conservation.

## Matching locations across resolutions

Refining the spherical mesh preserves the earlier vertices as the first region centers, so comparisons below use **exactly the same directions** on adjacent levels, weighted by the coarse regions' physical areas. `Mean raw difference` compares the unprepared height at those directions; `mean preparation difference` compares the signed height change caused by four passes. Generated-world values are medians across the 12 paired seeds. The analytic control samples one fixed continuous field, `4000 sin(11x+3y) + 3000 cos(7y−5z) + 2000 sin(13z−4x)` meters on unit-sphere coordinates; its raw values at shared directions are identical by construction. It is a numerical test surface, not a geological model.

| Coarse → fine | Shared locations | Generated raw difference | Generated preparation difference | Analytic raw difference | Analytic preparation difference |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 2 → 3 | 162 | 153.35 m | 12.63 m | 0 m | 127.04 m |
| 3 → 4 | 642 | 259.99 m | 20.27 m | 0 m | 101.50 m |
| 4 → 5 | 2,562 | 319.65 m | 16.79 m | 0 m | 74.89 m |
| 5 → 6 | 10,242 | 348.30 m | 16.18 m | 0 m | 43.43 m |

The generated world's *input bed* changes substantially even at shared positions. One known cause is that plate roots are selected from the mesh using its region count; the same RNG draw therefore maps to different candidate regions at different resolutions. Other upstream discretization effects may contribute. Generated-world paired differences cannot by themselves measure convergence of the preparation kernel. On the fixed analytic bed, the preparation difference decreases with refinement but is still 43.43 m between subdivisions 5 and 6 after four passes. Four data points on one synthetic field are insufficient to extrapolate a converged limit.

## Engineering conclusion

The operation is deterministic and conserves its declared reference-area material, but its effect depends strongly on model resolution. The zero effect at subdivision 2, the non-monotonic mean change from subdivisions 4–6, and the active per-pass cap prevent treating the current constants as resolution-independent physical parameters. The paired-location control also identifies resolution-dependent upstream geography; aggregate generated-world medians therefore cannot isolate the kernel or establish convergence.

Keep `terrain-prep-1` as a reproducible experimental baseline. Do not silently retune its threshold, relaxation fraction, or cap under the same model version. Before selecting a successor, separate resolution-stable initial geography from the preparation kernel, then run controlled coefficient/cap sweeps on fixed fields with explicit acceptance criteria for material accounting, slope behavior, geographic effects, and water sensitivity. A changed algorithm or constants require a new model version and regression fixtures; the current desktop default of four passes is a demonstrable setting, not a calibrated geological estimate.
