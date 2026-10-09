# Unified soil-water desktop integration

October 9, 2026: Electron now exposes the existing `regional-seasonal-water-1` family through an explicit opt-in mode. The [regional soil equations](regional-soil-cycle.md) and [protocol-15 transport](soil-water-transport.md) are unchanged. This closes the adapter/display gate, not general hydrology, physical calibration or milestone C/D.

## Use and ownership

Generate a fresh world, then choose **Initialize unified soil water** in the seasonal-water controls. Run/pause and the selected interval use the same bounded native clock as other seasonal modes. Regeneration clears the selection. A manual inventory or a different initialized seasonal family cannot share this session; switching requires a fresh world or complete checkpoint replacement. Existing default initialization and old saved trajectories remain unchanged.

The mode exposes ten seasonal layers in both the flat atlas and relief globe: terrestrial liquid, snow, soil, delayed soil drainage, vapor, interval precipitation, interval drainage departure, terrestrial liquid depth, cumulative surface inflow and cumulative surface outflow. Layer labels identify unified liquid and soil-only drainage rather than interpreting them as legacy runoff. The legacy terminal-stock layer is disabled: this family has no such owner. Fixed color scales affect presentation only; precipitation and drainage rates use actual completed transfers over the requested interval, not annual normals or cumulative history divided by the current interval.

The physical surface additionally uses the native visible depth/level snapshot. Globe water caps remain separate from the unchanged bed; the atlas uses the same physical regions. Initial reference-body geometry stays prescribed. Initially terrestrial regions become visibly wet only when the native leading-component depth is representably positive. A sub-display liquid pair can remain in the simulation without creating a fictitious visible cap. Whole-region masks are a display approximation, not resolved shorelines; switching projections does not advance the native state.

Global budgets distinguish five paired regional owners from finite reference-body liquid, counted once per connected body. Selected-region inspection shows received high and signed-low kilogram components separately, including signed zero, alongside rounded leading-component columns. Selecting an initially wet region reports the whole shared body pair explicitly, not a fictitious local allocation. Cumulative local histories and surface-face integrals are completed transfers, not extra inventory. No synthetic terminal evaporation or second liquid-runoff process is inserted.

Persistent donor-retention diagnostics remain visible: lifetime deferred-request count, summed requested mass and largest request. Deferred water stays at its donor. The sum can count repeated requests against the same water; it is neither net lost mass nor a physical-error bound. Coupled, atmospheric and surface substep counts are shown separately. Empirical exchange, capped mobility, prescribed heads/coasts and uncalibrated settings remain identified in the inspector.

## Boundary and persistence

The sandboxed preload exposes two additional named operations, `initializeSoilMoisture(epoch)` and `seasonalSoilMoisture(epoch, seconds)`. `NativeController` keeps an independent private paired-frame baseline for strict interval decoding. Consumer mutation of a returned or candidate frame cannot change this baseline. The renderer accepts a discriminated presentation union; legacy `MoistureFrame` fields and numerical owners are not expanded or reinterpreted. Geography is still delivered once per generated session, not retransmitted with every seasonal frame. IPC copies remain; this is not a zero-copy implementation.

The shared **Open seasonal checkpoint** and **Save seasonal checkpoint** dialogs support complete family/schema-1 state. Dispatch checks the family as well as the schema: legacy seasonal schema 1 is still unsupported. Original JSON text goes to native restore without JavaScript reserialization of physical numbers. All five pairs, unique body pairs, directed histories, settings, subsystem pins, diagnostics and absolute clock remain native checkpoint state. Custom headless settings and incompatible pins reject instead of being migrated.

The existing [bounded file lifecycle](seasonal-water-checkpoints.md) is reused: 64 MiB complete checkpoint limit, bounded UTF-8 read, separate candidate generation/restore, intent revision, explicit acceptance and temporary-file replacement. A failed or canceled candidate leaves the accepted session intact, including an already requested old-world interval. Stale epochs cannot publish a new step. Restored views start paused with a zero-second observation and zero interval rates. Camera, selected layer and previous display rates are not checkpoint state.

## Validation and limits

The [validation record](data/soil-desktop-validation.json) distinguishes this product increment from earlier annual numerical evidence. Three focused adapter/display tests cover candidate cancellation and corruption, stale epochs, private baseline ownership, exact same-build continuation, signed zero, all supported layer values, immutable cap/inspector preparation and restored dense-grid deferral diagnostics. The shared funded fixture was extracted without changing the preceding eight transport tests.

Real Electron regression checks cover initialization, correct labels and owner budgets, disabled incompatible modes/layers, all ten seasonal layers plus physical surface in both projections, and byte-identical complete checkpoint continuation after save/load. A supplied finite-funded adjacent-face control verifies changing depth textures, exact selected soil pairs in both views, and complete GUI checkpoint equality with an independent headless controller. Regeneration clears the family. Existing legacy GUI scenarios run in the same suite.

Validation completed October 9: full `npm test` passes, including type checking, native build, 502 passing native tests and all 18 Node test files. The single pre-existing ignored annual-snow qualification test remains unchanged. All three new focused tests additionally pass with individual TAP reporting. Production build and the complete Electron interaction suite pass without renderer exceptions; the existing large-chunk build warning remains. Diff checks pass. Pre-existing owner camera/dragging and style changes were present during working-tree checks and remain outside this integration commit.

The funded control uses a real neighboring atmospheric face and one finite body donor, but is an edited stress history, **not naturally reached rainfall**. A coarse globe screenshot is a visual smoke check, not detailed terrain or shoreline qualification. Native equations were not changed, and this increment does not rerun the annual qualification cohort, establish dense active-flow spatial convergence, benchmark GUI playback throughput or qualify long-duration finest-grid dynamics. Physical calibration, reference-body level/coast evolution, wet-area fractions, groundwater, ecology and population remain future work.

Reproduce the focused and full checks:

```bash
npm test
node --import tsx --test --test-isolation=none --test-reporter=tap tests/soil-desktop.test.ts
npm run test:desktop
```
