# Exact limiting-capacity commit: milestone C3m

Status: implemented in the bounded headless `seeded-multi-entry-network-3` experiment, September 29, 2026. This is a numerical endpoint correction to [C3l's expanded generated-water experiment](expanded-seeded-network.md), not scalable storage, calibrated discharge, or desktop water dynamics. Generated starts above 128 regions now use v3. Existing v1 and v2 checkpoints restore under their original rules; the dry C3f/C3g/C3i experiments are unchanged.

## Measured failure and versioned rule

The recorded `profile-01` subdivision-5 run failed in v2 at branch 254 during an exactly limiting event. At the point of failure, the old stock was approximately 5.29726258129178711 × 10^12 m³, capacity was 2.83942744282468555 × 10^13 m³, and the prescribed grant was `capacity - old`. Floating-point addition rounded `old + grant` one representable step (0.00390625 m³) **above** capacity. The v2 capacity check correctly rejected the whole interval; no input was committed or lost.

For v3, only when the event solver identifies a branch as **exactly limiting**, it commits that branch's validated capacity as its endpoint. It verifies that the grant is exactly the represented deficit and that the ordinary rounded sum differs from capacity by no more than one adjacent `f64` step. Event accounting uses the **actual** stock difference (`new - old`), and the existing segment, interval and complete-ledger audits still run. Nonlimiting overcapacity, unrepresentable additions and budget failures still reject the entire interval. There is no general clipping of excess water and no tolerance-based saturation of a merely near-full branch.

The branch/entry weighting and global event sequence are otherwise unchanged. This deliberately creates a new experiment version because a previously rejected input is now accepted and continued history can differ. It does not solve inputs below stock precision, exact dry-sill initialization, drying/splitting, or long-run numerical stability.

## Evidence and remaining limits

The v3 `profile-01` interval now completes and records branch 254's saturation; its reported full-ledger budget residual is -22 m³, below the existing relative budget tolerance. Checkpoint serialization and replay produce the same interval and state. A targeted arithmetic regression demonstrates the one-step overshoot and verifies that the legacy v2 run still rejects atomically and that v3 rejects a nonlimiting overcapacity or mismatched grant.

The fixed [20-seed C3l sample](data/expanded-seeded-profile.csv) was rerun in release mode with the same subdivision-5 geometry, 25% initial coverage and 2 × 10^14 m³ at region 3. The [complete v3 outcomes](data/exact-limit-profile.csv) show 17/20 accepted: `profile-01` joined the 16 previously accepted seeds. `profile-06`, `profile-16` and `profile-17` still fail preparation at the 500,000 duplicated curve-column-reference cap. All accepted runs reported absolute complete-ledger residuals of at most 22 m³. The C3l CSV remains the historical **v2** baseline, including its numerical rejection; it is not relabeled as v3 data. This fixed sample is an engineering smoke test, not a general acceptance rate.

Reproduce each row by changing only `start.generated.recipe.seed` to its recorded seed and `start.generated.recipe.subdivision` to 5 in [the generated interval scenario](scenarios/seeded-network.json), then running the release `seeded_profile` example. The CSV intentionally omits wall-clock measurements, which vary between runs and were not needed to diagnose this arithmetic change.

The next gate is replacing repeated per-branch storage curves with an indexed representation and measuring distributed forcing, many intervals, replay and supported/unsupported seeds at the intended default resolution. The v3 correction removes one known arithmetic false rejection; it does not establish robust planetary water dynamics.
