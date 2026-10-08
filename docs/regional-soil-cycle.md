# Unified regional liquid and seasonal soil cycle

October 6, 2026: implemented headless native family `regional-seasonal-water-1`, checkpoint schema 1. This is a separate family, **not legacy seasonal schema 1 or a model-15 migration**. It applies ponded-soil exchange to the evolving seasonal world with paired atmospheric and delayed-drainage stocks. Existing desktop modes and saved trajectories are unchanged. The October 8 [protocol-15 transport and typed session](soil-water-transport.md) prepare integration; an actual Electron mode for this family is not implemented.

## Purpose and ownership

The retained [model-15 thin-film defect](ponded-soil-exchange.md) showed that any positive pool film could hide the soil of an entire terrestrial column. The new cycle has no separate local/pool liquid partition and no whole-column soil-exposure mask. Rain, snowmelt, surface arrivals and terminal drainage deliveries enter the **same available liquid owner** on initially terrestrial regions; soil remains active regardless of liquid depth.

All stocks and cumulative transfers use normalized signed high/low kilogram pairs. Low components belong to their stock; adapters never discard them. Dense arrays use the same generated region IDs and physical areas as the existing sphere.

| Owner | Explicit receipts | Explicit debits |
| --- | --- | --- |
| Terrestrial liquid | Rain, melt, neighboring surface inflow, terminal drainage | Liquid evaporation, infiltration, neighboring surface outflow |
| Terrestrial soil | Infiltration | Soil evaporation, soil drainage |
| Regional snow | Snowfall | Melt |
| Regional vapor | Liquid/body and soil evaporation, atmospheric inflow | Rain/snowfall, atmospheric outflow |
| Delayed drainage | Soil drainage, upstream nonterminal arrival | One downstream receiver transfer |
| Initial connected reference body | Rain/melt at wet contacts, surface arrivals, terminal drainage | Evaporation at its actual wet contacts |

Each initial connected water body owns one finite mobile stock, not one stock per wet region. Initially wet regions do not also own terrestrial liquid/soil/drainage. Snow and vapor remain regional. The initial active body depth defaults to 10 m; all other stocks begin at zero. This is a finite active-layer initialization, not total ocean inventory or an equilibrated climate.

Cumulative histories are **gross completed transfers**, never additional water. Local histories record rain, snowfall, melt, liquid evaporation, soil evaporation, infiltration and soil drainage. Atmospheric and surface histories separately record both directions of every canonical physical face, including inactive directions. Drainage history records each region's one outgoing receiver edge, including terminal self edges. Geometry and graph order are regenerated and checked on restore.

## Applied processes and interval order

Static geography and prescribed monthly temperature, wind, saturation capacity and upslope response reuse existing preparation. The old seasonal state machine is never advanced. Monthly forcing is selected from the absolute simulation day; coupled stages split at absolute cadence boundaries and midnight. The default ceiling is 900 s, further bounded by the inherited process/transport preparation. Caller intervals are 1–86400 s and complete state is bounded to ten model years.

Each coupled interval executes:

1. Vertical half-stage: supersaturation relaxation into rain or snow, degree-day melt, and warm terrestrial liquid/soil exchange.
2. Delayed drainage half-stage, then neighboring surface-flow half-stage.
3. A full paired upwind atmospheric transport stage, with requests frozen at each atmospheric substep's start.
4. The same vertical, drainage and surface half-stages again.

Vertical terrestrial exchange follows [the documented capacity-response closure](ponded-soil-exchange.md): one shared evaporation demand, infiltration capped by finite liquid and soil room, then drainage above retained soil capacity. The inherited air-temperature gate disables these four processes at `T <= 0`; this is not frozen-soil thermodynamics. Supersaturation includes the vapor low component. Evaporation demand is atmospheric deficit relaxation, not an energy balance.

Wet-contact evaporation requests are frozen together and allocated against their shared finite body using a common shortage factor. Actual transfer is attributed to each contact. If numerical retention defers a contact grant, it stays in that body and is not reassigned to another contact in the same stage.

Delayed drainage is **not a second surface-runoff generator**. It receives only soil drainage. Frozen starting stocks make outgoing requests; nonterminal edges use the existing distance/speed response, while terminal stocks deliver into local liquid or the actual wet body. Nonterminal arrivals cannot depart in the same half-stage. Terminal arrivals are delivered on reaching that terminal. Surface liquid moves only through the existing neighboring-face closure, avoiding double charging mobile water through both routes.

Surface flow uses the [model-15 capped Manning-inspired face law](regional-surface-flow.md), but debits/credits paired unified stocks and paired directed histories. Requests read frozen leading-component depths. Its explicit stability bound, positivity checks and work cap remain active. Initial wet-body heads and coasts are still prescribed; bodies receive terrestrial flow but do not supply face outflow.

## Explicit numerical policies

The default `rejectUnrepresentable` policy preserves the strict component's behavior. Every grant must reconcile actual represented donor debit, recipient credit and history increment within `32 * epsilon * grant`. Failure rejects the **whole caller interval** without changing state, clock or histories. It does not widen the tolerance to a planetary stock scale.

The opt-in `retainDonor` policy uses typed precision outcomes, not error-string catching. Both owners and the corresponding gross history are proposed together. If the proposed grant cannot satisfy the same precision check, none of the three is changed: the entire grant stays at the donor. Other processes continue. Invalid settings/state, overflow, positivity violations and work-limit failures still reject the complete caller. Neither policy repairs global mass or invents an escrow owner.

Retention is a numerical flux limiter, **not an exact integration of the requested physical rate**. Its checkpointed diagnostics record the number, summed requested kilograms and largest requested kilograms deferred. The sum may repeatedly count a request against the same retained water; it is not a stock, completed transfer or estimate of net physical error. It must be reported when evaluating a run. Requests are reconsidered from the current state on later stages; there is no hidden pending-flux queue.

| Policy | Soil | Atmosphere | Surface | Drainage |
| --- | --- | --- | --- | --- |
| Reject | `ponded-soil-exchange-1` | `moisture-transport-paired-1` | `regional-surface-flow-paired-1` | `runoff-transport-paired-1` |
| Retain donor | `ponded-soil-exchange-2` | `moisture-transport-paired-2` | `regional-surface-flow-paired-2` | `runoff-transport-paired-2` |

The numerical policy is a resolved setting, not a silently selected fallback after failure. Restore verifies all subsystem pins and the required presence/absence of resolution diagnostics. Explicit null diagnostics reject. Temperature/wind/orography/reference-body pins, complete arrays, histories, resolved generation recipe, settings and absolute clock are also preserved.

## Independent accounting and directed checks

Every regional stock identity and every shared body's contact identity is reconstructed independently from gross local/edge histories. Global accounting counts each owner once. Local cumulative identities use `128 * epsilon * max(abs(individual ledger terms), 1 kg)`; global residual must be at most `1e-12 * max(initial mobile mass, 1 kg)`. These are cumulative arithmetic audit bounds, not permission to manufacture water. Actual transfers retain the tighter grant-relative check above.

Tests cover the actual seasonal thin-film limit, paired stocks and active infiltration, fixed-terrain continuation, exact JSON save/restore, aligned daily/hourly caller partitions, dry/all-wet and disabled-process controls, model-pin/shape/pair corruption, balanced geographic teleportation, numerical-policy corruption and complete work-limit rollback. Independent integer atmospheric tests verify every owner and directed history across 1024 one-kilogram crossings against `2^70 kg` stocks. Retention tests separately check an unrepresentable recipient and an unrepresentable gross history: neither is allowed to credit only one side, and tiny retained liquid does not disable soil drainage.

The annual report runs eight generated 162-region worlds (four seeds, two coverage targets), one 642-region world, and three explicitly finite-funded active-flow controls at 900/450/225 s. Funded history follows a real adjacent wet/land atmospheric face with matching contact evaporation and rain; it is a supplied state, **not evidence that prescribed winds naturally produce that event**. The report retains each first refusal, the last accepted complete checkpoint, rollback status and same-build exact continuation.

The initial strict annual cohort retained refusals in all twelve cases: ordinary coarse worlds first refused on days 71–121, the denser world on day 1, and funded controls on days 70–71. Every refusal was atomic and every last accepted checkpoint restored and continued exactly for an hour. These failed annual trajectories are not qualified years and must not be compared at unequal final times.

The donor-retaining annual cohort completes **all twelve** 365-day caller sequences. All checkpoints round-trip exactly and continue for an hour exactly. Maximum measured global relative residual is `1.954305482968942e-16`; maximum local relative residual is `4.248548595842353e-16`. Ordinary worlds have 65–741 active surface directions, with gross annual crossings of `1.107652226825219e12`–`5.758432698767567e13 kg`. The funded controls have 129–136 active directions and about `1.836e17 kg` gross crossings. Gross crossings may count the same water repeatedly; these are not inventories or river-discharge observations.

Retention is frequent: approximately 0.92–23.80 million requests per case. Summed deferred requests range from about 41.05 to 2496.71 kg; the largest single request across the cohort is `3.6724419510754562 kg`. These tiny masses at planetary column scales explain why successful accounting is compatible with many deferrals, but do **not** prove a global physical-error bound. Local stock identities and active soil exchange remain independently checked.

The three funded cases start at second 900 and end at exactly second 31,536,900. Thus their cadence comparison is at equal absolute time, unlike the strict failures:

| Ceilings [s] | Regional liquid L1 difference / finer liquid | Regional soil L1 difference / finer soil | Relative gross surface-crossing difference |
| --- | ---: | ---: | ---: |
| 900 → 450 | `9.168448340998935e-5` | `9.833901590157023e-5` | `9.983742239403896e-7` |
| 450 → 225 | `4.5902285316367255e-5` | `4.911568198158087e-5` | `4.970173032560382e-7` |

L1 sums absolute per-region differences of both signed components, then divides by the corresponding finer case's total stock. All three differences approximately halve. This supports time refinement for this supplied active-flow state and closure, not universal convergence or spatial qualification.

Reproduce:

```sh
cargo test --locked --manifest-path native/Cargo.toml --test ponded_soil --test regional_soil
cargo test --locked --manifest-path native/Cargo.toml --lib moisture_transport::paired
cargo run --release --locked --manifest-path native/Cargo.toml --example regional_soil_report -- 365 artifacts/regional-soil-strict.json
cargo run --release --locked --manifest-path native/Cargo.toml --example regional_soil_report -- 365 artifacts/regional-soil-retaining.json --retain-donor
```

Reports refuse existing output paths. Full checkpoint reports belong in ignored `artifacts/`; [the retained qualification summary](data/regional-soil-cycle-validation.json) records resolved settings, common recipe, per-case overrides, subsystem pins, complete-checkpoint hashes, refusals, diagnostics and matched-time comparisons. Repeating the full donor-retaining report produced byte-identical output. Rerunning the strict cohort after the typed transfer refactor preserved every last accepted checkpoint exactly.

## Read-only desktop preparation

October 7 follow-up: `Model::observe` implements the separate native `regional-soil-observation-1` snapshot. It validates the supplied state and returns all five regional stock pairs, unique reference-body IDs and pairs, local cumulative histories, numerical-policy diagnostics, resolved water settings and subsystem pins. No clock, stock or accumulator is changed. The snapshot is **not a checkpoint or an Electron packet**; binary transport and actual interface selection are still to be connected.

Derived fields provide regional liquid depth, visible water depth/level and cumulative incoming/outgoing surface mass. Live land columns share the same fixed bed in both projections; initial wet-body geometry remains explicitly prescribed. Leading-component depths are display quantities. A depth below representable resolution may show no surface while its complete high/low stock remains available to the inspector and simulation. Zero levels are placeholders only where derived depth is not positive. Cumulative flows are rounded integrals, not instantaneous discharge or extra inventory.

A generated, finite-funded active-flow test checks all copied owners and signed tails, sorted unique body IDs, native depth/level formulas, matching gross incoming/outgoing totals, incompatible-model rejection and exact observed/unobserved continuation. Repeated observation leaves the complete checkpoint and regenerated terrain unchanged. This prepares a narrow shared map/globe data contract without altering the qualified seasonal equations or advertising a working new desktop mode.

Validation completed October 7: full `npm test` exits successfully, including TypeScript checking, native build, 499 passing native tests and all 16 Node test files. The single pre-existing ignored annual-snow test remains unchanged. The twelve vertical-operator tests, seven regional-cycle/observation tests and three paired-atmosphere tests also pass in release; the latest observer metadata assertions were separately rerun after the full suite. Warnings-denied all-target Clippy, formatting and diff checks pass. No new tests are ignored and no GUI interaction test was rerun for this native-only work.

## Remaining gates

October 8 transport follow-up: [a separate binary contract, strict TypeScript decoder and native-process session](soil-water-transport.md) now carry all five regional owner pairs, counted-once body pairs, local histories, actual interval transfers and policy diagnostics. Complete original-text restore and same-build continuation are tested independently of legacy modes. This does not change the qualified equations or connect Electron controls, rendering or file dialogs.

This closes an explicit seasonal ownership/coupling implementation, not a claim of general hydrology or scientific calibration. Reference heads/coasts, homogeneous soil parameters, cold gating, saturation/evaporation laws and mobility caps are still approximations. Dense active-flow spatial qualification, cadence sensitivity, longer-term drift, physical calibration and an explicit desktop adapter remain work. Dynamic reference-body levels, ocean circulation, groundwater backpressure, wet-area fractions, ecology and population readiness are not implemented by this increment. Legacy desktop model 15 deliberately retains its old soil behavior until a new mode is explicitly connected.
