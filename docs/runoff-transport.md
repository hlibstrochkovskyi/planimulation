# Delayed runoff and terminal-water ownership

`runoff-transport-1` adds a pure native transport operator on the existing frozen, single-receiver drainage graph. `seasonal-moisture-3` supplies physically owned runoff from its soil/liquid transfers and retains received water in a new terminal stock from which it can evaporate. It does not connect to the separate manual basin inventory or modify its exact accounting.

## Model basis and limits

[Hydrologic storage routing](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/reservoir-modeling/reservoir-modeling-concepts-and-equations) balances inflow, outflow, and storage change. A [linear reservoir](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/baseflow/linear-reservoir-model) relates storage to outflow through a response time. Our selected network is an empirical cascade of linear stores, **not** an implementation of HEC-HMS baseflow/reservoir routing, Muskingum, Manning flow, or shallow-water hydraulics. No external source code is copied. Neither source calibrates our geometric response law:

```text
tau_i = max(physical_receiver_edge_length_i / effective_speed, minimum_response)
departure_i = old_transit_i × (1 − exp(−dt/tau_i))
```

An isolated unforced reach solves dQ/dt = −Q/tau analytically. A staged network uses the old stock for all departures, then accepts arrivals, so its multi-reach solution is approximate. An independent two-reach reference has Q0(t)=M exp(−t/tau), Q1(t)=M(t/tau) exp(−t/tau), and terminal water M−Q0−Q1. Tests show decreasing network error when dt is reduced. There is no hard parcel travel-time cutoff: a linear store releases a small fraction immediately, with tau as its response time, not a promise that every drop waits exactly length/speed.

Effective speed defaults to 1 m/s (supported 0.1–5); minimum response defaults to one hour (60–86,400 s). These values are uncalibrated. Physical edge lengths come from the spherical adjacency, not projection distance or display relief. The minimum prevents arbitrarily fast short-edge release, but adds resolution dependence when it dominates. No slope, channel cross-section, roughness, or discharge feedback determines speed.

## Transfer and terminal rules

Nonterminal runoff moves only to its immediate receiver. New arrivals cannot depart again within the same routing pass. Departures to nonterminal receivers credit transit; departures to self-receiving endpoints credit terminal water directly. Runoff generated locally on a terminal transfers immediately into that terminal's stock, rather than inventing a zero-length downstream reach.

Existing wet cells and closed dry sinks are both endpoints. Several wet cells may share one canonical water-body label, but arrivals retain their actual contact cell. No minimum-ID teleport or unmodeled ocean redistribution occurs. No ocean discard occurs on this closed sphere. A terminal pool remains there except for separately recorded evaporation; it cannot spill, merge, or equilibrate a body level in this model.

Every pass checks sum(old transit)=sum(new transit)+sum(terminal deliveries), with a 32-machine-epsilon stock-scale tolerance and 1 kg floor. Invalid stocks, cycles, non-neighbor receivers, bad lengths, excessive intervals, or nonfinite/overflowing results reject. The graph is immutable, input stocks are borrowed, and results are returned separately. A nonterminal routing interval must be no longer than the shortest response divided by six. The seasonal caller already subdivides its clock to satisfy this bound; there is no unbounded internal retry.

## Coupled accounting and observation

The seasonal checkpoint saves four integrated transfer records per region: sent, received transit, terminal delivery, terminal evaporation. Each uses a checkpointed Kahan correction. Stocks are separate from these cumulative diagnostics: summing gross departures across all reaches repeatedly counts traveling water and is not a production or total-inventory measure.

The caller debits terminal evaporation once, gives only remaining atmospheric demand to other donors, and credits actual total evaporation to vapor. Closed dry pools use their region area as an effective evaporating footprint while the original soil/temperature mask stays fixed. This deliberately coarse rule does not infer a dynamic lake surface, area, freezing state, or new biome.

Reported sent kilograms over a known interval can be divided by 1,000 kg/m³ and interval seconds to obtain an average modeled volumetric transfer rate. It is not instantaneous measured river discharge or a solved flood wave. There is no current desktop dynamic river overlay.

Directed tests cover a pulse moving one edge, finite storage, analytical decay/cascade controls, branches, physical relabeling, distance/speed scaling, first-contact water cells, a receiver crossing the atlas seam on a real spherical mesh, malformed physical adjacency, full terminal retention, cycles, overflow, and checkpointed summation. Seasonal tests reject a globally balanced fabricated arrival on a node that the receiver graph does not supply. Generated-year and ten-year ensembles additionally measure total water, per-region ledgers, exact replay, step sensitivity, and an otherwise matched routing-disabled control. [Measured results and scope](seasonal-moisture.md#measured-version-3-evidence) are recorded separately from these algorithm choices.
