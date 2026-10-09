# Unified soil-water native transport

October 8, 2026: protocol 15 and an independent typed TypeScript session connect the existing [unified regional soil cycle](regional-soil-cycle.md) to a Node client. This increment implements transport and exact checkpoint continuation, **not an Electron mode, map layer, inspector or file dialog**. Existing desktop defaults, protocol 14, legacy saves and seasonal equations are unchanged.

## Selected family and lifecycle

The family is `regional-seasonal-water-1`, complete checkpoint schema 1. It is not legacy seasonal schema 1 or a migration of model 15. The product transport explicitly selects the already-qualified `retainDonor` policy with the existing default water, temperature and wind settings. Its soil, atmosphere, surface-flow and drainage pins differ from strict numerical refusal. Custom headless settings and strict-policy checkpoints are refused by this adapter, never silently converted.

Native commands are:

- `initializeSoilMoisture`: initialize this family on a fresh generated world.
- `restoreSoilMoisture` with original `checkpointJson`: restore into a fresh matching generated world after authoritative native validation.
- `seasonalMoisture` with `seconds`: observe at zero or advance by an integer 1–86400 seconds.
- `exportMoisture`: export the selected family's complete checkpoint.

Exactly one seasonal family can own a process. Neither legacy seasonal state nor changed manual water inventory can coexist with this cycle. Regeneration clears the selected family. Clock limits remain ten model years. A new-cycle advance runs on a candidate state: computation, observation validation and frame publication must succeed before the session replaces its prior state. An injected writer failure test verifies that the state is not published. A partially failed pipe is not a recoverable client connection; malformed transport closes the client.

`SoilMoistureSession.initialize` and `.restore` each create an independent native process. A failed candidate does not replace an existing session. `advance`, `checkpoint`, read-only snapshot getters and `close` provide a narrow API; existing Electron `NativeController` mode selection is deliberately not extended yet. Snapshots returned to consumers are copies, so display code cannot mutate the previous frame used for interval validation. Requests retain the existing one-in-flight backpressure and timeout.

## Display packet

Framing remains a four-byte little-endian header length, JSON header and bounded binary body. Protocol 15 accepts only `soilMoisture` and `soilMoistureCheckpoint`. World geometry still uses its own unchanged protocol and is sent once per generated session. Display frames contain no terrain or graph geometry.

For `N` regions and `B` initial connected reference bodies, the display body contains `(37N + 2B)` little-endian float64 values, field-major:

| Regional field index | Values | Units |
| --- | --- | --- |
| 0–9 | Liquid, soil, snow, vapor, delayed drainage; high then low for each owner | kg |
| 10–23 | Cumulative rain, snowfall, melt, liquid evaporation, soil evaporation, infiltration, soil drainage; high then low for each history | kg |
| 24–30 | Completed local transfer in the requested interval, in the same seven-process order | kg |
| 31 | Terrestrial liquid depth from its leading component and physical area | m |
| 32 | Visible land liquid depth or prescribed reference-body depth | m |
| 33 | Visible water level; zero placeholder at zero depth | m |
| 34–35 | Rounded cumulative actual-face incoming and outgoing surface mass | kg |
| 36 | Completed outgoing delayed-drainage transfer in the interval | kg |
| After the regional fields | One body-high array, then one body-low array of length `B` | kg |

The two components belong to one stock; signed low components are not errors to discard or additional owners. Initially wet regions do not also own terrestrial liquid, soil or drainage. Body IDs are reconstructed once from the initial world in ascending positive order; body mass is counted once per connected body, not once per wet region. No synthetic legacy terminal-water or liquid-runoff field is inserted.

Interval quantities are cumulative-pair differences using compensated summation. They are completed transfers, not rates, demands or numerical deferrals. Zero-second observations and restored snapshots report zero interval quantities. Cumulative actual-face fields are rounded integrals, not instantaneous discharge or stocks. Reference geometry remains prescribed even when the finite mobile body stock changes.

The header includes the family, all subsystem/forcing/observer pins, resolved settings, region/body counts, absolute and interval clocks, coupled/atmospheric/surface substep counts, validated native budget and persistent numerical-resolution diagnostics. Deferred requests remain separate from completed transfers. Their summed requested mass can include repeated requests for the same retained donor water; it is not net lost water.

## Validation and persistence

The independent TypeScript decoder checks exact field shape, finite nonnegative leading components, normalized signed pairs, ownership, soil capacity, derived depth/level formulas, counted-once planetary inventory, gross surface inflow/outflow reconciliation, metadata/settings/pins and bounded clocks/substeps. When a previous frame is available, it checks local cumulative increments against interval transfers and monotonic diagnostics/flows. Zero-second observation cannot alter any transported stock/history or budget. These display checks are not a substitute for Rust's complete per-region, body and canonical-contact checkpoint ledgers; atmospheric and surface directed histories remain in the complete checkpoint rather than every display frame.

JSON restore passes the original text to Rust without reserializing JavaScript numbers. Export appends one newline to the original native JSON. Signed zero and persistent low components survive complete replay. Bounds remain 32 MiB display body, 32 KiB header, 64 MiB complete checkpoint including the appended newline and a separately bounded escaped restore envelope. Unsupported settings, family mismatches and malformed files fail before candidate publication.

Tests cover byte-exact native field layout, read-only observations, full checkpoint continuation, failed output rollback, real-process daily/hourly equality, preserved signed zero, funded neighboring face flow with active soil, dense donor-retention diagnostics, corrupted transport rejection, legacy/manual isolation and a short 40,962-region save/replay check. The directed funded history uses a real adjacent atmospheric contact and a finite body donor; it is a stress fixture, not naturally reached rainfall.

The eight focused TypeScript tests can be run with individually reported results:

```bash
npm run build:native
node --import tsx --test --test-isolation=none --test-reporter=tap tests/soil-water.test.ts
cargo test --locked --manifest-path native/Cargo.toml --test soil_desktop --bin planimulation-core
```

The measured 40,962-region `first-light` terrain-preparation world at second 3600 produces a 12,125,040-byte display body and a 32,071,164-byte checkpoint including the appended newline. Both fit their existing limits; these are one short trajectory's measured sizes, not guarantees for every future clock/seed.

The [transport validation record](data/soil-water-transport-validation.json) keeps focused checks, measured sizes and source fingerprints separate from the earlier annual numerical qualification. This increment does not rerun or relabel that earlier scientific cohort.

Validation completed October 8: full `npm test` exits successfully with 502 passing native tests and all 17 Node test files. The single pre-existing ignored annual-snow test remains unchanged. The eight new TypeScript tests additionally pass with individual TAP reporting; two native transport tests and the failed-output rollback test pass in debug and release. Warnings-denied all-target Clippy, formatting, diff checks and production build pass. The production build retains its large-chunk warning; no GUI interaction test or long-duration finest-grid study was rerun for this transport-only increment.

## Next gate and limits

At the October 8 transport increment, explicit Electron candidate acceptance, opt-in mode selection, named preload operations, ownership-aware layers/budgets/inspectors and GUI regression checks were the next product gate. The October 9 [desktop follow-up](soil-water-desktop.md) implements that gate using this unchanged protocol and equations; its evidence is recorded separately from the transport validation above.

Long-duration largest-grid performance, dynamic reference-body heads/coasts, mobility/soil calibration, dense active-flow spatial qualification and long-term physical drift remain open. Short checkpoint size/replay checks and successful desktop integration do not qualify general hydrology or ecology readiness.
