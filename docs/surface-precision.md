# Persistent condensed-water precision and desktop replay

Implemented October 4, 2026. This changes numerical representation, **not climate or hydrology laws**. It extends the [soil candidate](soil-precision.md) to liquid/snow components in headless version 6, then terminal-water components in version 7. The latter is separately selectable in the desktop. Default version 3, opt-in version 4, and headless version 5 remain unchanged. Milestone D is incomplete.

## Diagnosis and retained limits

Version 5 fixes the original soil witnesses but retains snow-ledger failures near days 160/188. An unreleased snow-only diagnostic removed the audited snow drift and exposed liquid-ledger failures around day 189. That trial was not a supported checkpoint format.

Version 6 compensates liquid, snow, and soil. Six annual matched/refined cases, three denser-grid annual cases, and a default-coupled ten-year case pass. Independent selected-region snow and liquid audits complete the year with zero accumulated represented drift in these workloads. This does not promise exactness for every history.

The additional ten-year, 60-second-coupled version-6 reference rejects at second **100,314,000**, region **8**, terminal ledger **6**, relative residual `1.00052087178e-12`. Its last accepted state is day 1161; rejection is atomic, that state round-trips exactly, and repeating the next-hour rejection reproduces exactly. Its terminal stock still used ordinary additions/debits despite a compensated transfer ledger.

Version 7 changes only terminal-stock representation on the version-6 physical path. The [retained record](data/surface-precision-validation.json) includes failed version-6 qualification, successful version-7 results, and independent local-operation audits. Earlier [version-4/5 evidence](data/soil-precision-validation.json) is not rewritten; its entire matched-control/failed-qualification report reproduces exactly after the helper refactor.

The independent region-8 terminal audit accumulates `-242.8782118450849 kg` of represented arithmetic drift in version 6, including its rejected provisional interval. Version 7 reaches the ten-year limit with `-1.944588582529068e-14 kg` over 21,024,000 provisional debit/credit operations. The latter is small but **not zero**; this independent expansion audit is not a claim of exact arithmetic.

## One mass, persistent components

For each compensated stock:

```text
owned mass = high + low
(sum, error) = TwoSum(high, signed transfer)
(new high, new low) = TwoSum(sum, low + error)
```

The normalized signed low component belongs to the same stock, not another reservoir or an input. [Knuth's TwoSum, described by Ogita](https://ogilab.w.waseda.jp/ogita/math/presen/Dag2005_Ogita.pdf), underlies the update. This bounded two-component algorithm can still round its low-component addition; it is not arbitrary precision or proof of unlimited conservation.

Transfers retain existing order, temperature gates, rates, capacities, delay rules, and coefficients. Snowfall/rain/melt/terminal arrivals credit actual recipients. Evaporation and melt debit grants bounded by represented donor mass. Infiltration is bounded by both liquid donor and remaining soil capacity. Negative low tails use the next smaller leading float for a conservative grant, following Rust's [`next_down` semantics](https://doc.rust-lang.org/std/primitive.f64.html#method.next_down). Tails are not discarded at depletion/saturation; overflow rejects instead of clipping input.

Coefficients still evaluate leading fields. Transit and vapor stocks remain ordinary binary64; their transport kernels are not replaced. All six stocks still have one owner. Cumulative Kahan corrections keep their distinct accounting meaning and are not physical mass. Regional identities and global totals include applicable low components. No budget repair, redistribution, stock reconstruction from ledgers, or tolerance widening is introduced.

## Version and checkpoint contracts

| Mode | Model/schema | Persistent low arrays | Additional settings/module pins | Desktop |
| --- | --- | --- | --- | --- |
| Soil candidate | 5 | soil | `soilNumerics: "compensated"`, `surface-water-compensated-soil-1` | No |
| Surface candidate | 6 | soil, snow, liquid | Above plus `surfaceNumerics: "compensated"`, `surface-water-compensated-surface-1` | No |
| Surface and terminal | 7 | soil, snow, liquid, terminal | Above plus `terminalNumerics: "compensated"`, `terminalStockModelVersion: "terminal-stock-compensated-1"` | Opt-in protocol 13 |

Candidates require the existing upslope configuration and `orographic-response-1`. Schema 7 stores `soilLowKilograms`, `snowLowKilograms`, `surfaceLowKilograms`, and `terminalLowKilograms`. Model/schema/settings/module pins must agree. Arrays require region count, normalized finite components, soil capacity/reference-water rules, and terminal graph ownership. Initial components cannot conceal extra water. Missing fields preserve legacy shapes; present `null` rejects. No legacy file silently upgrades.

Cloned-update/commit covers the added components. Independent surface/terminal observers receive copied provisional operations, including an interval later rejected. They cannot mutate authoritative state; failed-interval observations are diagnostics, not accepted flow totals. Batching and observed/unobserved tests compare the entire state.

## Measured scope

Version 7 completes six subdivision-2 annual cases: three seeds at radii 1,000/6,371 km. Each includes a forty-day version-5/control comparison at matched actual 1,800-second coupling, then a full-year version-7 comparison at 1,800/60 seconds. Complete JSON round trips and another hour match exactly.

- Matched forty-day componentwise L1 bound / initial water: `2.15e-15`–`3.43e-15`.
- Annual time-refinement bound / initial water: `3.33e-4`–`2.36e-3`.
- Maximum relative mass residual across refined annual cases: `1.12e-15`.
- Maximum regional-ledger residual across refined annual cases: `2.16e-14`.

Additional annual runs pass at subdivision 3 (two seed/radius cases, 900-second limit) and subdivision 4 (one default-coupled case). A subdivision-2 Earth-radius reference completes ten years at default coupling. These are generated-world scope checks, **not spatial convergence evidence**.

The separate 1,000 km-radius, subdivision-2 reference completes **3,650 days at 60-second coupling**. Maximum global relative residual is `6.49e-15`, maximum regional-ledger residual `9.61e-15`, below unchanged `1e-12` gates. Its final checkpoint round-trips exactly. No extra interval is permitted at the ten-year clock limit: continuation is explicitly not applicable, not a successful eleventh-year step.

These fixed samples do not establish arbitrary-seed/parameter stability, physical calibration, realistic climate, or cross-platform bitwise agreement. Transit/vapor precision, weather, groundwater, thermodynamic freezing, dynamic lake ownership, and geography feedback remain separate work.

## Desktop and protocol 13

Choose **Initialize precise upslope** before initialization. The default button still selects version 3; the earlier upslope button selects version 4. Regenerate or open a compatible checkpoint to change mode. Playback, budgets, eight layers, inspection, both projections, and save/load use the same native version-7 state. This does not improve scientific calibration of its empirical climate.

Protocol 13 uses `preciseMoisture` and `preciseMoistureCheckpoint`. Display stays eighteen field-major Float64 arrays: regional mass fields are rounded leading values, not complete state. Native budgets include low components; full JSON preserves them. Geometry is not resent/changed. Protocols 11/12, initial-world fingerprints, 32 MiB display and 64 MiB checkpoint bounds, original-decimal-token restoration, and candidate acceptance/cancellation remain intact. Unsupported headless schemas 5/6 reject instead of masquerading as version 7.

Tests cover independent integer/dyadic oracles, small credits, signed tails, capacity/depletion, cold/disabled processes, invalid pins/components, nonzero-low and signed-zero replay, batching, cross-mode rejection, and atomic failures. A 40,962-region one-hour checkpoint is **15,847,167 bytes** for the test recipe; restore/another hour matches complete uninterrupted state. This is short fine-grid persistence evidence, not long-run level-6 qualification. Real Electron tests cover mode selection, map/globe layers, full file save/load, and exact next-hour checkpoint continuation. Old model/desktop regressions remain applicable; previous performance measurements do not benchmark version 7.

Final verification passes all native targets, 72 Node/adapter tests, the real Electron suite, Rust formatting, and Clippy with warnings denied. The normally ignored release annual surface test was also run explicitly and passes; its ignored status in the default suite is not used as qualification evidence.

## Reproduction and next work

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_precision_report
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_precision_report -- --long-refined
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_precision_report -- --long-refined --surface-only
cargo run --release --locked --manifest-path native/Cargo.toml --example terminal_ledger_probe
cargo test --locked --manifest-path native/Cargo.toml --test surface_precision --test terminal_precision
cargo test --release --locked --manifest-path native/Cargo.toml --test surface_precision -- --ignored
npm test
npm run test:desktop
```

The `--surface-only` long control selects version 6 and exits nonzero after printing its failure. Qualification reports must not hide refusals behind short-run success. The terminal audit reports both attempts and the old provisional drift; successful audit-process exit is not successful version-6 qualification.

Next: broaden parameter controls and spatially paired terrain/rainfall/runoff diagnostics, measure sensitivity, and establish seasonal summaries before adding coherent weather or ecology feedback. Keep version 7 opt-in; closing this numerical gate does not complete planetary climate/hydrology.
