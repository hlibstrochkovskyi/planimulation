# Bounded prescribed-water inventory

Status: opt-in native-core state and checkpoint for prescribed runoff that stays **strictly below every reached spill threshold**. It is not a general dynamic water solver, a climate model, or a desktop feature.

## Accounting contract

The state imports the [exact initial-water accounting](exact-initial-accounting.md) for a generated recipe, then applies [prescribed runoff routing](prescribed-runoff-routing.md) on that world's static receiver graph. Each canonical terminal's integer volume is assigned to its initial active basin branch. Existing oceans and closed dry sinks remain accounted for; no terminal input is dropped. The physical ledger uses `i128` units of `2^-56 m³`, with decimal strings in JSON checkpoints:

```text
sum(active branch stock) = exact imported initial total + accepted prescribed input
```

Capacities are reconstructed from the same represented regional `area × depth` contributions as the initial import, evaluated at each active branch's spill level. A positive input that reaches or crosses a spill threshold rejects atomically. This avoids inventing a tie-break, rounding excess into a sibling, or silently creating a merged branch. Inputs that fit remain owned by their receiving branch, including amounts too small to change a displayed water level.

The optional level query inverts the existing basin storage curve approximately for inspection. It never writes the exact stock or controls routing. A level may appear unchanged after a small accepted input.

## Checkpoint and limits

The checkpoint records its own version, the complete versioned initial import and recipe, active branch IDs and exact stocks, accepted-input total, and a step count bounded to JSON's exact integer range. Restoration regenerates the pinned world and rejects a mismatched origin, noncanonical decimal units, altered frontier, impossible stock, unsupported spill state, or broken ledger. Failed inputs do not change stock or counters. Directed tests cover dry sinks, a fully wet closed planet, small exact ocean inputs, replay after JSON round-trip, and atomic failures.

This is **prescribed terminal delivery**, not a timed flow law. The receiver graph is frozen at the initial generated world; expanding water within an unchanged active branch can alter the visible shore, but this increment does not recompute drainage, transport time, or water-body IDs. In particular, it cannot continue past a spill or merge, withdraw water, model evaporation, or claim that its derived level is a physically equilibrated surface after a finite interval. The next integration must specify an explicit conserved spill/merge transition and refresh or replace routes when the wet topology changes. This bounded inventory does not resolve the general simultaneous-allocation and numerical-ownership gates in [water accounting](water-accounting-contract.md).
