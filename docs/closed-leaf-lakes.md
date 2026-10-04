# Closed-leaf lake surface and evaporation component

Implemented as the headless `closed-leaf-lake-1` component. It derives a minimum-basin water surface from a finite high/low liquid donor and evaluates a conservative evaporation interval. This stand-alone service does not itself advance seasonal evolution or introduce a desktop feature. Subsequent, separately versioned [model-9 coupling](coupled-leaf-lakes.md) is documented independently; the original model-8 component controls, defaults and clocks remain unchanged.

## Purpose and evidence boundary

The [body-aware annual assessment](body-seasonal-preparation.md) identified continued closed-dry terminal accumulation despite nearly constant rainfall. That identifies a changing owner, not automatically a bug or evidence that the lake needs a larger evaporation area. This component makes that geometry testable and supplies a reusable finite return operator before any coupled lake law is chosen.

Evaporation volume depends on depth loss and exposed surface area. [HEC-HMS's evaporation equations](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/reservoir-modeling/reservoir-modeling-concepts-and-equations/evaporation) motivate that separation. The implemented graph columns, frozen-footprint split, temperature gate and atmospheric-demand law remain project approximations, not HEC-HMS routing or calibrated lake physics.

## Exclusive geometry, not duplicated water

`closed_lake::Layout::from_world` uses the existing generated basin hierarchy and static drainage. Each initially dry self-receiving terminal must belong to a distinct minimum leaf, at its minimum bed, with no initially wet members. Exact-height minimum plateaus retain the static drainage's canonical endpoint. An incompatible/multiply assigned leaf rejects; no regional IDs are merged arbitrarily.

Only that leaf's exclusive region columns enter its lake. Slopes assigned to an ancestor, adjacent leaves and reference-water bodies are not borrowed. Total copied columns across these leaves are at most N; no ancestor-subtree duplication is introduced. Construction/index preparation takes O(N + K + Σ M log M) time, with O(N + K) working memory and O(Σ M) retained geometry for leaf sizes M and K basin branches. This is not a planet-scale dynamic solver benchmark.

The existing isolated `Reservoir` curve is reused as a **read-only storage index**, with zero index inventory. Its `ExternalCollector` boundary marks a finite geometric first connection only: this component never sends water to that collector. Parent storage cannot be used while neighboring leaves remain partly empty.

For local datum `z_min` and relative level d:

```text
V(d) = Σ A_i × max(d − (z_i − z_min), 0)
exposed(d) = {i : z_i − z_min < d}
A_exposed(d) = Σ A_i over exposed(d)
```

Invert the piecewise-linear curve from liquid mass / `1000 kg/m³`. Zero liquid has no level and no exposed area. A zero-depth shelf is **not** an evaporating surface; at the exact shelf height only lower columns are exposed. Whole-region columns give discontinuous area changes and no sub-grid shoreline; renderer interpolation must not replace this geometry.

Relative heights drive exposure, avoiding the subtraction of a rounded large absolute level. `absoluteLevelMeters` is an optional display coordinate: it is absent if its reconstructed relative height cannot satisfy the existing relative level scale. The relative surface can still be valid. The stock is never replaced by reconstructed volume, and reconstruction residuals remain visible.

The supplied high/low pair is authoritative and validated for normalization/nonnegativity. Geometry uses a rounded derived binary64 volume, not a new independently spendable stock. Finite first-connection capacity is also checked in mass units **before** kg→m³ conversion can conceal a positive low tail over capacity. `atSpillThreshold` requires an exact represented mass-capacity pair, not merely a rounded volume equality. Stock above the first connection rejects; this is not a spill event. Nonfinite/overflow/underflow or unresolvable geometry rejects without altering the supplied donor.

## Finite evaporation operator

`Lake::evaporate` accepts one donor by value, one interval-integrated potential demand in kg/m² per leaf region, local air temperature, and an enable flag. The caller is responsible for deriving demand and choosing an interval. There is no hidden atmospheric stock or standalone replay clock.

1. Derive the **before-interval** exposed footprint.
2. Request `area_i × potential_i` only at exposed regions with temperature > 0 and evaporation enabled.
3. Reuse model 8's conservative [common-fraction allocator](reference-water-pool.md), including signed donor floors and its owned arithmetic reserve.
4. Return actual regional grants, one remaining high/low donor, allocation residual, and the derived after-interval surface.

Each actual grant has one donor debit. The future caller must credit those grants to the respective atmospheric recipients exactly once; the operator does not perform that coupling. Invalid demand, donor, allocation or output geometry rejects with no caller mutation. There is no ocean borrowing, negative stock, final mass correction, last-recipient remainder, or reassignment of trapped water.

Exposure is frozen within this one operator interval. Drying across a shelf changes the **next** interval's footprint, not grants already made. This is an explicit split approximation requiring refinement when coupled; arbitrary interval partitions across shelves are not promised equivalent. A leaf can contract without splitting before its first connection. Parent drying/splitting, overflow, backpressure and neighboring lake merging are unsupported.

## Generated observations and frozen probes

`Layout::capture` validates a matching native model-8 state, then derives surfaces from its actual compensated closed terminal owners. Geometric refusal remains in each observation alongside the unchanged full donor; stock is not clipped. Models 3–7 and foreign recipes refuse.

`Layout::probe` evaluates an independent 1–3,600-second endpoint operator for one operational month. It uses that region's existing fixed-geography temperature/capacity and held endpoint vapor:

```text
potential_i = max(capacity_i − vapor_i, 0)
              × (1 − exp(−seconds / evaporation_response)) / area_i
```

The report uses twelve independent 300-second probes on the **same** annual state. They are not successive simulation intervals, an annual evaporation estimate, or a forecast. No precipitation, melt, soil demand, transport, lake-temperature response, latent heat, new wet mask or dynamic river-routing feedback is applied. Source temperatures remain reference-land normals even when a derived lake covers a region.

`closed_lake_report` retains annual physical budgets and flows, all closed-lake surface/refusal records, actual annual terminal delivery/evaporation, catchment area, monthly terminal temperatures, frozen grants, unchanged-checkpoint checks, and native replay. Physical-run success is separate from per-surface/per-probe refusal. It refuses to overwrite existing output evidence. Default duration is one year; `--decade` selects the existing ten-year bound and `--refined` halves the coupling ceiling. No clock reset occurs.

## Static first-spill recipients

The subsequent `closed-leaf-spill-connections-1` API, `spill::first_connections`, connects each leaf to **all** direct receiving-branch/plateau alternatives in the existing [spill incidence graph](spill-connections.md). It rebuilds and checks the hierarchy against the generated lake geometry. A canonical sill passage enters the selected lower receiving subtree; a separately recorded static downhill path follows actual neighboring receivers to either a reference body's **actual contact region** or another closed terminal.

The path is a static certificate, not flowing water. It validates graph bounds, acyclicity/outlet consistency, neighbor edges, decreasing bed/flat rank, and no return to the source. It never treats the lowest-ID wet-body display slot as the arrival location or substitutes a nested receiver's lowest minimum for its actual entry. Multiple candidates stay visible; `hasUniqueRoute` means one retained passage/terminal certificate, not a hydraulic permission or chosen allocation policy. The root has no external route. Building this inspection service includes a fresh basin/incidence analysis; passage queries retain the existing O(N) scratch cost and are not a per-tick routing benchmark.

The [retained static report](data/closed-leaf-spill-validation.json) records the same reference recipe:

- Region 73 reaches its first connection at relative **34.632508 m**. Its sole certificate is `73 → 69 (sill) → 20 → 66`, ending at actual contact 66 in reference body 1.
- Region 91 first connects at relative **81.860177 m**. Its sole certificate is `91 → 21 (sill) → 73`: it first reaches the other closed terminal, not the ocean. Its two-column capacity divided only by the minimum region's area is **not** this spill height.

Four additional tests check real graph paths/owners on three generated seeds, a three-way chain that must enter its unfilled middle leaf, unresolved two-way alternatives, nested entry, a noncanonical wet-body contact, a closed root, and rejected stale/corrupt geometry. The static example is `cargo run --release --locked --manifest-path native/Cargo.toml --example closed_lake_connections`. This identifies first recipients; it does not inspect their active water levels, allow transit through full receivers, debit or credit a stock, select discharge, merge reservoirs, or add a simulation checkpoint. Those remain coupling work.

## Controls and next coupling gate

The [retained record](data/closed-leaf-lake-validation.json) contains matched 900/450-second decade controls for the subdivision-2 Earth-radius reference with 1 m initial active depth. All 20 annual physical budgets and 40 annual global P/E scalars exactly reproduce the preceding body-preparation record. Every annual observation leaves the complete serialized checkpoint unchanged, and both full native JSON round trips are exact. Next-hour continuation at the ten-year bound is not applicable.

Both closed terminals remain exposed only at their own minimum region in every recorded year. The dominant region-73 leaf has **one column**: expanding a footprint within this discretization cannot happen before its first connection. At year ten it holds only **23.53%** of that connection capacity; its relative depth is approximately **8.149 m**, versus **34.633 m** at connection. Its static upstream dry catchment is 2.814 times the terminal area. Region 91's leaf has two columns, but its next column remains dry at both step sizes.

| Terminal / coupling | Year-ten depth | Actual annual delivery | Actual annual evaporation |
| --- | ---: | ---: | ---: |
| 73 / 900 s | 8.148947 m | 903.917548 mm | 45.632605 mm |
| 73 / 450 s | 8.148825 m | 903.846984 mm | 45.584582 mm |
| 91 / 900 s | 0.016262 m | 205.675294 mm | 205.675294 mm |
| 91 / 450 s | 0.016318 m | 205.756079 mm | 205.756079 mm |

Actual flows difference committed cumulative terminal transfers, normalized by the terminal region's area. They are not frozen-probe totals. Region 73 is warm in all twelve operational months (approximately 19.88–22.41 °C); a permanent cold lock is not the explanation. Its influx exceeds evaporation by approximately 858 mm in year ten. Region 91's late annual delivery/evaporation nearly balance. This local contrast would be lost by reporting only total closed-water stock.

All **40 derived surface observations** and **480 independent frozen probes** succeed. The largest derived-volume reconstruction residual is `0.00390625 m³`, without replacing mass; the maximum relative allocation residual is approximately `9.99e-17`. Full `npm test`, final six-unit/four-integration component controls, warnings-denied all-target clippy, formatting and diff checks pass. The ordinary suite retains its pre-existing ignored long release qualification; no new skips or GUI validation were introduced.

This measurement **does not establish that real lake filling is correct**. It does establish that changing from point-stock labeling to this existing leaf geometry alone cannot enlarge the exposed footprint in the retained interval. Artificially spreading region-73 water over its entire catchment, borrowing adjacent leaves, or declaring it over capacity would invent geometry. Continued filling below a sill need not be a conservation failure or something a stationary-climate policy may erase. Physical/thermal review and explicit first-spill ownership remain necessary; there is no justification here for increasing the clock or selecting a larger evaporation coefficient.

Unit controls independently specify weighted storage/exposed-area values, exact shelves and capacity tails; test finite/depleted/signed-low donors, warming/zero-temperature/disabled gates, contraction, dry and closed-root cases, datum/area scaling, dyadic recipient reordering, invalid forcing and underflow.

Generated integration controls independently reconstruct leaf prisms and areas, check exclusive dry ownership on three seeds, compare complete observed/unobserved 40-day states, verify frozen forcing from the separate temperature API, reject foreign/version/invalid probe inputs, and preserve exact JSON continuation. These control declared accounting and geometry, not physical calibration.

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib closed_lake
cargo test --locked --manifest-path native/Cargo.toml --test closed_lake
cargo run --release --locked --manifest-path native/Cargo.toml --example closed_lake_report -- --decade --output artifacts/new-closed-lake-report.json
cargo run --release --locked --manifest-path native/Cargo.toml --example closed_lake_report -- --decade --refined --output artifacts/new-closed-lake-refined-report.json
npm test
```

Future coupled lake ownership must define precipitation/melt interception, existing land liquid/soil/snow beneath newly exposed water, atmospheric transfer provenance, thermal-mask assumptions, active spill-receiver/backpressure and exact transfer ownership, and checkpoint/version isolation. A geometric threshold is not an unlimited external drain. Do not add artificial return or change stationarity policy solely to make the decade case pass. A persistent filling transient can be admissible in an evolving world; its physical plausibility and preparedness interpretation still need review.

Subsequent follow-up: [bounded coupled leaf-lake exchange](coupled-leaf-lakes.md) now implements a separately pinned headless model/schema 9 below the first connection, with liquid interception, preserved inactive submerged soil, actual regional evaporation and independent lake identities. This stand-alone component remains a read-only surface/probe service; observations additionally accept matching model-9 states. First-spill/backpressure/merge remains unimplemented, and its threshold refuses rather than draining implicitly.
