# Concurrent event roundoff witness: C3q diagnostic

Status: measured and covered by a directed regression test, September 29, 2026. This increment **does not change the solver, checkpoint, acceptance rate, or desktop behavior**. The versioned v5 level inverse remains opt-in. The purpose is to rule out an attractive but unsound one-ULP stock correction before changing water accounting.

## Reproducible failure

The retained `profile-15` world from [C3o](repeated-forcing-probe.md), with the large alternating forcing fixture, accepts interval 1 and atomically rejects interval 2 at its second internal event. For this event:

| Quantity | m³ |
| --- | ---: |
| Nominal supplied segment | 8,175,431,636.645996 |
| Sum of represented stock changes | 8,175,431,636.661499 |
| Difference | +0.0155029296875 |
| Existing event tolerance | approximately 0.008175432 |

The three recipients are branches 190, 197 and 203. Branch 197 is the limiting stock and must land exactly on its validated capacity. At the resulting large stock magnitudes, one representable `f64` step is 0.0625 m³ for branch 190 and 0.03125 m³ for branch 203. The discrepancy is smaller than either nonlimiting stock's single step, but larger than the allowed event residual. A directed test reconstructs the witness through the production stock-commit function and checks every combination of retaining or moving each nonlimiting result by one neighboring representable value. None meets the existing event budget. Moving the limiting stock would violate its exact capacity endpoint.

Therefore a local “nudge one stock by one float” policy cannot solve even this specific failure without breaking another invariant. Raising the tolerance would hide a real representational mismatch and does not specify where the fractional water belongs. The event is correctly rejected under current semantics; no water is silently discarded.

## Accounting decision still needed

A robust version needs to make sub-ULP contributions to large stocks explicit and checkpointed. A candidate is a compensated residual **owned by each active stock**, transferred with that stock during a merge, with a precise rule for capacity comparison, derived water level, and subsequent routing. A single global remainder is less defensible: it loses the source/recipient geography needed when frontiers change. This is a design direction, not an implemented policy. It needs directed reservoirs, the fixed 20-seed sweeps, checkpoint replay, long-run residual bounds and performance measurements before replacing atomic rejection.

Meanwhile, the next product-facing milestone can use a bounded prescribed-water desktop demonstration only if it reports unsupported/rejected runs explicitly and does not claim general continuous hydrology. No climate-derived rain, drying/splitting or scientifically timed discharge follows from this diagnostic.
