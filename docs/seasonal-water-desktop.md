# Seasonal water in the desktop

Status: implemented October 3, 2026. This is an opt-in viewer/controller for the unchanged `seasonal-moisture-3` finite-water model, not general planetary hydrology. The same Rust model runs headlessly and in the desktop native process. Generation recipes, initial-world fingerprints, and earlier water-frame layouts remain unchanged.

## Operation and ownership

1. Generate a world and select a region in the flat atlas or globe.
2. Choose **Initialize seasonal water**. Initialization is lazy; ordinary world viewing does not allocate or run the seasonal model.
3. Choose a one-hour or one-day caller interval. **Advance one interval** submits exactly that elapsed duration; **Run seasonal water** repeats bounded requests.
4. **Pause seasonal water** stops scheduling. One already requested interval can finish and publish its accepted result. There is no queued backlog of future days.

The Rust process owns the model, six stocks, all cumulative ledgers and corrections, and the elapsed clock. UI refresh rate, view changes, zoom, display exaggeration, and layer selection do not advance that clock. The caller interval is not the internal numerical step: the existing hourly-or-smaller coupling and transport stability substeps remain authoritative. The existing ten-year run limit still applies.

Initialization uses the pinned report defaults: up to a 1 m active partition of initial water, an initially dry atmosphere, 150 kg/m² soil capacity, and empirical routing at an effective 1 m/s with a one-hour response floor. It does not subtract this partition from the full reference ocean or the manual basin inventory. Temperature and wind settings are also pinned to their existing defaults and validated on receipt.

The manual water laboratory is separate. Accepted manual input blocks seasonal initialization until regeneration; seasonal initialization blocks manual input and manual checkpoint export in that session. Reading a step-zero manual budget before initialization is allowed and does not supply water to the seasonal partition. Opening a manual checkpoint uses a new candidate process, not a conversion between inventories. Diagnostic diffusion is disabled in the seasonal UI, although it remains a separate native diagnostic operation.

The twelve-month normals selector remains an independent analytical view. Actual seasonal forcing follows elapsed model time, not that selector. The displayed next forcing month is derived from the existing 365-day calendar.

## Fields, units, and inspection

Six stock layers show local liquid, snow water equivalent, soil water, runoff in transit, terminal water, and atmospheric vapor. At 1,000 kg/m³ water density, `kg / reference_region_area` is numerically millimeters of water equivalent. A terminal pool's reference-area column is **not a lake depth**, wet fraction, or surface elevation.

The precipitation layer is the last interval's `(rain + snowfall) / area × 86400 / seconds`, in mm/day. The drainage layer is the last interval's `sent / 1000 / seconds`, in m³/s: a mean recorded departure rate, **not instantaneous hydraulic discharge**. Gross departures count traversals; they do not create water. A zero-second observation has no interval transfers or derived rate.

Colors use fixed logarithmic scales, `min(1, log(1 + value) / log(1 + maximum))`. Maxima are presentation choices: 1,000 mm WE for liquid/snow/transit/terminal, 150 mm WE for soil, 60 mm WE for vapor, 20 mm/day for precipitation, and 1,000,000 m³/s for departures. Legends mark the saturated upper endpoint with `≥`. Stocks above these bounds remain unchanged and are shown numerically in the inspector. These scales are not physical capacity limits.

The region inspector shows all six current stocks and twelve transfer amounts from the last received interval. The global inspector shows exclusive stock totals, initial mobile water, mass and regional-ledger residuals, and cumulative atmospheric/terminal exchanges. Flow integrals can count recirculated water and must not be added to stock totals. Map and inspectors use the same received snapshot.

Seasonal frames only change a scalar texture; they do not rebuild terrain or water caps. **Land and water**, depth, body IDs, drainage, and basin hierarchy continue to show initial geography (or the separate manual display). No evolving lake levels or shoreline updates are inferred from pooled kilograms.

## Protocol 11 and lifecycle

The named preload operation is `seasonalMoisture(epoch, seconds)`. Integer `seconds = 0` initializes or observes without advancing; `1..86400` advances an initialized model. Requests reject stale epochs, simultaneous session commands, invalid durations, and clock-limit violations. The native process independently validates these rules and mixed-mode restrictions.

Only `kind: "moisture"` uses protocol 11. Initial world protocols 9/10, diagnostic frames, manual water, and read-only normal frames are unchanged. The existing 32 KiB header and 32 MiB body bounds remain in force. The header carries region count, six model versions, resolved moisture/temperature/wind settings, elapsed/interval seconds, substep counts, residual diagnostics, and the full global budget.

The binary body is little-endian, field-major Float64, exactly `18 × N × 8` bytes:

| Position | Fields in order | Meaning |
| --- | --- | --- |
| 0–5 | liquid, snow, soil, transit, terminal water, vapor | Current owned kilograms |
| 6–13 | rain, snowfall, melt, liquid evaporation, soil evaporation, infiltration, liquid runoff, soil drainage | Last-interval kilograms |
| 14–17 | sent, received transit, terminal delivery, terminal evaporation | Last-interval kilograms |

The adapter checks pinned settings/versions, array lengths, finite nonnegative values, fixed-area soil bounds, terminal ownership, clock/substep metadata, and reconciliation of the six totals, cumulative transfer identities, and vapor/global budgets. Each new interval's aggregate transfers must also reconcile with the previously received cumulative budget. These display checks do not replace the Rust checkpoint validator or prove historical reachability of edited state.

Only one request is outstanding, with at least 33 ms between completed playback requests. The existing 60-second native timeout still applies. A candidate world does not replace the active process until generation and GPU preparation succeed. An old-world seasonal step that finishes during candidate preparation remains visible if the candidate is canceled; a result from an actually replaced epoch cannot overwrite the new world.

The implementation follows named, narrow [Electron context bridges](https://www.electronjs.org/docs/latest/api/context-bridge), keeps heavy calculation outside the UI as discussed in [Electron's performance guide](https://www.electronjs.org/docs/latest/tutorial/performance), and updates the existing [Three.js DataTexture](https://threejs.org/docs/pages/DataTexture.html). IPC still copies arrays; this is not zero-copy transport.

## Validation and measured limits

Checks include exact field-major agreement with native headless stocks/transfers, one-day versus 24-hour caller agreement, read-only observations, monthly/year-boundary continuation, malformed protocol/stock/settings rejection, transfer/budget reconciliation, command backpressure, mode isolation, and preserved accepted state after candidate cancellation. The real Electron suite checks both projections, units/legends, region values against a separate native headless controller, run/pause, cancellation with an in-flight step, and reset on regeneration. Full project tests, formatting, and Clippy pass; unchanged model equations retain their earlier [nineteen-case numerical evidence](seasonal-moisture.md#measured-version-3-evidence).

Run the GUI timing workload separately from tests/builds:

```sh
npm run test:desktop
npm run benchmark:desktop -- --seasonal
```

Measured October 3 on Ryzen 5 PRO 4650U, approximately 14.84 GiB usable RAM, Electron WebGL/GPU compositing enabled. Each sample has 180 requested-animation-frame intervals under alternating zoom, vapor texture updates, a selected-region inspector, and daily native steps after 30 days of warmup. These are scheduling observations, not GPU execution timers or a promised FPS. Only 4–16 request timings were observed per sample; this is a short workload, not long-run fine-grid validation.

| Regions / view | Request median / observed maximum | RAF median / p95 / maximum |
| --- | ---: | ---: |
| 10,242 / flat | 155.2 / 158.2 ms | 16.7 / 16.8 / 33.3 ms |
| 10,242 / globe | 142.1 / 148.3 ms | 16.7 / 16.7 / 16.8 ms |
| 40,962 / flat | 739.8 / 752.3 ms | 18.2 / 23.8 / 69.8 ms |
| 40,962 / globe | 522.3 / 584.3 ms | 16.7 / 19.0 / 50.0 ms |

Request timings include native calculation, pipe transport, decoding, and Electron IPC, but exclude subsequent renderer/GPU work. Initialization including UI handling took approximately 1.49 s at level 5 and 2.95 s at level 6. Geometry was not resent; binary water frames were 1,474,848 and 5,898,528 bytes respectively. Short-sample global relative residuals were at most 1.8e−16 and regional-ledger residuals at most 4.2e−14. The [retained compact timing record](data/seasonal-desktop-validation.json) contains all four samples; raw GPU/process information is in ignored `artifacts/seasonal-desktop-benchmark.json`.

The largest grid showed occasional 50–70 ms scheduling gaps. This does not establish ideal smoothness, millennia-per-second throughput, or a bottleneck attribution. Layer-selective snapshots, reduced inspector cadence, profiling of decoding/copies, and larger-world LOD are possible measured follow-ups, not implemented optimizations.

## Persistence and next work

A display frame is not a complete checkpoint: it omits cumulative regional corrections and other resumable data. The native model already has complete schema-3 checkpoint replay, but **desktop seasonal save/load is not implemented** in this increment. Recipes, resolved initial-world exports, and manual water checkpoints do not preserve a seasonal run. Regeneration or closing/reloading the application discards it; the UI states this explicitly.

The next bounded integration is desktop save/load of the complete seasonal checkpoint with candidate-process validation and exact continuation tests. Lake spill levels, groundwater, changing geometry, terrain lifting, weather, ecology, and calibration remain separate model gates. Milestone D is still incomplete.
