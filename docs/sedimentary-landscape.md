# Delivered-material dry landscape preparation

October 14, 2026. Native diagnostic kernels: `sediment-delivery-1`,
`minimum-leaf-aggradation-1` and `sedimentary-landscape-preparation-1`.
These are synthetic prehistory components, not a selected desktop generator,
dynamic lake solver, calibrated sediment model or completed R3 milestone.
The frozen uplift/incision model, production recipes and old fingerprints remain
unchanged. Product principles still come from [DESIGN.md](../DESIGN.md).

## Process choice and approximations

The preceding [uplift preparation](landscape-evolution.md) removed all incised
material to an external reservoir. This candidate instead routes real accepted
erosion to the current physical terminal and retains finite dry stock. It tests
whether actual material supply can change depressions and subsequent drainage.
No material source means no depositional bed rise.

[Landlab's erosion/deposition component](https://landlab.csdms.io/generated/api/landlab.components.erosion_deposition.erosion_deposition.html)
distinguishes pits from mapped flooded depressions, with different trapping and
passing behavior. Its adaptive integration constrains upstream/downstream
flattening. [FastScape](https://fastscape.org/fastscapelib-fortran/) distinguishes
bedrock/sediment erodibility and continental/marine processes. These motivate
separate erosion, transport and deposition accounting; neither implementation
nor their full governing models is copied here.

Explicit candidate assumptions:

- The existing stream-power law supplies accepted solid-equivalent reference
  prisms under prescribed effective runoff. No hourly rainfall or water flux is
  advanced by this preparation.
- All material passes nonterminal downhill/flat links instantaneously. There is
  no finite carrying capacity, travel delay, grain size, porosity, compaction or
  suspended load. Complete terminal retention is an upper-bound trapping study.
- Deposited sediment and inferred underlying bedrock have equal effective
  erodibility in this first candidate. Active sediment is removed first when a
  later incision step lowers the same region.
- Original wet beds and their water-level base stay fixed. Their external solid
  collectors distinguish the designated main ocean from other initial water
  bodies, with a per-body catalog. Inland collection is not marine export.
  No underwater deposition or evolving lake geometry is implied.
- Prescribed uplift remains kinematic displacement bookkeeping, not new rock
  mass or mantle extraction. All accounting uses reference-sphere areas.

## Owned step sequence

Initial active sediment and mobile pending material are zero. Each step:

1. Reuse the unchanged one-step uplift/incision kernel and accepted removals.
2. Reconstruct its exact raised, pre-incision bed and actual downhill DAG.
3. Remove existing active sediment first; distinguish gross removal from its
   remobilized fraction. Underlying material supplies the rest.
4. Route accepted removal plus old pending volume on that DAG. Pending is keyed
   by stable physical region ID, never changing basin/node IDs. Its repeated
   transport is not booked as new erosion.
5. Transfer wet arrivals to their explicit external collectors. Dry arrivals
   remain finite pending stock at actual dry self-receivers.
6. Rebuild the basin hierarchy on the post-incision bed. Deposit only from stock
   on a current minimum plateau into that leaf's exclusive physical footprint.
   Raise its level by at most `aggradation_rate * dt`, no higher than its first
   sibling spill, and no more than the delivered material permits.
7. Debit real stock, add active/cumulative deposits and keep excess pending.
   Rebuild the actual bed DAG. Any new connection must follow changed heights;
   a minimax/spill witness is never installed as a physical receiver.

The first sibling connection is not an ocean escape. Further transfer happens
only when the next actual DAG allows it. A root without finite spill retains
its inventory instead of receiving an invented outlet or deposition threshold.
Stock on nonminimum cells remains available for subsequent downhill rerouting.

For leaf cells with bed `h[i]` and reference area `a[i]`, deposited prism at
candidate level `z` is `sum(a[i] * max(z - h[i], 0))`. A deterministic feasible
lower-bound inversion chooses a level within the rate/spill/material bounds.
Footprints are exclusive, so sibling deposits do not double-charge nested
capacities. This is a stated aggradation surrogate, not a settling equation.

## Ledgers and numerical observations

The owner independently reconstructs:

```text
gross_erosion = gross_deposition + wet_exports + final_pending
net_bed_prism = accepted_uplift - gross_erosion + gross_deposition
surviving_sediment = gross_deposition - remobilized_sediment
```

Gross erosion/deposition can include the same material on different steps;
they are not final inventories. Each ledger uses its own gross exchange scale
and rejects relative residual above `1e-11`. Large uplift cannot conceal an error
in a small active sediment stock. Per-body exports are checked against split
aggregate collectors. No height or material stock is repaired to close a ledger.

Deposition separately reports accepted geometric prism and the actual floating
stock debit. A leaf proposal commits only if debit and geometry agree within
`1e-11` relative to that leaf's deposited prism. Otherwise that whole leaf keeps
its original bed and stock, and reports an unresolved proposal. Its candidate
volume is a numerical diagnostic, **not an inventory or accepted transfer**;
repeated skipped proposals can count the same potential deposit several times.
No partial donor decrement survives a skipped leaf.

This matters when a micrometre-scale first spill is paired with enormous pending
stock. A retained first-light level-4 case requested about 5.253 million m³ from
18.189 trillion m³; subtraction rounded the debit by about 0.000812 m³. The
prism-relative discrepancy exceeded tolerance. Retaining the proposal without
changing bed/stock allows a later step to resolve it, rather than relaxing the
accounting threshold or adding an artificial epsilon height.

Positive prism product underflow, nonfinite fields and malformed topology still
refuse. Unresolved uplift/incision/deposition, source assembly rounding and wholly
lost positive source additions are recorded per step. This is bounded floating
arithmetic, not exact arbitrary-range stock conservation. Input arrays remain
unchanged on failure. Bounds: at most one million regions, 128 steps and 64
million region-steps; these are workload limits, not a runtime guarantee or new
supported desktop resolution.

## Controlled actual-bed review

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example sedimentary_landscape -- artifacts/NEW_DIRECTORY 6

cargo run --locked --release --manifest-path native/Cargo.toml \
  --example sedimentary_landscape -- artifacts/NEW_DIRECTORY 6 first-light --steps=64
```

An optional seed selects one case; otherwise the same twenty ordinary seeds and
fragmented/broad/low-relief controls are retained. Subdivision 4–6 only. Compare
original `multiscale-geography-2`, frozen all-material-export preparation and new
terminal-retention preparation, with identical initial source and water stock.
Candidate settings: `K=1e-6/year`, `H=2000 m`, `T=800000 years`, 32 steps and an
aggradation cap of `0.001 m/year` (25 m per default step). Step controls 16/32/64
retain total time. These are explicit initial-condition choices, not Earth
calibration.

New directories only. Full initial recipe/fingerprint/native arrays, source,
raw prepared beds/inventories/history, independent ledgers and physical metrics
are retained. `manifest.json` is written last; partial runs do not count as
completed reviews. Final diagnostic water is fitted once to the original resolved
volume; it is not water advanced during prehistory and not a resumable prepared
World. Timings are observations, outside physical identity.

Three-panel atlases use actual prepared beds, one shared height/depth palette,
static receiver overlays and genuine dry terminal markers. Lines are filtered by
contributing area, not simulated river discharge. Colors are not biomes. Such a
comparison can show model consequences; neither invariant tests nor improved
closed-catchment metrics alone establish plausible Earth-like geography.

### Retained twenty-world comparison

All twenty ordinary seeds and three controls complete at 40,962 samples. Retained
manifest: `artifacts/sedimentary-review-final-l6/manifest.json`. Its 69 complete
initial/old/new preparation records reproduce the preceding complete run exactly
for raw physical fields, arrays, histories, candidate initial water and physical
diagnostics; only timing/observer-summary fields are excluded from that comparison.
The inspected first-light atlas and twenty-case contact sheet are retained at
`artifacts/sedimentary-review-first-light.png` and
`artifacts/sedimentary-review-contact.png`.

Medians across the twenty ordinary worlds, same H=2000 source:

| Diagnostic | Previous all-material export | Delivered-material candidate |
| --- | ---: | ---: |
| Static closed share of dry land | 23.6417% | 10.9787% |
| Closed dry minima | 55 | 10.5 |
| Dry height p95 | 1456.547 m | 1456.901 m |
| Additional potential water-escape storage | 437614 km³ | 151883 km³ |

Every ordinary candidate has a lower closed share and lower potential storage
than its export control. Candidate closed share ranges from 3.11% to 20.70%.
Maximum coupled ledger residual across all 23 cases is `3.462e-16`; maximum
independently represented water-stock residual is `2.914e-14`. These are static
bed/catchment diagnostics, not simulated river discharge or permanent endorheic
coverage. No unfavorable seed is removed.

Median cumulative deposit is 224310 km³, surviving active deposit 109933 km³,
remobilized sediment 112230 km³ and remaining mobile pending stock 2143694 km³.
Medians of separate quantities do not sum into a ledger. Large pending inventory
is real retained material in this surrogate, not automatically converted to soil
or an unsupported parent/ocean transfer. Water inventory is separate.

First-light changes from 55 to 15 closed minima and from 26.24% to 12.33% closed
dry area. Its 202 unresolved leaf-step proposals retain stock rather than failing
the world; repeated candidate-volume observations are not missing material.
There are zero wholly lost source additions. Measured source-assembly absolute
rounding totals 0.1193 m³ across all its steps; maximum accepted deposition
inventory residual is `1.427e-13`. Broad continent/upland morphology still looks
broad in the actual-bed atlas. This mechanism improves drainage, not coast shape
or resolved subregional relief by itself.

### Temporal qualification remains open

First-light with equal total time, 16/32/64 steps:

| Diagnostic | 16 | 32 | 64 |
| --- | ---: | ---: | ---: |
| Dry height p95 | 1480.320 m | 1479.333 m | 1479.389 m |
| Static closed dry area | 7.0042% | 12.3310% | 14.8024% |
| Closed minima | 7 | 15 | 17 |
| Remaining mobile stock | 1102925 km³ | 1867033 km³ | 2473911 km³ |

16→32 and 32→64 unweighted per-region bed RMS differences are 7.91/4.56 m;
maximum differences are 395.66/200.75 m. Nearly stable p95 does not establish
converged transient routing/deposition. Retained preparation records are in
`artifacts/sedimentary-refinement{16,64}-l6`. The prototype is not promoted as
a calibrated or timestep-qualified desktop default. Future coupling must qualify
capture/spill timing and finer sampling instead of hiding these differences.

### Bounded finer sampling

The separate `sedimentary_scale` example composes the same initial structural
fields without constructing a dense World, runs the frozen export control and
the coupled candidate, then retains summary-only actual-bed diagnostics:

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example sedimentary_scale -- artifacts/NEW_REPORT.json first-light 8
```

Sampling levels 6–8 are explicit probe metadata. The reference Recipe remains
valid at level 6; it does not masquerade as a supported level-8 recipe. At level
6, both variants' physical geography/hypsometry/drainage/neighbor diagnostics
match the full World-based review exactly. Extra depression/minimax diagnostics
are omitted above level 6, not replaced by inferred depths. The preparation
itself still constructs its authoritative basin hierarchy for actual deposition.
Field checksums are noncryptographic diagnostics, not checkpoint identities.

Retained sequential first-light reports: `artifacts/sedimentary-scale-l{6,7,8}.json`.

| Samples | Export-control closed dry area | Candidate closed dry area | Candidate minima | Candidate p95 | Coupled preparation | Two-variant total | Peak process RSS |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 40962 | 26.24% | 12.33% | 15 | 1479.33 m | 2.290 s | 3.149 s | 37.22 MiB |
| 163842 | 39.56% | 21.85% | 38 | 1606.92 m | 7.833 s | 10.776 s | 129.41 MiB |
| 655362 | 51.53% | 35.82% | 154 | 1719.57 m | 28.025 s | 37.286 s | 502.86 MiB |

The dense repeat takes 32.135 s preparation / 42.215 s total and 501.47 MiB
peak RSS. Initial fields, source, both independently reconstructed physical
summaries, new per-step history summaries and bed field diagnostics repeat exactly
apart from declared timing/RSS observations. Summary output is about 41 KiB;
it does not contain a complete dense state. No dense twenty-seed ensemble or
full-state bitwise/checkpoint reproduction is implied by that summary repeat.

Coupled global ledger residuals remain below `2.735e-16`; independent pending,
active sediment and split collector reconstruction also passes its declared
tolerance. Candidate physical-bed diagnostics
are `f7ce8015`, `eec49ddc`, `1942a8ac` at sampling 6/7/8. The 655k run observes
3328 unresolved deposition leaf-step proposals and retains finite pending stock;
it does not convert those observations into forced outlets.

More samples remain feasible for **prehistory generation**, not yet for full
dynamic water/climate or interactive dense display. Cumulative process RSS
includes geometry, both sequential variants and allocator history, not isolated
candidate memory. The remaining increase in closed area with resolution is a
model limitation; density alone is not a geographic fix. Tiled display, explicit
coarse-climate coupling and local physical relief remain outstanding R3/R4 work.

## Verification scope

Delivery controls independently walk weighted DAGs and generated-sphere accepted
incision; distinguish inland/ocean collectors; reroute pending stock; reject
uphill/nonadjacent/cyclic/disconnected fields; and cover a 100,000-link iterative
flat DAG, representability, immutability and repeatability.

Deposition controls cover weighted/nested footprints, flat donors, rate/material
bounds, first sibling spill versus ocean escape, root retention, wet exclusion,
zero supply, large-area fractional debit, product underflow and transactional
unresolved proposals, including the observed micrometre-spill case.

Coupled controls demonstrate a known sink reaching its real first spill, the
changed DAG opening a physical outlet, subsequent rerouting of pending stock and
remobilization of deposited sediment. They also cover no-supply uplift, exact
zero-aggradation compatibility, finite closed-root stock, separate wet collectors,
all-wet/subprecision controls and four exact generated-world repeats with
independent per-region/global prisms. These narrower contracts do not certify
geological realism or satisfy full R3/R4 acceptance.

Full `npm test` passes 763 native tests across 134 targets and 96 Node tests,
with the same pre-existing ignored native test. This includes 12 delivery,
14 deposition, 10 coupled-preparation and five new review-example controls.
Strict all-target Clippy, formatting, typechecking and whitespace checks pass.
Six preparation CLI refusals and an existing dangling-symlink control preserve
old output and create no new result. Retained full log:
`artifacts/sedimentary-validation.log`. Owner renderer hashes remain unchanged
and their existing edits are excluded from this increment. No new packaged
desktop capability is claimed.

The subsequent sampling example receives three focused tests after that full
run: supported reference versus probe limits, independent actual-sphere stock
reconstruction and corrupt catalog/net refusal, and all-wet zero-scale/omitted
diagnostic controls. These are additional focused controls, not a newly claimed
full 766-test run. Strict all-target Clippy, formatting and whitespace pass.
Five sampling binary refusals and a dangling-symlink control preserve existing
output and create no result. The old/full World path is unchanged.
