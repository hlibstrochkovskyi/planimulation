# Concurrent network forcing: milestone C3g

Implemented scope: a standalone, closed, initially dry network with at most 128 regional columns. Experiment `simultaneous-network-1`, policy `constant-forcing-frontiers-1`. It reuses [C3f](spill-network.md) geometry, receiving-frontier queries, storage curves and validation, but **never calls its ordered-pulse filler**. C3f and its checkpoint versions remain unchanged. World model `basins-1`, protocol 7 and desktop water remain unchanged.

## What simultaneous means here

Each interval prescribes nonnegative volumes at distinct surface regions. All inputs are supplied uniformly throughout the **same normalized interval**. Missing regions supply zero. Array order has no temporal meaning: inputs are validated and sorted by region before any arithmetic, and duplicate regions are rejected rather than interpreted as repeated pulses. Empty or zero-only intervals are allowed and advance the interval counter without water events.

This is one explicit experimental interpretation, not a claim that a volume batch uniquely determines real hydrology. The model does not specify seconds, channel conductance, travel delay, momentum or pressure-driven discharge. Routing between communicating reservoirs is instantaneous; input volumes describe a constant forcing profile. Changing that profile between intervals can change the result.

In exact arithmetic, proportionally splitting one interval preserves its constant forcing profile and final stocks. Arbitrarily splitting its sources into successive calls does not. Floating-point partition and relabeling comparisons use tolerances; identical canonical requests and checkpoint continuation are tested for exact equality on the supported build.

## Fixed-state rate routing

For each segment between storage events:

1. Route external regional forcing to its existing dry-bed terminal, as in C3f.
2. At an active, underfilled branch, retain its total forcing.
3. At an inactive parent, find the receiving frontier for each saturated immediate child. Traverse only saturated siblings and actual sill contacts; stop at underfilled recipients.
4. Divide that source's forcing among unique frontier recipients according to the prescribed branch weights. Add it at each recipient's unambiguous internal entry leaf.
5. Combine incoming and local forcing before recursively computing rates inside each underfilled child.

No stock changes while these rates are being assembled. Thus two saturated sources can feed the same recipient concurrently; neither gets to fill it first. Cycles use the existing visited incidence graph. Repeated contacts and alternative paths do not multiply weights. Receiver weights remain dimensionless preferences, not measured conductances.

For a fixed active frontier this routing is linear in forcing. Each full child is handled by its parent; each underfilled child is evaluated once with its combined input. Different nested entry leaves remain unsupported: preparation retains C3f's conservative rejection rule.

## Global storage events

Let `Q` be the interval's total supplied volume and `q_i` the volume retained by active stock `i` per normalized interval under the current routing. Use cumulative supplied volume, not repeatedly subtracted time fractions, as the event coordinate:

```text
f_i = q_i / Q
event_input_i = (capacity_i - stock_i) / f_i
step = min(remaining_input, all finite positive event_input_i)
delta_stock_i = f_i × step
duration_fraction = step / Q
```

The closed root has no finite capacity. An exactly limiting stock receives its remaining deficit. All active stocks advance over the same segment; only then are full siblings replaced by their parent stock. Recompute routing after every saturation, including an inner merge that changes where incoming water is retained. The last segment can end without saturation. A zero-volume interval bypasses division.

This coordinate change is algebraically equivalent to constant forcing over time. A regression initially exposed a spurious tiny tail when a 9.5 m³ input was expressed through repeatedly subtracted normalized fractions; direct supplied-volume coordinates handle that existing nested fixture without epsilon truncation.

Each nonfinal segment saturates a previously underfilled branch. No branch can unfill, so the loop permits at most `K` saturation segments plus a final tail for `K` hierarchy nodes. This is not iterative flow relaxation.

## Distinguishing concurrency from source order

Use C3f's fork: capacities A=4, B=1, C=2, D=3 m³, equal weights, separate sills joining A–B, B–C and A–D. Initially dry, prescribe A=5 and B=1 m³ over one interval.

- A fills at fraction `4/5`. B then contains `4/5` m³.
- B fills after another `2/35` of the interval, receiving both its local input and half A's overflow. D receives `1/7` m³ during this segment.
- For the remaining `1/7`, the full A–B component supplies C and D at equal rates of 3 m³ per interval each.

| Input interpretation | Final C | Final D |
| --- | ---: | ---: |
| Ordered A, then B (C3f) | 1/4 | 3/4 |
| Ordered B, then A (C3f) | 1/2 | 1/2 |
| Constant concurrent forcing (C3g) | 3/7 | 4/7 |

Every case conserves 6 m³, but they are different histories. Sorting ordered pulses would provide deterministic priority, not concurrency.

## Precision and atomicity

The existing stock contract is retained: interval and cumulative water budgets use `max(1e-9 m³, scale × 1e-12)`; reconstructed storage uses `max(1e-9 m³, stock × 1e-10)`. Each segment additionally checks retained-rate and volume sums. Residuals are diagnostics, never stock corrections. A final canonical receiver gets the checked arithmetic remainder of each weighted source-rate split, as in earlier allocation experiments.

Nonfinite arithmetic, nonpositive progress, lost positive additions, invalid thresholds, counter overflow and budget failures reject the **entire interval**. No source commits if another concurrent update fails. An unresolved tiny addition is still a supported failure mode, even if other inputs make the interval total representable. Requests are not retried with a changed forcing profile.

Decision for this increment: **do not introduce an implicit tiny-volume accumulator**. Retaining water for later release would require an explicit pending stock, a source/entry association, a ledger and a temporal policy: delayed release can change which routes are open. Those semantics and persistence need a separate versioned experiment before small-step planetary forcing is safe. This model neither discards tiny input nor claims to solve all numerical scales.

## Reports, persistence and limits

Each interval reports canonical inputs and chronological global segments. Segments include supplied volume, interval-duration fraction, routing-rate witnesses, retained exclusive-stock increments, saturated branches and parent merges. Rates are `cubicMetersPerInterval`, **not m³/s**. Nested transfer rates describe internal routing and must not be summed as additional external input. Retained increments are measured before endpoint merges.

Checkpoints include the new experiment/policy versions, geometry, weights, exclusive stocks, cumulative input and the completed-interval counter (the reused `pulseCount` field). Only interval boundaries are saved; there is no hidden fractional interval, queue or pending water. Derived indexes are rebuilt and validated. An old ordered-network checkpoint fails rather than being silently reinterpreted. The internal reuse of C3f's audited stock representation is not a public migration operation.

The retained geometry remains C3f's `O(NK + K² + E)` representation. Per-segment routing traverses the bounded hierarchy and frontier searches, with temporary branch arrays and route witnesses. At most `K+1` segments are retained in one interval report. Neither planetary scaling nor a linear runtime bound is claimed; the developer CLI also bounds request bytes and interval count.

## Developer scenario

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example simultaneous_network -- docs/scenarios/simultaneous-network.json
```

The first interval supplies the fork above. The second supplies 11 m³ at A, producing one common 17 m³ lake at 5 m, including all three sill columns. Continuing with 7 m³ produces 24 m³ at 6 m.

Requests use `{"start":{"dry":SETUP},"intervals":[[INPUT,...],...]}` or `{"start":{"checkpoint":CHECKPOINT},"intervals":[...]}`. Each input is `{ "region": ID, "volumeCubicMeters": V }`. Use `-` for stdin. Unknown fields, more than 32 KiB or more than 1,024 intervals fail. The whole request must succeed before stdout is published. This is not a world-state importer or desktop control.

## Validation

Coverage includes exact input-list permutation, region/edge relabeling within tolerance, proportional interval subdivision, exact JSON continuation, blocked and long transit, multiple concurrent spill sources, simultaneous threshold ties, inner/outer merge events, cycles and alternative paths, common sill storage, empty/root cases, and atomic numeric/validation failures. Single-source results are compared to C3f; shared-sill stocks are compared to C3e.

One hundred weighted and area-scaled forks match independent three-segment equations. Sixty branched/cyclic networks match an independent leaf-graph oracle that pools full connected components and advances physical time, rather than using the production hierarchy traversal or supplied-volume coordinate. Raw columns independently check leaf level–volume reconstruction. All generated test cases are retained; none are skipped after failure.

Validation on 2026-09-28:

- `npm test` passed: type checking, release native build, 123 Rust tests across all targets and 43 TypeScript tests. The final non-binary checkpoint assertion and event-coordinate identifier cleanup also passed targeted reruns.
- `cargo fmt --check`, Clippy across all targets with warnings denied, and `git diff --check` passed.
- Release CLI file/stdin outputs and reordered-input outputs matched byte-for-byte. The fixture ended at 17 m³ / 5 m; continuation with 7 m³ produced 24 m³ / 6 m and exactly matched uninterrupted execution.
- Ten invalid CLI cases failed without stdout, including duplicate regions, old policy/checkpoint versions, a late invalid interval, and the 1,025-interval count limit below the byte bound.
- `npm run test:desktop` passed. Desktop and separate headless checks retained fingerprint `2e66ac09`; default world arrays remain 10,242 regions and 5,792,952 bytes.

The existing Vite chunk-size advisory remains. No new packaging run, visual feature, scientific calibration or planetary performance claim is included.

## Next increment

Specify explicit pending-input ownership and timing before attempting sub-precision forcing, then test initial-inventory mapping and ambiguous receiving entries on generated basin geometries. Planetary-scale storage, timed discharge, drying/splitting and desktop water animation remain separate work. Concurrent laboratory forcing is not yet a planetary water cycle.
