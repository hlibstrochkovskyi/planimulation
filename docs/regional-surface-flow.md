# Regional surface-flow candidate

October 5, 2026 opt-in native candidate, numerically screened on the recorded cohort below; not physically calibrated or promoted to the default. A recorded seasonal year is now checked alongside genuinely active directed flow. A subsequent [explicit desktop view](regional-water-desktop.md) is implemented. These results do not complete general planetary hydrology.

## Implemented candidate path, not a silent migration

Models 9–14 retain their existing basin-owned, instantaneous and bounded contracts. The new opt-in `seasonal-moisture-15` candidate instead retains liquid in compensated **regional columns** and exchanges actual mass over shared physical faces. Basin hierarchy remains an analysis tool, not a state machine that must enumerate every possible nested merge. Simultaneous neighboring flows use the previous substep's depths; receivers do not spend new arrivals to compute the same substep's rates. Connectivity of wet columns can consequently change without transferring inherited lake ownership or allocating water to arbitrary branch IDs.

This is a change of physical approximation, not a numerical repair of the previous solver. It does not reproduce instantaneous Fill–Spill–Merge trajectories. The candidate must be checked independently and must not replace the default merely because its topology is less restrictive.

## Face closure and stability

[HEC-RAS's diffusive-wave reference](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) describes gravity/friction-dominated flow down the water-surface gradient and the near-flat explicit-step restriction. The present closure is **inspired by** that approximation; it is not HEC-RAS, a full shallow-water discretization or a calibrated river model.

For each face, bed height is piecewise constant in either computational column and its crest is the greater bed height. Width is the existing spherical dual boundary's physical length; separation is the neighboring region-center distance. A land donor's surface must exceed both the receiving surface and the crest. Relative height differences are evaluated without first rounding both absolute water levels.

```text
drive = min(upstream surface - downstream surface, upstream depth above crest)
beta  = min(maximum_diffusivity, depth_above_crest^(5/3) / (roughness * sqrt(drive / distance)))
Q     = beta * width / distance * drive                 [m³/s]
mass  = 1000 * Q * dt                                  [kg]
```

Zero or negative drive gives zero flow; no below-crest leak is inserted. Uniform Manning-like roughness is an empirical parameter. The explicit maximum diffusivity regularizes near-flat mobility and limits how quickly lakes equalize; it is a recorded approximation, not a calibrated physical constant or hidden slope threshold. Defaults are roughness `0.04 s/m^(1/3)` and maximum diffusivity `1e6 m²/s`. Step refinement and cap changes are separate controls, not a claim of identical trajectories under different approximations.

With face conductance bounded by `maximum_diffusivity * width / distance`, the substep is at most `0.45 * min(area / summed_conductance)`. This bounds simultaneous withdrawals below each old column's available mass in exact arithmetic. Subdivision follows this geometry-derived bound, not source order or frame rate. Nonfinite/overflowing operations, unresolved source/history debits or exceeding 16,384 substeps for one half-stage refuse the complete caller interval.

Two explicit arithmetic-resolution limiters apply only to this candidate. A face request whose recipient pair does not change, or whose proposed donor/recipient changes cannot reconcile within `32 * epsilon * request`, is not published. A regional evaporation receipt that cannot change the existing scalar atmospheric stock is likewise not published. Both leave the entire requested liquid at its donor, with no transfer/evaporation history. They do not repair stocks, create pending unowned water, swallow other errors or widen the existing budget tolerances. Accepted scalar atmospheric receipts retain the existing local rounding check.

`Step.regional_surface_flow` records accepted moved mass/substeps and separate counts, sums and maxima of those unapplied face and evaporation requests. They are **requests, not inventory**: repeated requests may refer to the same retained water. These limiters change the numerical flux; tiny mass alone does not prove a negligible physical effect. A tiny retained film still marks its whole column as exposed in this coarse coupling and can change later interception/infiltration. That wet/dry and soil interaction must be reviewed before physical/default promotion.

Initial connected reference bodies retain model 8's finite mobile stocks and prescribed fixed levels. Face arrivals credit their actual contact/body, never a global collector. Reference bodies do not supply regional face outflow in this candidate; there is no moving coastline, ocean dynamics, momentum, energy conservation or thermodynamic ice coupling. Coarse whole-column exposure is not fine shoreline geometry.

## Seasonal integration and accounting

The same seasonal kernel provides finite evaporation/precipitation, snow, soil, and delayed bed-based runoff. Regional surface liquid intercepts local rain/melt on its frozen half-stage footprint and evaporates from its own column. Actual terminal runoff delivers to its regional column. After each half-stage, face flow advances the regional field. Overflow follows water-surface gradients rather than the fixed bed-only runoff graph; both processes have separate histories.

`regionalSurfaceFlow` records `regional-surface-flow-1` and one high/low gross transfer pair for each direction of each canonical physical face. Those pairs are provenance, not owned stocks. Independent regional identities reconstruct both incoming and outgoing mass from the actual graph; finite reference-body identities account for wet contact arrivals. Existing complete global and atmospheric ledgers remain active with unchanged tolerances. The reused `terminalWaterKilograms` arrays mean **regional surface liquid**, including nonterminals, only under schema 15; old wire formats and basin-owner observations must refuse rather than reinterpret it.

Settings explicitly select the following payload; the legacy string variants are unchanged:

```json
{
  "closedLakeExchange": {
    "frozenRegionalSurfaceFlow": {
      "roughness": 0.04,
      "maximumDiffusivitySquareMetersPerSecond": 1000000.0
    }
  }
}
```

Compensated soil/surface/terminal numerics, the finite reference-body pool and orographic settings are also required by the existing coupled contract. The checkpoint requires matching schema/model/flow pins, complete face history, capture history and normalized stock tails, and forbids legacy leaf/parent ownership state. Missing, null or incompatible history refuses. No save migration is implemented. `Model::regional_surface_observation` exposes read-only `regional-surface-observation-1` with exact stock pairs, approximate depths/levels, recipe, clock and stability bound. Display levels may round to the bed for extremely shallow liquid; they never replace its owned mass. Existing basin-owner observations and desktop wire formats refuse model 15 rather than reinterpret its arrays.

## Recorded numerical evidence

[Retained validation summary](data/regional-surface-flow-validation.json) records every cohort case, complete resolved recipes/settings, failures, replay flags, diagnostics and source-report hashes. The reproducible full report additionally contains the final regional high/low/depth arrays. All worlds remain on fixed geography with finite prescribed-level reference bodies and an initial mobile layer of 10 m.

Nine operator tests check flat equilibrium, exact/below-crest blocking, independent dyadic conservation, analytic two-column relaxation under step refinement, symmetric simultaneous forks, no same-substep reuse of new arrivals, nested bowls crossing both real sills, actual finite-body contacts, datum/relabeling/face-order controls, geometrically scaled area/time controls, and both arithmetic limiters. These are small directed geometries, not an ensemble of resolved rivers. Four generated integration tests check concurrent real-geography columns, complete checkpoint continuation, aligned caller partitioning, independent graph identities, malformed saves, read-only observation, obsolete-wire refusal, forty-day seasonal exchange and whole-caller work-bound rollback.

The initial unreleased twenty-case prototype completed 18 seasonal years. The retained refusals were `readiness-01` at coverage 0.1 on day 104 and low-relief `first-light` with a 450 s ceiling on day 65: a positive evaporation receipt could not change scalar vapor. Both left the complete caller state unchanged. The donor-retaining vapor limiter resolves these witnesses without removing them from the cohort or changing any old model.

The finalized cohort completes **25/25 runs of 365 days**, with exact full-checkpoint round trips and next-hour continuation in every case:

- Twelve ordinary cases: four declared seeds × initial water coverage 0.71/0.3/0.1, 162 regions, 900 s ceiling and diffusivity cap `1e6 m²/s`.
- Four low-relief cases on the same seeds, coverage 0.3 and relief/detail scaled by 0.01; two first-light mobility controls (zero/`1e5`), a 450 s control and a 642-region ordinary case.
- Five deliberately funded first-light cases: synthetic rain of `1.4e17 kg` at region 8 and `2.4e17 kg` at region 156, fully debited from actual finite reference liquid with matching contact/region histories. Ceilings 900/450/225 s use the `1e6` cap; two 900 s controls use zero/`1e5`.

The ordinary cohort has **no active face flow in nineteen cases**; low-relief receiver-2 alone activates one directed contact. Its success is seasonal accounting evidence, not broad natural overflow qualification. The three funded refinement runs activate ten directed contacts each; the lower-cap control activates nine and the zero-cap control none. Funding is an artificial stress scenario, not spontaneous climate or a discovered natural history.

Maximum relative global/local residuals over accepted daily budgets are respectively `3.954e-16` and `1.027e-14`; the existing acceptance tolerances are unchanged. The regional-liquid L1 difference drops from `1.018087e12 kg` (900 versus 450 s) to `5.097116e11 kg` (450 versus 225 s); corresponding maximum depth differences drop from `0.000113629 m` to `0.000056928 m`. This is measured refinement on one directed geography, not universal convergence. Changing the cap to `1e5` instead changes L1 liquid by `8.01823e15 kg` and maximum depth by `1.06144 m`: the cap affects the physical trajectory and needs review/calibration, not a claim of cap independence.

The finest directed run records 674 unapplied face requests totaling `24.522 kg` (maximum `0.4195 kg`); the largest individual face request in the cohort is `0.4522 kg`. Its four unapplied evaporation requests total `0.00756 kg`. Across the full cohort, the largest individual unapplied evaporation request is `0.005926 kg`. No such amount is reported as transported or evaporated. These aggregate requests are not lost mass or separate reserve stocks.

One warmed release executable run of all 25 annual cases took 154.03 s wall/151.57 s CPU and 12,776 KiB maximum RSS on a Ryzen 5 PRO 4650U. Linux child-process resource usage was measured with Python's `resource.getrusage`; Cargo builds were excluded, other regression work was running. This is a small 162/642-region native cohort, not a finest-grid or GUI performance promise.

The complete retained model-14 report remains unchanged after rebuilding. Repeating the complete 25-case report is byte-for-byte exact, including all regional arrays and precision diagnostics. Full `npm test` passes native all-target tests, TypeScript checking and all 72 Node tests. A final debug rerun passes all 58 library units and four new integrations; all nine regional operator tests and four integrations also pass separately in release. Warnings-denied all-target Clippy and formatting pass. The pre-existing ignored long annual-snow qualification is unchanged and was not rerun. No GUI test or desktop promotion is implied.

## Reproduction and next product gate

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example regional_surface_flow_report -- --days 365 --output artifacts/new-regional-surface-flow-report.json
cargo test --locked --manifest-path native/Cargo.toml --lib
cargo test --release --locked --manifest-path native/Cargo.toml --test regional_surface_flow
```

Report outputs use `create_new` and refuse overwrites. Every dynamical failure records its first day, message and whole-state rollback; it is not filtered out. Recipe/model construction failures abort report creation. There are no new ignored tests.

Required before promotion:

1. An explicitly versioned desktop frame/checkpoint path for this ownership model, regional depths and initial reference bodies kept distinct on both projections, with graph budgets and exact replay. Do not silently migrate the existing seasonal mode.
2. Review wet/dry exposure and submerged-soil coupling, fixed body heads, uniform roughness, capped mobility and their physical implications. Finite reference inventories do not currently change prescribed heads or coastlines.
3. Denser active-flow ensembles, paired spatial-resolution/cap controls and longer/extreme trajectories. Coarse directed and annual accounting success is not a validated climate, hydrodynamic forecast, ecology readiness or completion of milestones C/D.

The subsequent [desktop integration](regional-water-desktop.md) implements item 1 with separate protocol-14 frames, both projections, explicit body ownership and complete schema-15 persistence. The core evidence above predates that integration. Items 2–3 remain physical qualification gates; product availability is not calibrated hydrology or default promotion.

The October 6 [ponded-soil follow-up](ponded-soil-exchange.md) retains a generated witness of the tiny-film soil switch and implements a separate conservative vertical operator. No seasonal coupling yet applies that operator; model-15 equations, checkpoints and cohort evidence above remain unchanged.
