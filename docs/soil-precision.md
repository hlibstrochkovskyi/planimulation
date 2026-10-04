# Checkpointed soil precision candidate

Implemented and measured October 4, 2026. This is a **headless-only numerical candidate**, not a new climate law or a qualified desktop mode. `seasonal-moisture-5`, schema 5, uses `surface-water-compensated-soil-1` with the existing opt-in upslope response. Default version 3 and desktop version 4 remain unchanged. Milestone D is incomplete.

The subsequent [surface/terminal continuation](surface-precision.md) retains this version-5 model and its failed qualification while adding versions 6/7. Version 7 has a separate opt-in desktop mode and bounded ten-year evidence; version 5 remains headless-only. The measurements below are historical, not silently upgraded results.

## Diagnosis before correction

The [version-4 refinement record](data/orographic-response-validation.json) retains two soil-ledger rejections at 60-second coupling. Surface transfer totals already use compensated accumulation; soil stocks still used ordinary floating-point additions/subtractions. Compensating a flow ledger does not compensate the stock it describes.

A read-only observer now exposes copied before/after local surface operations. A separate growing floating expansion audits the represented stock change against the actual transfers, rather than repeating the production ledger's Kahan accumulation. Its audit includes provisional operations in a rejected caller interval; these are **not committed transfers**. The state remains unchanged on rejection.

| Version-4 witness | Region | Rejected endpoint, seconds | Accumulated soil arithmetic drift, kg |
| --- | ---: | ---: | ---: |
| `seasonal-reference`, 1,000 km radius | 99 | 2,654,880 | +23.7119955180 |
| `moisture-coast`, 1,000 km radius | 103 | 2,979,660 | −28.7980729559 |

Each observed operation's relative rounding residual is below `2.0e-16`, but repeated updates accumulate enough drift to cross the unchanged regional `1e-12` ledger gate. This identifies a numerical representation problem in these witnesses; it does not establish that all future failures have the same cause. Full recipes, settings, expansion terms, rejection messages, and scope labels are retained in the [new measured record](data/soil-precision-validation.json). The original record is not rewritten.

## Representation and bounded transfers

Each soil stock is represented by two normalized components:

```text
soil mass = high + low
(sum, error) = TwoSum(high, transfer)
(new_high, new_low) = TwoSum(sum, low + error)
```

The signed `low` is part of the **same owned soil mass**, not a seventh reservoir, an external input, or a flow-ledger correction. Both components persist between updates and are included in stock identities. Existing Kahan flow-ledger correction fields keep their separate meaning.

The error-free transformation for a pair of finite floating inputs follows Knuth's TwoSum, described in [Ogita's numerical-accuracy presentation](https://ogilab.w.waseda.jp/ogita/math/presen/Dag2005_Ogita.pdf). This implementation is a bounded two-component approximation: `low + error` can itself round. It is not arbitrary precision, an exact accumulator for every possible history, or a proof of the complete simulator's conservation.

Withdrawal is bounded by the greatest leading floating value no greater than the represented stock. When the normalized low component is negative, that bound is `high.next_down()`. Deposit computes represented remaining capacity and applies the same conservative bound. Rust documents [`next_down`](https://doc.rust-lang.org/std/primitive.f64.html#method.next_down) as the next smaller representable value; using it for these transfer grants is our numerical policy, not a borrowed physical law.

Normalization, finiteness, nonnegative represented mass, and capacity bounds are checked. Positive low mass above full capacity rejects. Small positive or negative tails are not erased at depletion or saturation. No stock is reconstructed from cumulative flows, no final budget repair is applied, and no arithmetic or ledger tolerance is widened.

Process order and empirical coefficients are unchanged. Soil-dependent response coefficients still evaluate the rounded leading field. Liquid, snow, vapor, transit, and terminal water retain their existing ordinary `f64` stock representation. Consequently this increment does not resolve precision throughout the water cycle.

## Versioning, persistence, and isolation

Schema 5 requires all of the following to agree:

- `modelVersion: "seasonal-moisture-5"` and `surfaceModelVersion: "surface-water-compensated-soil-1"`.
- Resolved `settings.soilNumerics: "compensated"`, an enabled upslope configuration, and its unchanged `orographic-response-1` module pin.
- A `soilLowKilograms` array matching the region count, with normalized, bounded components. Reference-water regions cannot own soil low mass.

Missing optional fields preserve legacy schema-3/4 records; present `null` rejects. Legacy records omit both new fields and are never silently upgraded. Version-5 JSON checkpoints restore all components and support exact same-build continuation. The six matched version-4 budgets exactly match the previously retained record.

There is deliberately **no desktop protocol for version 5**. Native display/checkpoint writers reject this candidate before emitting bytes; existing desktop protocols 11/12 and initialization remain unchanged. Headless checkpoint support does not imply application save/load support.

## Measured acceptance and refusal

The two original soil witnesses complete 40 days at 60-second coupling. Their independent selected-region soil audits report zero accumulated represented arithmetic drift for this workload. This is measured evidence, not an unlimited exactness claim.

Six generated cases cover three seeds, subdivision 2, and radii 1,000/6,371 km. Each compares version 4 against version 5 at the same actual 1,800-second coupling, then refines version 5 to 60 seconds. All six 40-day runs pass the unchanged budgets and exact JSON round-trip/one-hour continuation checks. Componentwise six-stock-plus-low L1 bounds, normalized by initial mobile water, are:

- Candidate versus legacy at matched coupling: `7.8e-19`–`3.5e-16`.
- 60-second versus 1,800-second candidate: `5.9e-5`–`4.5e-4`.

These are componentwise difference bounds, not calibrated climate errors or a spatial convergence study. The largest local ledger residual across the refined cases is about `8.95e-13`, already close to the `1e-12` gate; this maximum includes the other, uncompensated stocks.

Two attempted 365-day, 60-second-coupled qualifications reject at the **snow** ledger:

| Version-5 witness | Region | Rejected caller endpoint, seconds | Relative snow-ledger residual | Audited snow arithmetic drift, kg |
| --- | ---: | ---: | ---: | ---: |
| `seasonal-reference`, 1,000 km radius | 151 | 13,824,000 | `1.00253961655e-12` | −24.7403049861 |
| `moisture-coast`, 1,000 km radius | 95 | 16,261,200 | `1.00617745945e-12` | −27.0171917618 |

These are failed hourly caller endpoints, around days 160 and 188, not completed annual states or a measurement of the first individual substep crossing. Independent snow audits retain the same provisional-interval scope and confirm accumulated ordinary-stock rounding. The qualification record explicitly reports `qualificationPassed: false` and `failureCount: 2`. No failed case is excluded from the report.

Native tests cover an independent integer oracle for 100,000 bounded soil updates, signed tails, capacity/depletion, cold-interval persistence, malformed components/pins, omitted legacy fields, observed/unobserved batching, both original 40-day witnesses, and day-31 exact checkpoint continuation with nonzero low mass. The expansion audit also has a cancellation oracle. Full native/all-target tests, 68 TypeScript/adapter tests, warning-free Clippy, and real Electron regression checks pass. These tests do not override the failed annual qualification.

## Reproduction and next gate

Run from the repository root:

```sh
cargo run --release --manifest-path native/Cargo.toml --example soil_ledger_probe
cargo run --release --manifest-path native/Cargo.toml --example soil_ledger_probe -- --compensated
cargo run --release --manifest-path native/Cargo.toml --example soil_ledger_probe -- --compensated --snow
cargo run --release --manifest-path native/Cargo.toml --example soil_precision_report
cargo test --manifest-path native/Cargo.toml --test soil_precision
```

The qualification report prints all results and then exits nonzero if any case fails. With the retained inputs, its two annual failures are expected; this must not be interpreted as successful qualification merely because the short runs pass.

The next numerical gate is persistent precision for snow and the other long-lived stocks, with signed-component accounting, strict checkpoint validation, and retained long-run failures. Broader time/resolution/parameter sensitivity and climate calibration remain separate gates. Do not promote this candidate to the desktop/default or begin ecology on the strength of these short runs alone.
