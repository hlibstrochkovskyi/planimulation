# Bounded shared-face planar evolution

October 12, 2026. This follows [frozen shared-face verification](surface-shared-face.md). The private native laboratory now performs consecutive explicit steps, rebuilding all interior fluxes from the accepted stocks at every old time. This is not a spherical operator, selected seasonal model, persistence format or desktop feature.

## State and transaction

Each owner retains the existing normalized high/low stock components between steps. A rounded projection of their sum divided by width supplies the moment reconstruction; that observation never replaces the physical stock. The old mean-initialized step delegates to the owned-stock step. Both old components enter the local and global ledger. The complete old-stock outgoing budget is checked before incoming credits, and requests are never resized.

Boundary input/output counters also retain normalized high/low components. Each of the two exterior `q*dt` amounts is credited separately, not through their rounded combined sum. Proposed owners, counters and accepted-step count commit together only after stock and cumulative ledger validation. Reconstruction, transfer, counter or ledger refusal preserves the last accepted state. The laboratory arithmetic bound remains `256*epsilon`; native audit tolerances are unchanged.

An independent integer oracle checks seven consecutive one-unit paired transfers at stocks `2^54`, including retained low components. The floating Kahan ledger observer can report cancellation roundoff even when that integer oracle is exact; it is not guaranteed to report zero. Directed tests also cover negative-low donor availability, malformed components, overflow, early stencil refusal and late boundary-counter refusal. In-memory checkpoint cloning reproduces one subsequent step bit-for-bit; no serialized save/restore support is claimed.

## Fixed experiment and observers

The domain is `[-100,100] m`, roughness `.04`, cap `100 m²/s`, wave speed `.1 m/s`, and final time `.1 s`. The unchanged independent analytic primitive supplies initial integrated means. Exact instantaneous exterior fluxes are prescribed at each old time. This is explicit Euler forcing, not an exact time-integrated boundary input. No exact slope, speed, front or future state reaches interior flux construction.

The original cohort contains 32 trials: widths `1/.5 m`, physical initial fronts `0/.4 m`, directions `+/-`, time steps `.001/.0005 s`, and two separately reported methods: pressure penalty 1 and the stock-depth baseline. Physical initial fronts stay fixed across grid refinement; fixed cell phases would compare different problems. After observing this cohort, a separate 16-trial follow-up uses front `.995 m` to cross the common face at normal coordinate `1 m`, with exact onset near `.05 s`. Neither operator nor coefficient is changed for that follow-up.

Every accepted step updates actual stocks. Recorded observations include initial/first/final states and common times `.049/.05/.051 s`, final high/low arrays, boundary counters, maximum local/cumulative ledger residuals, all-cell RMS/maximum mean error and volume in initially dry owners. All-cell reconstruction discovers support and `1 mm / 1 cm / 5 cm` roots before truth comparison. Nonunique, missing or incomplete observations have null contour error. Refusals retain their attempted step, reason and last accepted time.

Spatial and temporal comparisons require both trials to finish at the same requested end time. Incomplete comparisons have null ratios. First representable dry arrival is recorded without a film or detection threshold; tiny numerical traces can trigger it, so it must be interpreted alongside actual volume and exact onset, not as an independently accurate front measurement.

The related [Manorost–Bastian paper, sections 3.1–3.2](https://arxiv.org/html/2512.16525v1) uses implicit formulations and its own friction regularization/cut-cell machinery. Its nonnegativity result does not prove stability or convergence of this explicit pressure hypothesis. The original capped physical law is unchanged; donor-budget safety is not a diffusion stability certificate.

## Results and retained limitations

All 24 pressure trials finish their 100/200 rebuilding steps: 3,600 accepted steps in total. Stocks remain normalized/nonnegative, and local/cumulative ledgers stay within the unchanged arithmetic bound. All four final contours are uniquely observed. Temporal RMS ratios are approximately `2.000–2.090`, and arrival/contour errors also improve in the positive-arrival controls. Two step sizes in one short manufactured family do not establish a general temporal order theorem.

Aligned-front arrival at final time, in square metres per unit breadth:

| Width, m | Step, s | Actual initially dry volume | Exact volume |
| ---: | ---: | ---: | ---: |
| 1 | .001 | 1.222658e-5 | 1.231069e-5 |
| 1 | .0005 | 1.226927e-5 | 1.231069e-5 |
| .5 | .001 | 1.223038e-5 | 1.231069e-5 |
| .5 | .0005 | 1.227114e-5 | 1.231069e-5 |

The delayed `.995` crossing has no represented dry-owner volume at or before `.05 s`. First positive arrival is at `.052 s` for the coarse step and `.051 s` for the fine step: a retained two-step delay, not exact crossing timing. At `.051 s`, coarse-step arrival is still zero despite exact volume `1.71056e-8 m²`; the fine step produces about `6.45e-9 m²`. At final time, width-1 arrivals are `4.500986e-6 / 4.538710e-6 m²`, against exact `4.573403e-6 m²`. Width-.5 arrivals are `4.503118e-6 / 4.539722e-6 m²`. Maximum final support error in this follow-up is `5.583e-5 m`, greater than the original cohort's maximum `4.835e-5 m`; its separate regression bound does not relax the original controls.

For initial front `.4`, the exact front never reaches an initially dry owner's face during this time window, and both actual and exact dry arrival remain zero. That is not a successful new-owner wetting test. The delayed follow-up supplies that distinct check.

Spatial RMS does **not** improve at fixed time step: pressure coarse/fine RMS ratios are about `.735–.748`, so halving width increases the measured RMS. Some contour/arrival comparisons improve while others worsen. Keep all comparisons; no spatial convergence or full moving-front qualification follows. Temporal and spatial errors near a singular support must be separated before interpreting a grid order.

All 24 stock baselines refuse attempted step 6 after five accepted steps, at `.005/.0025 s`. Successively tiny downstream tails produce an unrepresentable positive flux branch under the existing strict arithmetic contract. This is not donor overdraw, negative stock, or proof that first-order finite volume cannot work. Their terminal errors must not be compared directly with pressure results at `.1 s`. Premature baseline dry arrival, multiple roots and unresolved observation cells remain recorded rather than silently corrected.

## Decision and next gate

The pressure hypothesis passes this bounded rebuilding/ownership check and demonstrates delayed new-owner wetting with a measured temporal lag. It remains unselected. Total exact front movement is only `.01 m`: no sustained propagation across full cells, general extrema/plateaus, variable bed, barriers, triangle/spherical reconstruction, seasonal coupling or runtime scaling is qualified.

Next separate temporal error from spatial error using matched time refinement tied to grid width, then a bounded full-cell crossing with explicit stability/cost limits. Diagnose the stock baseline's arithmetic tail separately if retaining it as a longer comparison; do not insert a film, clamp or fallback. Keep the expensive spherical witness deferred until planar propagation and representation gates justify it.

## Reproduction and validation

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_paired_evolution_report -- artifacts/new-paired-evolution.json
cargo test --locked --manifest-path native/Cargo.toml --lib paired_evolution
cargo test --locked --manifest-path native/Cargo.toml --lib consecutive_owned
```

The CLI requires one new output path, refuses overwrite/extra arguments, and computes the bounded report before file creation. `surface-paired-evolution-report-1` is a laboratory version, not a product/model pin. The [validation record](data/surface-paired-evolution-validation.json) retains every case's completion/refusal, scalar observations, matched refinement and fingerprints. Full stock arrays remain in the ignored reports. Final test/report-repeat evidence is recorded separately; no GUI qualification or new evolving spherical run is claimed.

Final source passes all 156 optimized library tests, twelve regional-soil and five regional-surface integration tests, the four directed rebuilding tests and the consecutive-owned test, strict all-target Clippy, typecheck, formatting and whitespace checks. The release native binary is rebuilt and all eighteen Node files pass (88 tests). Three CLI refusal checks preserve the existing report and leave invalid extra-argument output absent. The complete `npm test`/all-native-target suite is not rerun in this increment; older full-suite evidence is not relabelled as final-source evidence.

The complete 1,509,746-byte report repeats byte-for-byte and matches optimized output, SHA-256 `8684c711e4009578b6e0c76df67a0d06123531eb6ba02c9cbf57da11296aae58`. Rebuilding the previous frozen shared-face report preserves its SHA-256 `74ef32915c5a922c79fd7b1300c522d028f63e653b7182a1ef3bf95604044016`. Maximum local/cumulative relative ledger residuals in the evolving cohort are `1.159e-16 / 2.183e-17`. Pre-existing renderer edits are untouched and excluded.
