# Bounded generated-water continuation: milestone C3k

Status: implemented headless, `seeded-multi-entry-network-1`, September 29, 2026. C3k connects [C3j's faithful initial-water inventory](initial-water-inventory.md) to [C3i's concurrent multiple-entry laboratory](multi-entry-network.md) **only for at most 128 regions**. The original dry C3f/C3g/C3i checkpoint versions and semantics remain unchanged. This is not planet-scale water dynamics or a desktop feature.

## State and accounting

The separately versioned seeded checkpoint stores complete bounded geometry and explicit branch/entry weights, the validated C3j initial-water artifact, active stocks, cumulative **external** input, and completed-interval count. A generated start also records its origin recipe. Restore regenerates that small world and verifies the stored geometry and initial water against it, so provenance cannot silently diverge. Synthetic laboratory starts have no origin recipe. At every snapshot:

```text
initial water + accepted external input = active stored water + budget residual
```

The initial volume is never inserted into `inputCubicMeters`, and initialization consumes no interval. The internal solver checks the two ledgers together; save/restore rebuilds geometry and audits the initial artifact against that geometry. Legacy dry checkpoints cannot silently become seeded checkpoints. Failed intervals leave the previous state unchanged, following C3g/C3i's atomic update rule.

The importer preserves the generated water's regional depths and positive-depth body IDs before dynamics. A dynamic reservoir curve may reconstruct an initial level differing by floating-point roundoff; initialization rejects deviations beyond the stated native tolerance. At the exact dry sill, C3j correctly keeps child bodies separate, while the current dynamic frontier requires full siblings to be merged. C3k **rejects** that initial state explicitly rather than changing physical connectivity or pretending it is supported. Dry and fully covered initial worlds are supported in the bounded experiment. Water removal, basin splitting, sub-precision pending input, and climate-derived forcing are not implemented.

## Explicit generated-world policy

`generated_unit_setup` derives the 128-region-or-smaller column graph from the generated world and records weight 1 for each non-root branch and each distinct receiving entry. The policy is named `unit-branch-and-entry-weights-1`. These are deliberate laboratory weights, **not** inferred hydraulic conductance or calibrated runoff. The `branch-then-entry-weights-1` allocator, geometric contact rules and global saturation events remain those of C3i.

The [reproducible scenario](scenarios/seeded-network.json) generates `first-light`, subdivision 1, coverage 0.25 (42 regions). It applies 2 × 10^14 m³ at region 3 over one normalized interval. Initially there are three separate water bodies and 4,392,913,244,410,118 m³ of water. The interval has three events: branch 3 saturates, water transfers to branch 5, then their parent branch 7 becomes active. Final stored water is 4,592,913,244,410,118 m³; the checkpoint keeps 2 × 10^14 m³ as external input and the initial volume separately. The prescribed input is deliberately large to exercise thresholds, not a plausible rainfall estimate. An interval is a normalized forcing window, not a calibrated day or hour.

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example seeded_network -- docs/scenarios/seeded-network.json
```

The CLI accepts either `{"start":{"generated":{"recipe":...,"weightPolicyVersion":"unit-branch-and-entry-weights-1"}},"intervals":[...]}` or `{"start":{"checkpoint":...},"intervals":[...]}`. It limits requests to 32 KiB and 1,024 intervals. It emits no partial report if a late interval fails. Reports contain both initial and final checkpoints, an initial snapshot and interval event records. A checkpoint contains enough bounded geometry and state to resume; generated checkpoints additionally carry `originRecipe`, which is rechecked on restore. The original recipe is not rerun at every simulation interval.

## Validation and remaining gate

Directed tests cover distinct ledgers, zero elapsed intervals, accepted input, exact-sill rejection, corrupt initial/current checkpoints, false origin recipes, version isolation, JSON checkpoint replay, and a generated branch-to-branch spill/merge. A fixed smoke ensemble of 12 seeds × 3 coverage settings at 42 regions completed one prescribed interval per world without filtering failed seeds; separate dry and fully covered generated cases also advanced. This is evidence for this bounded setting, not proof for arbitrary planetary forcing or resolutions.

Validation on September 29, 2026: `npm test`, Clippy across all Rust targets with warnings denied, and `npm run test:desktop` passed. The desktop fingerprint remained `2e66ac09`; no renderer or binary-wire behavior changed. Release CLI file and stdin reports matched byte-for-byte. A 10^-4 m³ input into the already large sample lake was rejected atomically as below current stock precision; no pending stock was invented. The existing Vite chunk-size advisory remains.

[C3l](expanded-seeded-network.md) replaces the dense membership matrix, places explicit bounds on the remaining duplicated curves, and measures conditional runs through subdivision 5 under a separately versioned expanded checkpoint. This v1 experiment and its 128-region limit remain as a replayable reference. Only after scalable storage and explicit treatment of the measured numeric/threshold failures should the model drive a limited desktop run/pause water demonstration.
