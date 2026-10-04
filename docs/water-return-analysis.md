# Connected reference-water ownership and frozen-demand controls

Implemented October 4, 2026 as `water-return-analysis-1`, a read-only native diagnostic for seasonal models 3–7. This does not move water, select a new hydrology default, or implement shared-body evaporation. [Seasonal preparation](seasonal-preparation.md) remains unresolved.

## Question and scope

The decade preparation runs conserve water while liquid decreases and terminal inventories grow. Their permanently cold region count is zero. This does not tell us whether terminal water mainly accumulates in connected reference oceans/lakes or in closed dry sinks, nor whether local donor availability limits potential evaporation.

Current routing retains arrivals at their actual contact region. Terminal evaporation uses only that region's atmospheric demand and stock. Other regions in the same reference body cannot withdraw this water. Closed dry terminals have the same local-footprint approximation, but must not be assigned to an ocean just to return their water.

Reservoir evaporation depends on exposed water area, not stored volume alone: HEC-HMS multiplies interval evaporation depth by current reservoir surface area and requires elevation-area storage data. This motivates checking exposure/ownership; it does not validate our atmosphere-demand law, 300-second probe, fixed footprints, or perfect-sharing approximation. [HEC-HMS Technical Reference Manual: Evaporation](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/reservoir-modeling/reservoir-modeling-concepts-and-equations/evaporation).

## Immutable ownership

The native model retains a copy of the existing generated reference-water body labels. These are immutable connectivity metadata, not another stock. They use O(N) additional u32 storage (four bytes per region, excluding vector overhead); they are not transported to the renderer or added to checkpoints. Recipe restoration regenerates the same labels. No physical process reads them in models 3–7.

`water_return::Audit::capture` validates the native state and partitions every region into one of:

- Its existing connected reference-water body, with one record per positive body ID.
- An initially dry self-receiving endpoint: a closed dry terminal.
- Other initially dry land.

Reports include physical area, region count, all six exclusive stocks with signed low parts, and cumulative terminal delivery/evaporation and liquid/soil evaporation. Flow columns are cumulative approximate binary64 diagnostics, not independent stocks or annual integrals. They can exceed initial water after repeated circulation.

Positive-terminal-stock support area and the largest terminal stock's region/mass locate concentration. Positive support is not a modeled lake surface, a useful evaporation footprint, or evidence that evaporation is presently possible. Zero/empty support has a null witness, not an invented region.

## Frozen liquid-only probe

At each accepted annual boundary, the report evaluates twelve **independent** 300-second probes, one for each operational temperature/capacity month. The same endpoint vapor and liquid/terminal stocks are held fixed for every probe. These are not successive steps, an annual mean, actual weather, or a new year of simulation.

For each body and month:

```text
D_i = T_i > 0 and evaporation enabled
    ? max(capacity_i − vapor_i, 0) × (1 − exp(−probe_seconds / response_seconds))
    : 0

E_local ≈ Σ min(D_i, terminal_i + liquid_i)
E_perfect_sharing_cap ≈ min(Σ D_i, Σ(terminal_i + liquid_i))
additional_sharing_cap = E_perfect_sharing_cap − E_local
```

Local evaluation copies the existing compensated donor type, withdraws terminal first, then liquid, and respects negative-low conservative floors. It never debits authoritative state. Even for ordinary-stock models, a normalized zero low is used. The formula above describes the real-arithmetic comparison; finite arithmetic and donor floors are preserved in the recorded local result.

The perfect-sharing cap uses total liquid/terminal mass **within one existing body**, including cold-cell liquid. It never borrows another body's or a dry terminal's water. Summing represented components produces an approximate binary64 bound, not a directed-rounded conservative grant. No distribution, transfer ledger, mixing timescale, or validated checkpoint is constructed from it. Its signed difference is not clipped; tiny negative roundoff differences remain visible.

Precipitation, snowmelt, soil extraction, atmospheric transport, altered vapor after evaporation, latent heat, and subsequent feedback are omitted. Consequently this is not the complete current exchange operator or a forecast of a rainfall gain. In particular, current warm snowmelt could provide water omitted from this probe. The probe isolates liquid-donor location under held demand; a positive difference shows an instantaneous allocation restriction, not its share of the decade precipitation decline.

Only initially wet bodies receive these probes. Closed dry terminals retain ownership reports but are not evaluated as fully exposed reference-water bodies. Inputs outside 1–3,600 seconds, foreign/native-invalid states, incompatible models, inconsistent masks, or nonfinite reductions reject. Future physical laws require a separate interpretation review.

## Directed controls and verification

A constructed local-operator control has 30 kg of liquid/terminal water and three warm locations demanding 10 kg each. Concentrating the water in one location gives 10 kg locally versus a 30 kg perfect-sharing cap. Distributing the same 30 kg across all three gives 30 kg locally and the same cap. This identifies localization alone under held forcing, not a generated-world intervention.

Other controls verify finite body mass, zero-temperature demand, exclusion of outside-body mass, region relabeling, terminal priority, negative low tails, and malformed donors. These synthetic operator inputs are explicitly not accepted simulation checkpoints.

Integration tests independently sum actual body memberships, areas, all six stocks, and terminal deliveries; compare observed/unobserved complete advancement; restore identical audits and exact continuation; and cover empty/dry/fully wet worlds, disabled evaporation, invalid durations, and foreign-state rejection. A bounded structural coverage-fixture search also checks separate caps in genuinely disconnected generated water bodies; it is not a climate-success ensemble.

## Reproduce

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib water_return
cargo test --locked --manifest-path native/Cargo.toml --test water_return
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_preparation_report -- --smoke --return-audit
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_preparation_report -- --return-audit
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_preparation_report -- --refined --return-audit
```

Without `--return-audit`, the earlier preparation report version/shape remain unchanged. With it, report version 2 adds annual ownership audits, the separate diagnostic version, flow column labels, and explicit probe limitations. Physical model/schema/settings and preparation-analysis version remain unchanged.

## Measured ownership and allocation restriction

The [retained validation record](data/water-return-validation.json) contains eight 900-second and two 450-second decade runs, all hundred annual ownership audits, configurations, final budgets, failures, and explicit frozen-probe columns. Removing only the added audit field from each native case reproduces the corresponding case in the earlier preparation record **exactly**, including all annual assessments, budgets, settings, and replay flags. This is direct same-build parity, not an assumption that diagnostics are harmless.

All ten runs pass the existing numerical checks and checkpoint round trips. Each has one connected reference-water body. Year-ten 900-second results below are percentages of initial total mobile water for the two terminal-stock columns; local supply is the minimum–maximum fraction of frozen liquid demand satisfied across the twelve probes.

| Seed / radius / active depth | Reference-body terminal stock | Closed-dry terminal stock | Local frozen supply / demand |
| --- | ---: | ---: | ---: |
| reference / 1,000 km / 1 m | 62.51% | 0% | 9.48–16.92% |
| coast / 1,000 km / 1 m | 57.73% | < 0.000001% | 10.20–26.44% |
| interior / 1,000 km / 1 m | 20.36% | 0% | 8.01–17.96% |
| reference / 6,371 km / 1 m | 2.57% | 4.20% | 8.08–37.26% |
| coast / 6,371 km / 1 m | 5.25% | 0.00245% | 6.51–48.14% |
| interior / 6,371 km / 1 m | 5.34% | 0.662% | 3.66–25.23% |
| reference / 1,000 km / 10 m | 15.83% | 0% | 54.85–71.10% |
| reference / 6,371 km / 10 m | 0.784% | 0.684% | approximately 100% |

In every year-ten probe, that body's total liquid/terminal mass is sufficient for its total frozen demand; its perfect-sharing cap equals demand. Most cases nevertheless cannot satisfy local demand. In the 1,000 km, 1 m reference, terminal water occupies only 17.32% of reference-water area at year ten, while holding 62.51% of initial mobile mass. This establishes a within-body liquid-availability restriction under held forcing; lack of total body water is not the limiting factor in those probes.

The refined 1 m reference still has 62.48% in reference-body terminal water and satisfies only 8.66–16.36% of frozen demand locally. The 10 m reference supplies 54.84–71.09%. The restriction survives the halving, but local probe magnitudes are not declared converged. These are low-resolution fixed cases, not arbitrary-seed or spatial-convergence evidence.

Two qualifications matter for implementation. First, the Earth-radius reference retains 4.20% of initial mass in closed dry terminals at 1 m; distributing only reference-body water cannot release it. Second, the 10 m Earth-radius reference satisfies almost all frozen liquid demand but still fails the preceding annual stock-stationarity check. Shared-body availability is therefore not a demonstrated universal cure for nonperiodicity. Different radii also generate different upstream geography; this table is not a geographically paired radius intervention.

The full `npm test` run passes native targets and 72 Node/adapter tests. Four new module controls and five final targeted integration tests pass, as do formatting and Clippy with warnings denied. No new desktop feature or GUI qualification is claimed.

## Selected next candidate and integration contract

**Proposed, not implemented:** test a finite connected-reference-body liquid inventory with explicit body-level availability. This is the next bounded model candidate because the measured restriction occurs within already connected water, not because a particular rainfall total is desired. Do not globally share water across separate bodies or use the full immutable deep-water inventory as an unrecorded source.

Required contracts before integration:

1. Every wet-region kilogram has one authoritative owner. A body-owned inventory replaces, rather than duplicates, its local liquid/terminal ownership. Snow, soil, vapor, transit, and closed-dry terminal stocks remain distinct.
2. Preserve actual runoff contact regions and regional atmospheric transfers as provenance. Validate body stock changes against summed rain/melt, actual routed arrivals, and actual evaporation; regional diagnostics cannot silently become independently spendable duplicate stocks.
3. Evaluate regional atmospheric demands before allocating a finite body donor. Allocation must preserve temperature/disabled-process gates, avoid region-ID priority, and record actual conservative grants. The diagnostic sharing cap is not an allocation implementation.
4. Explicitly choose fast body equilibration versus finite internal redistribution. Fast equilibration would be a new coarse physical assumption, not an implementation of currents or proof that internal transport time is negligible. Finite redistribution needs a declared timescale/geometry rule and refinement controls; no coefficient is yet selected.
5. Pin the new physical model/schema and preserve complete donor components, hidden accumulators, settings, and exact continuation. Old models and checkpoints must neither migrate nor acquire pooling implicitly. Display fields must distinguish derived body distribution from authoritative per-region stock.
6. Compare otherwise identical trajectories against version 7, including the 10 m Earth-radius no-local-shortage control and closed-dry terminals. Check local/body/global ledgers, depletion, cold/disabled gates, body isolation, relabeling, temporal refinement, save/load, and unsupported-input atomicity before desktop promotion or climate-preparation claims.

This selects the next mechanism to implement and test, not a readiness declaration or a reason to broaden the clock, mask slow stores, or retune rainfall. Closed-lake surface/storage/spill and grounded calibration remain later gates.

Subsequent implementation: [finite connected-reference-water model 8](reference-water-pool.md) selects fast common availability as an explicit opt-in coarse hypothesis. It adds one finite compensated liquid owner per existing body, conservative regional grants, body ledgers, and schema-8 replay. The diagnostic and preceding records above still describe models 3–7; they reject model 8 rather than applying obsolete local-owner assumptions. No desktop/default promotion or seasonal-preparation qualification follows from this implementation.
