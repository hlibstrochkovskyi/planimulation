# Versioned water accounting: design gate

Status: design proposal with an isolated arithmetic experiment, not an implemented solver or checkpoint format. September 29, 2026. This record separates the product requirement of conserved, reproducible water from candidate arithmetic mechanisms. No desktop dynamics are enabled by it.

## Why the current representation is insufficient

The current seeded solver stores one `f64` volume per exclusive active branch. It also uses `f64` for interval input, capacity and event coordinates. This is adequate for the bounded experiments that pass their precision checks, but adding a small positive grant to a large stock can round the stock back to its old value. Conversely, a represented stock difference can exceed its nominal grant. [The measured C3q event](event-budget-roundoff.md) differs by 0.0155029296875 m³ while its existing event tolerance is about 0.008175432 m³. Moving a nonlimiting stock by one representable step cannot repair that event. The current atomic rejection is preferable to silently changing the input or widening the tolerance.

The needed property is not “more decimal places everywhere.” It is an explicit, spatially owned account of every accepted input and represented stock change, including sub-ULP contributions and rounded allocations. A derived water level may stay visually unchanged until enough volume accumulates; the underlying volume must still be retained.

## Required invariants for a new version

1. **Local ownership.** Any unrepresented contribution belongs to an active basin stock or to a specified, checkpointed source queue. A global unlocated remainder cannot be routed correctly after a frontier changes.
2. **Conservation.** For every accepted event and interval, the model can reconcile external input with exclusive active storage, including explicit pending water if that policy exists. An approximate display total is not the accounting authority.
3. **Capacity and topology.** A branch may saturate only at its validated capacity. Merge operations transfer the entire compensated content of children to the parent without duplication, disappearance or premature access to a new frontier.
4. **Atomicity.** Invalid, overflowing or unresolved updates leave the complete checkpoint unchanged. Zero and very small positive inputs have explicit behavior; no implicit discard or unbounded retry is allowed.
5. **Replay.** Checkpoints include every low-order component, pending amount, counter and policy version needed for exact same-build continuation. V1–v5 checkpoints and their prior semantics remain readable under their own versions; no silent migration pretends to recover precision already lost.
6. **Bounded error.** Rounding bounds must be attached to local operations and reported diagnostics, not scaled only by the planet's total water. Conservation tests include long runs and adversarial size ratios, not just one successful world.
7. **Separation.** Arithmetic changes physical state only through an explicit model transition. Rendering and visual level precision never feed back into stock accounting.

## Candidate mechanisms to compare

| Candidate | Advantage | Main risk / proof obligation |
| --- | --- | --- |
| Compensated high/low volume per active branch | Retains sub-ULP grants near very large stocks; locality follows the branch | Every threshold, merge, total and checkpoint operation must use the pair correctly; rounding in rate/grant allocation still needs a policy |
| Exact fixed-point volume with bounded unit | Straightforward integer accounting and replay | The unit must cover tiny inputs and huge worlds without overflow; geometric capacities and ratios remain floating-point |
| Explicit pending volume per source or receiving branch | Makes delayed water visible and locatable | Release timing can change which route is open; source identity and merge behavior become part of the physical model |

These are proposals, not interchangeable implementations. In particular, compensated stock addition alone does **not** guarantee that separately rounded grants sum to the event input. The event-coordinate, rate split, limiting endpoint and final remainder must be reconciled under one documented rule. A deterministic last-recipient remainder is one candidate only if it stays nonnegative, respects capacity, and has a proven local deviation bound; otherwise the event must remain atomic or carry an explicitly owned pending amount.

## Evaluation order

1. Build a small, isolated arithmetic experiment with independent reference values: a large stock plus many sub-ULP inputs, one exact threshold, a two-child merge, and the retained three-recipient `profile-15` event. Measure representation error and performance. Do not wire it into the desktop or silently replace the existing `Stock` type.
2. Choose the representation and event-remainder policy from these outcomes. Define a distinct checkpoint schema/version, including strict validation and serialization round-trip tests, before modifying the generated-world path.
3. Integrate behind an explicit opt-in version. Compare directed reservoirs, the 20-seed large/small repeated-forcing sweeps, unchanged legacy replay, atomic rejection and exact serialized continuation. Keep failures in the result set rather than selecting only successful seeds.
4. Run longer, varied-resolution tests and measure runtime/memory. Only then decide whether the new version is sound enough to become the generated-world default and support a desktop demonstration.

Passing one 20-seed sweep is a regression milestone, **not** a scientific or universal reliability claim. Climate-derived forcing, finite-time discharge, drying/splitting and ocean dynamics have separate requirements.

The first [test-only arithmetic experiment](../native/tests/compensated_water_experiment.rs) now covers three narrow cases. A single `f64` stock at 5 × 10^14 m³ loses each of 1,024 grants of 1/1,024 m³; an experimental high/low pair reaches the exact +1 m³ outcome even across JSON serialization. A sub-ULP remainder is visible in a capacity comparison. The three measured `profile-15` grants reconstruct their nominal event input when stock-addition roundoff is retained locally. These observations support further evaluation of high/low storage; they do **not** prove general event reconciliation, merge behavior, safe normalization at extremes, or production checkpoint correctness. The two-child merge and varied-ensemble comparisons remain open before choosing a representation.
