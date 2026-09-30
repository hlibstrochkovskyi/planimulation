# Exact initial-water accounting: opt-in import experiment

Status: implemented as a read-only, separately versioned native report, `exact-initial-accounting-1`, September 30, 2026. It does not advance water, alter generated terrain or water, initialize the existing spill solver, or define a full simulation checkpoint.

## Authority and reconciliation

The existing generator calculates each represented regional contribution as `surface_area_m² × depth_m`, then sums those binary64 values for its global resolved volume. The existing basin importer groups the same contributions by exclusive active branch with a different floating-point addition order. Its branch stocks and global total can therefore disagree when reinterpreted as exact values.

This opt-in experiment makes the **represented regional contributions** authoritative. A wet region belongs to exactly one active branch under the existing initial-water frontier. Its finite positive binary64 contribution must be an exact multiple of `2^-56 m³` and fit `i128`; otherwise the import rejects. Branch stocks and the initial ledger are checked integer sums of those units. The old global total and each old branch stock are retained only as exact representations of their already rounded binary64 values. `reconciliationUnits = exactTotalUnits - sourceTotalUnits` is explicit and signed. The difference is **not** injected as new water, assigned to an arbitrary basin, or hidden in a tolerance.

For `n` positive represented contributions with exact sum `S`, the sequential round-to-nearest summation error is bounded by `γ_(n-1) S`, where `γ_k = k·u / (1 - k·u)` and `u = 2^-53`. Under the supported region limit, `n·u < 1/2`, so the integer ceiling of `2·n·u·S` is a conservative audit bound. The report applies this independently to every legacy branch sum and the global legacy sum. This bound concerns summation of the **represented products**; it does not bound earlier area, depth, terrain or water-level errors. A zero, sub-unit, nonfinite or overflowing wet contribution rejects rather than disappearing.

`i128` amounts are decimal strings in JSON, because ordinary JSON consumers may parse numbers as binary64. Restore checks canonical decimal encoding, version, unit, ordered branch identities, exact stock sum and reconciliation, then regenerates the pinned `basins-1` recipe and compares the complete accounting record. Display conversion to `f64` is not an accounting operation. Existing `seeded-multi-entry-network-1` through `-5` checkpoint semantics are unchanged.

## Run and validation

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example exact_initial_accounting -- docs/scenarios/spill-connections.json
```

Use `-` for stdin. Recipe input is capped at 32 KiB. The report states its read-only scope and contains the origin recipe, version, unit, exact local stocks, source totals and reconciliation. This is a developer report, not a user-facing world save.

Directed tests cover a generated discrepancy witness, exact JSON round-trip and restore, zero/full/volume water settings, rejected unit scales, version changes, malformed decimal strings, altered stocks, altered recipe, a legacy-tolerated but analytically invalid source total, a fixed 20-seed subdivision-2 smoke sample, and one 10,242-region generated world. The sample only demonstrates that this narrow import can be made explicit and reproducible; it is not a reliability estimate for dynamic water. Next, a candidate dynamic representation still needs a bounded tie/ownership policy, event accounting, full checkpoint schema and repeated-forcing validation before desktop water movement.
