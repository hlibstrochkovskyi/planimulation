# Guarded generated-world scale: milestone C3l

Status: implemented bounded headless expansion, `seeded-multi-entry-network-2`, September 29, 2026. The earlier `seeded-multi-entry-network-1` checkpoint remains restorable with its 128-region limit. C3l tests one prescribed interval on a 10,242-region generated world; it does **not** make the water model generally planet-scale or add desktop dynamics.

## Storage change and explicit bounds

The shared spill-network core now uses two depth-first tree interval arrays to answer branch containment in constant time. This replaces its `K × K` Boolean membership matrix with `2K` indices, while preserving the same basin IDs and routing. C3f/C3g/C3i and seeded v1 still use their original checkpoint versions and remain limited to 128 regions. Existing scenario outputs and checkpoint-continuation tests must remain unchanged.

The larger seeded v2 path validates these budgets **before** building duplicated curves:

| Budget | Maximum |
| --- | ---: |
| Regions | 10,242 |
| Graph edges | 100,000 |
| Basin branches | 2,048 |
| Maximum branch depth | 128 |
| Duplicated curve-column references | 500,000 |

The curve-reference count is the sum of subtree region counts over every branch; it bounds one important memory driver, not total allocated bytes. Curves still duplicate regional columns, and the concurrent solver still copies branch-rate vectors while traversing nested states. The cap is a safety boundary, **not** a claim that the representation is efficient at arbitrary scale. Subdivision 6 (40,962 regions; 2,589,588 curve references for `first-light`) is explicitly unsupported. Dense geometry, unusual basin trees or other numerical limitations can cause rejection below 10,242 regions.

The expanded checkpoint has its own version and preserves the separate initial-water and external-input ledgers, resolved geometry and weights, original recipe and atomic failure behavior from [C3k](seeded-network.md). Its unit branch/entry weights remain an uncalibrated experimental policy. Restoring v1 enforces the old limit; relabeling an oversized v2 checkpoint as v1 is rejected.

## Reproducible profile

`seeded_profile` accepts the generated start and **one** interval from [the C3k scenario](scenarios/seeded-network.json). It emits a compact report rather than serializing the full network checkpoint. Preparation/interval durations are wall-clock observations; `peakRssKib` is Linux `VmHWM` when available and includes generation, setup, provenance checking and profiling overhead. It is not the solver's isolated memory footprint. The report records accepted or rejected outcomes and never silently omits a failed seed.

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example seeded_profile -- docs/scenarios/seeded-network.json
```

For another supported resolution, change only `start.generated.recipe.subdivision` in that scenario; all resolved recipe fields remain in the output. One release-build observation on an AMD Ryzen 5 PRO 4650U laptop with 14 GiB RAM, seed `first-light`, coverage 0.25 and 2 × 10^14 m³ at region 3:

| Subdivision | Regions | Branches | Curve references | Preparation | Interval | Process peak RSS | Outcome |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 42 | 9 | 72 | 0.48 ms | 0.008 ms | 3.7 MiB | Accepted, 3 events |
| 2 | 162 | 42 | 879 | 1.84 ms | 0.010 ms | 3.9 MiB | Accepted |
| 3 | 642 | 112 | 7,380 | 7.10 ms | 0.035 ms | 5.1 MiB | Accepted |
| 4 | 2,562 | 234 | 70,708 | 57.81 ms | 0.173 ms | 10.4 MiB | Accepted |
| 5 | 10,242 | 451 | 411,318 | 158.85 ms | 0.325 ms | 35.2 MiB | Accepted |

These are individual runs, not throughput guarantees. The timed interval contains one large input, not precipitation at every region or many years of simulation. For subdivision 5, the accepted report's initial, external and final volumes were 7,506,994,258,411,541; 200,000,000,000,000; and 7,706,994,258,411,541 m³. Its reported budget residual was 8 m³, within the existing relative audit tolerance; this residual is retained, not described as zero.

The [complete fixed 20-seed sample](data/expanded-seeded-profile.csv) at subdivision 5 used exactly the same water fraction, input region and volume. Sixteen runs were accepted; `profile-06`, `profile-16` and `profile-17` exceeded the 500,000-reference cap, while `profile-01` was prepared but rejected its interval with `Concurrent update exceeds capacity.` The latter is a numerical threshold case, not evidence that water disappeared: the attempted interval leaves the checkpoint unchanged. The accepted runs' absolute budget residuals were at most 22 m³, under the current tolerance. No seed was substituted or excluded from the record.

## Validation and next gate

Expanded tests check subdivisions 2, 3 and 5, checkpoint replay with the original recipe, input/storage budgets, the retained `profile-01` precision failure, and the `profile-06` storage-cap rejection. Earlier C3f/C3g/C3i/C3k tests check unchanged bounded behavior. The profiler is an observational developer tool, not a simulator output layer.

Validation on September 29, 2026: `npm test`, Clippy across all Rust targets with warnings denied, Rust formatting, and `npm run test:desktop` passed. The desktop fingerprint remained `2e66ac09`. The existing Vite chunk-size advisory remains; no renderer or native wire-protocol feature was changed.

[C3m](exact-limit-commit.md) identifies `profile-01`'s one-step floating-point endpoint overshoot and fixes exactly limiting events in a separate v3 experiment, preserving this v2 record. Before using this for a desktop water demonstration, replace the repeated per-branch curves with a scalable storage index, test distributed inputs and long replay, and measure supported/unsupported outcomes at the intended default resolution across more seeds. Current v2 already shows that 10,242 regions is **conditionally** feasible, not universally supported. Climate, erosion and rivers remain later work.
